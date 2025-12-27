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
use tokio::sync::Semaphore;
use tokio::time::timeout;

const BATCH_SIZE: usize = 10;
const TIMEOUT_DURATION: Duration = Duration::from_secs(10);

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

pub struct HealthResult {
    pub success: bool,
    pub status_code: u16,
    pub duration: Duration,
}

async fn check_health_urls(pool: &DbPool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = env::var("UPTIME_TELOXIDE_TOKEN").expect("UPTIME_TELOXIDE_TOKEN must be set");
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

    let semaphore = Arc::new(Semaphore::new(BATCH_SIZE));
    let client = Client::new();

    for health_url in health_urls {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let client = client.clone();
        let bot = bot.clone();
        let pool = pool.clone();
        tokio::spawn(async move {
            if let Err(e) = check_and_notify(&pool, &client, &bot, &health_url).await {
                eprintln!("Error checking URL: {:?} {}", e, health_url.url);
            }
            drop(permit);
        });
    }

    Ok(())
}

pub async fn check_health(client: &Client, url: &str) -> HealthResult {
    let start = std::time::Instant::now();
    let response = timeout(TIMEOUT_DURATION, client.get(url).send()).await;
    let duration = start.elapsed();

    match response {
        Ok(Ok(res)) => HealthResult {
            success: res.status().is_success(),
            status_code: res.status().as_u16(),
            duration,
        },
        Ok(Err(e)) => HealthResult {
            success: false,
            status_code: e.status().map_or(0, |status| status.as_u16()),
            duration,
        },
        Err(_) => HealthResult {
            success: false,
            status_code: reqwest::StatusCode::REQUEST_TIMEOUT.as_u16(),
            duration,
        },
    }
}

async fn check_and_notify(
    pool: &DbPool,
    client: &Client,
    bot: &Bot,
    health_url: &HealthUrl,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Uptime",
                    &format!(
                        "URL {} is down (status {})",
                        health_url.url, health_result.status_code
                    ),
                )
                .await;

            send_failure_message(
                pool,
                bot,
                health_url,
                health_result.status_code,
                health_result.duration,
            )
            .await?;
        }
    }

    Ok(())
}

fn is_success_status(status_code: u16) -> bool {
    (200..300).contains(&status_code)
}

async fn send_failure_message(
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
        "[ALARM] Health check failed for URL: {}\nStatus code: {}\nResponse time: {:.2}s. Uptime Bot will keep sending requests every minute but will send you a message only if it becomes healthy again.",
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

    Ok(())
}

pub async fn run_uptime_checker(pool: DbPool) {
    tracing::info!("Starting uptime checker...");

    loop {
        if let Err(e) = check_health_urls(&pool).await {
            eprintln!("Error in uptime checker: {:?}", e);
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
