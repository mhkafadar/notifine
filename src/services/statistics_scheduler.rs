use crate::observability::METRICS;
use crate::utils::telegram_admin::send_message_to_admin;
use chrono::{Duration, Utc};
use chrono_tz::Europe::Istanbul;
use std::env;
use teloxide::Bot;

pub async fn run_statistics_scheduler() {
    tracing::info!("Starting statistics scheduler...");

    let token = match env::var("GITLAB_TELOXIDE_TOKEN") {
        Ok(t) => t,
        Err(_) => {
            tracing::warn!("GITLAB_TELOXIDE_TOKEN not set, statistics scheduler disabled");
            return;
        }
    };

    let bot = Bot::new(token);

    loop {
        let now = Utc::now().with_timezone(&Istanbul);

        let tomorrow_midnight = match (now + Duration::days(1)).date_naive().and_hms_opt(0, 0, 0) {
            Some(time) => time,
            None => {
                tracing::error!("Failed to create midnight time, retrying in 1 hour");
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                continue;
            }
        };

        let duration_until_midnight =
            match tomorrow_midnight.and_local_timezone(Istanbul).earliest() {
                Some(midnight_tz) => {
                    let duration = midnight_tz - now;
                    duration
                        .to_std()
                        .unwrap_or(std::time::Duration::from_secs(86400))
                }
                None => {
                    tracing::warn!("DST transition detected, using 24 hour fallback");
                    std::time::Duration::from_secs(86400)
                }
            };

        tracing::info!(
            "Statistics scheduler: Next report in {} hours {} minutes",
            duration_until_midnight.as_secs() / 3600,
            (duration_until_midnight.as_secs() % 3600) / 60
        );

        tokio::time::sleep(duration_until_midnight).await;

        send_daily_report(&bot).await;
    }
}

async fn send_daily_report(bot: &Bot) {
    let snapshot = METRICS.snapshot_and_reset();
    tracing::info!("Daily metrics counters reset");

    let total_webhooks =
        snapshot.github_webhooks + snapshot.gitlab_webhooks + snapshot.beep_webhooks;

    let report = format!(
        "📊 <b>Daily Stats Report</b>\n\n\
        <b>Webhooks Received:</b>\n\
        • GitHub: {}\n\
        • GitLab: {}\n\
        • Beep: {}\n\
        Total: {}\n\n\
        <b>Messages Sent:</b>\n\
        • GitHub: {}\n\
        • GitLab: {}\n\
        • Beep: {}\n\
        • Uptime: {}\n\
        • Agreement: {}\n\
        Total: {}\n\n\
        <b>Chats:</b>\n\
        • New: {}\n\
        • Churned: {}\n\n\
        <b>Uptime Checks:</b>\n\
        • Total: {}\n\
        • Failures: {}\n\n\
        <b>Errors:</b> {}",
        snapshot.github_webhooks,
        snapshot.gitlab_webhooks,
        snapshot.beep_webhooks,
        total_webhooks,
        snapshot.github_messages_sent,
        snapshot.gitlab_messages_sent,
        snapshot.beep_messages_sent,
        snapshot.uptime_messages_sent,
        snapshot.agreement_messages_sent,
        snapshot.messages_sent,
        snapshot.new_chats,
        snapshot.churned_chats,
        snapshot.uptime_checks,
        snapshot.uptime_failures,
        snapshot.errors
    );

    if let Err(e) = send_message_to_admin(bot, report, 1).await {
        tracing::error!("Failed to send daily statistics report: {:?}", e);
        METRICS.increment_errors();
    } else {
        tracing::info!("Daily statistics report sent successfully");
    }
}
