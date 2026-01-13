use super::alerts::Severity;
use super::ALERTS;
use std::env;
use teloxide::Bot;

const BOT_TOKEN_VARS: [&str; 5] = [
    "GITLAB_TELOXIDE_TOKEN",
    "GITHUB_TELOXIDE_TOKEN",
    "BEEP_TELOXIDE_TOKEN",
    "UPTIME_TELOXIDE_TOKEN",
    "AGREEMENT_BOT_TOKEN",
];

fn get_any_bot_token() -> Option<String> {
    BOT_TOKEN_VARS
        .iter()
        .find_map(|var| env::var(var).ok().filter(|v| !v.is_empty()))
}

pub async fn send_startup_alert(severity: Severity, category: &str, message: &str) {
    if env::var("ADMIN_LOGS").unwrap_or_default() != "ACTIVE" {
        return;
    }

    let token = match get_any_bot_token() {
        Some(t) => t,
        None => {
            tracing::error!("Cannot send startup alert: no bot token configured");
            return;
        }
    };

    let bot = Bot::new(token);
    ALERTS.send_alert(&bot, severity, category, message).await;
}

pub async fn alert_database_error(error: &str) {
    tracing::error!("Database startup error: {}", error);
    send_startup_alert(
        Severity::Critical,
        "Startup-Database",
        &format!("Database initialization failed: {}", error),
    )
    .await;
}

pub async fn alert_migration_error(error: &str) {
    tracing::error!("Migration error: {}", error);
    send_startup_alert(
        Severity::Critical,
        "Startup-Migration",
        &format!("Database migration failed: {}", error),
    )
    .await;
}

pub async fn alert_http_server_error(error: &str) {
    tracing::error!("HTTP server error: {}", error);
    send_startup_alert(
        Severity::Critical,
        "Startup-HTTP",
        &format!("HTTP server failed to start: {}", error),
    )
    .await;
}

pub async fn alert_startup_success() {
    send_startup_alert(
        Severity::Info,
        "Startup",
        "Application started successfully",
    )
    .await;
}
