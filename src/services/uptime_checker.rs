use crate::observability::alerts::Severity;
use crate::observability::{ALERTS, METRICS};
use notifine::db::DbPool;
use notifine::models::HealthUrl;
use notifine::{find_chat_by_chat_id, get_all_health_urls, update_health_url_status};
use reqwest::Client;
use std::env;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::*;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;

const BATCH_SIZE: usize = 10;
const TIMEOUT_DURATION: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
const SLOW_REQUEST_THRESHOLD: Duration = Duration::from_secs(5);
const MASS_TIMEOUT_THRESHOLD_PERCENT: f64 = 50.0;

#[derive(Debug)]
pub enum HealthCheckError {
    Timeout,
    Request(reqwest::Error),
    InvalidTelegramId(String),
    ChatNotFound(i32),
    DatabaseError(String),
}

impl fmt::Display for HealthCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthCheckError::Timeout => write!(f, "Request timed out"),
            HealthCheckError::Request(e) => write!(f, "Request error: {}", e),
            HealthCheckError::InvalidTelegramId(id) => write!(f, "Invalid telegram_id: {}", id),
            HealthCheckError::ChatNotFound(id) => {
                write!(f, "Chat not found for health_url: {}", id)
            }
            HealthCheckError::DatabaseError(e) => write!(f, "Database error: {}", e),
        }
    }
}

impl std::error::Error for HealthCheckError {}

impl From<reqwest::Error> for HealthCheckError {
    fn from(error: reqwest::Error) -> Self {
        HealthCheckError::Request(error)
    }
}

/// Categorizes why a health check failed for debugging purposes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FailureReason {
    /// Request completed but returned non-2xx status
    HttpError,
    /// Request timed out (exceeded TIMEOUT_DURATION)
    Timeout,
    /// Request failed (connection, SSL, redirect, etc.)
    RequestError,
}

pub struct HealthResult {
    pub success: bool,
    pub status_code: u16,
    pub duration: Duration,
    pub failure_reason: Option<FailureReason>,
    pub error_message: Option<String>,
}

fn format_failure_reason(result: &HealthResult) -> String {
    if let Some(ref error_msg) = result.error_message {
        return error_msg.clone();
    }

    match result.failure_reason {
        Some(FailureReason::Timeout) => {
            format!("Timeout (no response in {}s)", TIMEOUT_DURATION.as_secs())
        }
        Some(FailureReason::HttpError) => format!("HTTP Error (status {})", result.status_code),
        _ => format!("Unknown (status {})", result.status_code),
    }
}

/// Statistics collected during a single check cycle for debugging.
#[derive(Debug, Default)]
struct CycleStats {
    total: u32,
    success: u32,
    timeout: u32,
    http_error: u32,
    request_error: u32,
    slow_requests: u32,
    max_duration_ms: u64,
}

impl CycleStats {
    fn record_result(&mut self, result: &HealthResult) {
        self.total += 1;
        let duration_ms = result.duration.as_millis() as u64;

        if duration_ms > self.max_duration_ms {
            self.max_duration_ms = duration_ms;
        }

        if result.duration > SLOW_REQUEST_THRESHOLD && result.success {
            self.slow_requests += 1;
        }

        if result.success {
            self.success += 1;
        } else {
            match result.failure_reason {
                Some(FailureReason::Timeout) => self.timeout += 1,
                Some(FailureReason::HttpError) => self.http_error += 1,
                Some(FailureReason::RequestError) => self.request_error += 1,
                None => {}
            }
        }
    }

    fn timeout_percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.timeout as f64 / self.total as f64) * 100.0
        }
    }

    fn is_mass_timeout(&self) -> bool {
        self.total >= 5 && self.timeout_percent() >= MASS_TIMEOUT_THRESHOLD_PERCENT
    }
}

