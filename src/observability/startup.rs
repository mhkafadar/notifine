use super::alerts::Severity;
use super::ALERTS;
use std::env;
use teloxide::Bot;

pub async fn send_startup_alert(severity: Severity, category: &str, message: &str) {
    if env::var("ADMIN_LOGS").unwrap_or_default() != "ACTIVE" {
        return;
    }

    let token = match env::var("GITLAB_TELOXIDE_TOKEN") {
        Ok(t) => t,
        Err(_) => {
            tracing::error!("Cannot send startup alert: GITLAB_TELOXIDE_TOKEN not set");
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
