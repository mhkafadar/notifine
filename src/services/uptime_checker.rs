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
}

impl fmt::Display for HealthCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthCheckError::Timeout => write!(f, "Request timed out"),
            HealthCheckError::Request(e) => write!(f, "Request error: {}", e),
        }
    }
}

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

async fn check_health_urls() -> Result<(), Box<dyn std::error::Error>> {
    let health_urls = get_all_health_urls();
    let semaphore = Arc::new(Semaphore::new(BATCH_SIZE));
    let client = Client::new();
    let token = env::var("UPTIME_TELOXIDE_TOKEN").expect("UPTIME_TELOXIDE_TOKEN must be set");
    let bot = Bot::new(token);

    for health_url in health_urls {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let client = client.clone();
        let bot = bot.clone();
        tokio::spawn(async move {
            if let Err(e) = check_and_notify(&client, &bot, &health_url).await {
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
    client: &Client,
    bot: &Bot,
    health_url: &HealthUrl,
) -> Result<(), Box<dyn std::error::Error>> {
    let health_result = check_health(client, &health_url.url).await;
    let previous_status_code = health_url.status_code;

    update_health_url_status(health_url.id, health_result.status_code as i32);

    if health_result.success {
        if !is_success_status(previous_status_code as u16) {
            send_recovery_message(
                bot,
                health_url,
                health_result.status_code,
                health_result.duration,
            )
            .await?;
        }
    } else if is_success_status(previous_status_code as u16) {
        send_failure_message(
            bot,
            health_url,
            health_result.status_code,
            health_result.duration,
        )
        .await?;
    }

    Ok(())
}

fn is_success_status(status_code: u16) -> bool {
    (200..300).contains(&status_code)
}

async fn send_failure_message(
    bot: &Bot,
    health_url: &HealthUrl,
    status_code: u16,
    duration: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let chat = find_chat_by_chat_id(health_url.chat_id).expect("Chat not found");
    let telegram_id = chat.telegram_id;

    let message = format!(
        "[ALARM] Health check failed for URL: {}\nStatus code: {}\nResponse time: {:.2}s. Uptime Bot will keep sending requests every minute but will send you a message only if it becomes healthy again.",
        health_url.url, status_code, duration.as_secs_f64()
    );

    let chat_id = ChatId(telegram_id.parse::<i64>().expect("Invalid telegram_id"));
    bot.send_message(chat_id, message).await?;

    Ok(())
}

async fn send_recovery_message(
    bot: &Bot,
    health_url: &HealthUrl,
    status_code: u16,
    duration: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let chat = find_chat_by_chat_id(health_url.chat_id).expect("Chat not found");
    let telegram_id = chat.telegram_id;

    let message = format!(
        "[FIXED] Your endpoint {} is now healthy with status code {}. Response time: {:.2}s. Uptime Bot will keep sending requests every minute but will send you a message only if it becomes unhealthy again.",
        health_url.url, status_code, duration.as_secs_f64()
    );

    let chat_id = ChatId(telegram_id.parse::<i64>().expect("Invalid telegram_id"));
    bot.send_message(chat_id, message).await?;

    Ok(())
}

pub async fn run_uptime_checker() {
    log::info!("Starting uptime checker...");

    loop {
        if let Err(e) = check_health_urls().await {
            eprintln!("Error in uptime checker: {:?}", e);
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