async fn check_health_urls(
    pool: &DbPool,
    client: &Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cycle_start = std::time::Instant::now();

    let token = match env::var("UPTIME_TELOXIDE_TOKEN") {
        Ok(token) => token,
        Err(_) => {
            tracing::error!("UPTIME_TELOXIDE_TOKEN not set, skipping health check");
            return Ok(());
        }
    };
    let bot = Bot::new(token);

    let health_urls = match get_all_health_urls(pool) {
        Ok(urls) => urls,
        Err(e) => {
            tracing::error!("Failed to get health URLs: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    &bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to get health URLs: {}", e),
                )
                .await;
            return Ok(());
        }
    };

    let url_count = health_urls.len();
    let semaphore = Arc::new(Semaphore::new(BATCH_SIZE));
    let stats = Arc::new(Mutex::new(CycleStats::default()));
    let mut handles = Vec::with_capacity(url_count);

    for health_url in health_urls {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let client = client.clone();
        let bot = bot.clone();
        let pool = pool.clone();
        let stats = stats.clone();

        let handle = tokio::spawn(async move {
            let result = check_and_notify(&pool, &client, &bot, &health_url).await;

            // Record stats
            if let Ok(ref health_result) = result {
                let mut stats = stats.lock().await;
                stats.record_result(health_result);

                // Log slow but successful requests
                if health_result.success && health_result.duration > SLOW_REQUEST_THRESHOLD {
                    tracing::warn!(
                        url = %health_url.url,
                        duration_ms = health_result.duration.as_millis() as u64,
                        "Slow request (but successful)"
                    );
                }

                // Log detailed failure info
                if !health_result.success {
                    tracing::warn!(
                        url = %health_url.url,
                        status_code = health_result.status_code,
                        duration_ms = health_result.duration.as_millis() as u64,
                        failure_reason = ?health_result.failure_reason,
                        "Health check failed"
                    );
                }
            }

            if let Err(ref e) = result {
                tracing::error!("Error checking URL {}: {:?}", health_url.url, e);
            }

            drop(permit);
        });

        handles.push(handle);
    }

    // Wait for all checks to complete
    for handle in handles {
        let _ = handle.await;
    }

    // Log cycle summary
    let cycle_duration = cycle_start.elapsed();
    let final_stats = stats.lock().await;

    tracing::info!(
        total = final_stats.total,
        success = final_stats.success,
        timeout = final_stats.timeout,
        http_error = final_stats.http_error,
        request_error = final_stats.request_error,
        slow_requests = final_stats.slow_requests,
        max_duration_ms = final_stats.max_duration_ms,
        cycle_duration_ms = cycle_duration.as_millis() as u64,
        "Uptime check cycle completed"
    );

    // Alert on mass timeout - this indicates a problem with the bot, not the monitored services
    if final_stats.is_mass_timeout() {
        let message = format!(
            "MASS TIMEOUT DETECTED: {}/{} checks timed out ({:.1}%). This likely indicates a problem with the monitoring server, not the monitored services. Max request duration: {}ms, Cycle duration: {}ms",
            final_stats.timeout,
            final_stats.total,
            final_stats.timeout_percent(),
            final_stats.max_duration_ms,
            cycle_duration.as_millis()
        );
        tracing::error!("{}", message);
        ALERTS
            .send_alert(&bot, Severity::Critical, "Uptime Bot", &message)
            .await;
    }

    Ok(())
}

fn extract_error_details(e: &reqwest::Error) -> String {
    let mut parts = Vec::with_capacity(2);

    if e.is_timeout() {
        parts.push("timeout".to_string());
    } else if e.is_connect() {
        parts.push("connection failed".to_string());
    } else if e.is_request() {
        parts.push("request failed".to_string());
    }

    let mut root_cause: Option<String> = None;
    let mut source = std::error::Error::source(e);
    while let Some(err) = source {
        let msg = err.to_string();
        if !msg.is_empty() {
            root_cause = Some(msg);
        }
        source = std::error::Error::source(err);
    }

    if let Some(cause) = root_cause {
        parts.push(cause);
    }

    if parts.is_empty() {
        e.to_string()
    } else {
        parts.join(": ")
    }
}

pub async fn check_health(client: &Client, url: &str) -> HealthResult {
    let start = std::time::Instant::now();
    let response = timeout(TIMEOUT_DURATION, client.get(url).send()).await;
    let duration = start.elapsed();

    match response {
        Ok(Ok(res)) => {
            let is_success = res.status().is_success();
            HealthResult {
                success: is_success,
                status_code: res.status().as_u16(),
                duration,
                failure_reason: if is_success {
                    None
                } else {
                    Some(FailureReason::HttpError)
                },
                error_message: None,
            }
        }
        Ok(Err(e)) => {
            let failure_reason = if e.is_timeout() {
                FailureReason::Timeout
            } else {
                FailureReason::RequestError
            };
            HealthResult {
                success: false,
                status_code: e.status().map_or(0, |status| status.as_u16()),
                duration,
                failure_reason: Some(failure_reason),
                error_message: Some(extract_error_details(&e)),
            }
        }
        Err(_) => {
            // Timeout from tokio::time::timeout
            HealthResult {
                success: false,
                status_code: reqwest::StatusCode::REQUEST_TIMEOUT.as_u16(),
                duration,
                failure_reason: Some(FailureReason::Timeout),
                error_message: None,
            }
        }
    }
}

async fn check_and_notify(
    pool: &DbPool,
    client: &Client,
    bot: &Bot,
    health_url: &HealthUrl,
) -> Result<HealthResult, Box<dyn std::error::Error + Send + Sync>> {
    METRICS.increment_uptime_check();

    let health_result = check_health(client, &health_url.url).await;
    let previous_status_code = health_url.status_code;

    if let Err(e) = update_health_url_status(pool, health_url.id, health_result.status_code as i32)
    {
        tracing::error!("Failed to update health URL status: {:?}", e);
        METRICS.increment_errors();
        ALERTS
            .send_alert(
                bot,
                Severity::Error,
                "Database",
                &format!(
                    "Failed to update health URL {} status: {}",
                    health_url.url, e
                ),
            )
            .await;
    }

    if health_result.success {
        if !is_success_status(previous_status_code as u16) {
            send_recovery_message(
                pool,
                bot,
                health_url,
                health_result.status_code,
                health_result.duration,
            )
            .await?;
        }
    } else {
        METRICS.increment_uptime_failure();
        if is_success_status(previous_status_code as u16) {
            let reason_text = format_failure_reason(&health_result);
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Uptime",
                    &format!("URL {} is down ({})", health_url.url, reason_text),
                )
                .await;

            send_failure_message(pool, bot, health_url, &health_result).await?;
        }
    }

    Ok(health_result)
}

fn is_success_status(status_code: u16) -> bool {
    (200..300).contains(&status_code)
}

async fn send_failure_message(
    pool: &DbPool,
    bot: &Bot,
    health_url: &HealthUrl,
    result: &HealthResult,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let chat = match find_chat_by_chat_id(pool, health_url.chat_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::error!("Chat not found for health_url: {}", health_url.id);
            return Err(Box::new(HealthCheckError::ChatNotFound(health_url.id)));
        }
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!(
                        "Failed to find chat for health_url {}: {}",
                        health_url.id, e
                    ),
                )
                .await;
            return Err(Box::new(HealthCheckError::DatabaseError(format!(
                "{:?}",
                e
            ))));
        }
    };
    let telegram_id = chat.telegram_id.clone();

    let reason_text = format_failure_reason(result);
    let message = format!(
        "[ALARM] Health check failed for URL: {}\nReason: {}\nResponse time: {:.2}s. Uptime Bot will keep sending requests every minute but will send you a message only if it becomes healthy again.",
        health_url.url, reason_text, result.duration.as_secs_f64()
    );

    let chat_id = match telegram_id.parse::<i64>() {
        Ok(id) => ChatId(id),
        Err(_) => {
            tracing::error!(
                "Invalid telegram_id '{}' for health_url: {}",
                telegram_id,
                health_url.id
            );
            return Err(Box::new(HealthCheckError::InvalidTelegramId(telegram_id)));
        }
    };
    bot.send_message(chat_id, message).await?;
    METRICS.increment_messages_sent_for_bot("uptime");

    Ok(())
}

async fn send_recovery_message(
    pool: &DbPool,
    bot: &Bot,
    health_url: &HealthUrl,
    status_code: u16,
    duration: Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let chat = match find_chat_by_chat_id(pool, health_url.chat_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::error!("Chat not found for health_url: {}", health_url.id);
            return Err(Box::new(HealthCheckError::ChatNotFound(health_url.id)));
        }
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!(
                        "Failed to find chat for health_url {}: {}",
                        health_url.id, e
                    ),
                )
                .await;
            return Err(Box::new(HealthCheckError::DatabaseError(format!(
                "{:?}",
                e
            ))));
        }
    };
    let telegram_id = chat.telegram_id.clone();

    let message = format!(
        "[FIXED] Your endpoint {} is now healthy with status code {}. Response time: {:.2}s. Uptime Bot will keep sending requests every minute but will send you a message only if it becomes unhealthy again.",
        health_url.url, status_code, duration.as_secs_f64()
    );

    let chat_id = match telegram_id.parse::<i64>() {
        Ok(id) => ChatId(id),
        Err(_) => {
            tracing::error!(
                "Invalid telegram_id '{}' for health_url: {}",
                telegram_id,
                health_url.id
            );
            return Err(Box::new(HealthCheckError::InvalidTelegramId(telegram_id)));
        }
    };
    bot.send_message(chat_id, message).await?;
    METRICS.increment_messages_sent_for_bot("uptime");

    Ok(())
}

/// Creates an HTTP client optimized for uptime monitoring.
///
/// The client is configured with:
/// - Connection timeout: 5 seconds (separate from request timeout)
/// - Pool idle timeout: 90 seconds (keeps connections alive between check cycles)
/// - TCP keepalive: enabled to maintain persistent connections
///
/// This client should be created once and reused across all check cycles
/// to benefit from connection pooling and DNS caching.
fn create_monitoring_client() -> Client {
    Client::builder()
        .user_agent("NotifineUptimeBot/1.0")
        .connect_timeout(CONNECT_TIMEOUT)
        .pool_idle_timeout(POOL_IDLE_TIMEOUT)
        .tcp_keepalive(Duration::from_secs(60))
        .build()
        .expect("Failed to create HTTP client")
}

pub async fn run_uptime_checker(pool: DbPool) {
    tracing::info!("Starting uptime checker...");

    // Create a single HTTP client that persists across all check cycles.
    // This enables DNS caching, connection reuse, and prevents false timeouts
    // caused by DNS resolution delays affecting all monitors simultaneously.
    let client = create_monitoring_client();

    loop {
        if let Err(e) = check_health_urls(&pool, &client).await {
            tracing::error!("Error in uptime checker: {:?}", e);
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
