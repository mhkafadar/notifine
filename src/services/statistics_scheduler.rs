use crate::observability::METRICS;
use crate::services::stats::get_todays_chat_events;
use crate::utils::telegram_admin::send_message_to_admin;
use chrono::{Duration, Utc};
use chrono_tz::Europe::Istanbul;
use notifine::db::DbPool;
use notifine::models::ChatEvent;
use std::collections::HashMap;
use std::env;
use teloxide::Bot;

pub async fn run_statistics_scheduler(pool: DbPool) {
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

        send_daily_report(&bot, &pool).await;
    }
}

async fn send_daily_report(bot: &Bot, pool: &DbPool) {
    let snapshot = METRICS.snapshot_and_reset();
    tracing::info!("Daily metrics counters reset");

    let total_webhooks =
        snapshot.github_webhooks + snapshot.gitlab_webhooks + snapshot.beep_webhooks;

    let today = Utc::now().with_timezone(&Istanbul).date_naive();
    let events = match get_todays_chat_events(pool, today) {
        Ok(events) => events,
        Err(e) => {
            tracing::error!("Failed to fetch chat events for daily report: {:?}", e);
            vec![]
        }
    };

    let chat_insights = build_chat_insights(&events);

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
        {}\
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
        chat_insights,
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

fn build_chat_insights(events: &[ChatEvent]) -> String {
    let new_events: Vec<&ChatEvent> = events.iter().filter(|e| e.event_type == "new").collect();
    let churn_events: Vec<&ChatEvent> = events.iter().filter(|e| e.event_type == "churn").collect();

    let mut result = String::new();

    if new_events.is_empty() && churn_events.is_empty() {
        result.push_str("<b>Chats:</b>\n• New: 0\n• Churned: 0\n\n");
        return result;
    }

    if !new_events.is_empty() {
        let mut by_bot: HashMap<&str, Vec<&ChatEvent>> = HashMap::new();
        for event in &new_events {
            by_bot.entry(&event.bot_type).or_default().push(event);
        }

        result.push_str(&format!("👥 <b>New Chats ({}):</b>\n", new_events.len()));

        let mut bot_types: Vec<&&str> = by_bot.keys().collect();
        bot_types.sort();

        for (i, bot_type) in bot_types.iter().enumerate() {
            let events = &by_bot[*bot_type];
            let is_last = i == bot_types.len() - 1;
            let prefix = if is_last { "└─" } else { "├─" };
            let line_prefix = if is_last { "   " } else { "│  " };

            result.push_str(&format!("{} {}: {}\n", prefix, bot_type, events.len()));

            for event in events {
                let username = event.inviter_username.as_deref().unwrap_or("unknown");
                let status = if event.is_cross_bot_user {
                    format!("cross-bot: {}", event.other_bots.as_deref().unwrap_or(""))
                } else {
                    "newcomer".to_string()
                };
                result.push_str(&format!("{}• @{} ({})\n", line_prefix, username, status));
            }
        }
        result.push('\n');
    } else {
        result.push_str("👥 <b>New Chats (0)</b>\n\n");
    }

    if !churn_events.is_empty() {
        let mut by_bot: HashMap<&str, Vec<&ChatEvent>> = HashMap::new();
        for event in &churn_events {
            by_bot.entry(&event.bot_type).or_default().push(event);
        }

        result.push_str(&format!("🚪 <b>Churned ({}):</b>\n", churn_events.len()));

        let mut bot_types: Vec<&&str> = by_bot.keys().collect();
        bot_types.sort();

        for (i, bot_type) in bot_types.iter().enumerate() {
            let events = &by_bot[*bot_type];
            let is_last = i == bot_types.len() - 1;
            let prefix = if is_last { "└─" } else { "├─" };
            let line_prefix = if is_last { "   " } else { "│  " };

            result.push_str(&format!("{} {}: {}\n", prefix, bot_type, events.len()));

            for event in events {
                let status = if event.is_cross_bot_user {
                    format!("still has: {}", event.other_bots.as_deref().unwrap_or(""))
                } else {
                    "fully churned".to_string()
                };
                result.push_str(&format!(
                    "{}• chat:{} ({})\n",
                    line_prefix, event.telegram_chat_id, status
                ));
            }
        }
        result.push('\n');
    } else {
        result.push_str("🚪 <b>Churned (0)</b>\n\n");
    }

    let new_cross_bot = new_events.iter().filter(|e| e.is_cross_bot_user).count();
    let churn_still_has = churn_events.iter().filter(|e| e.is_cross_bot_user).count();

    if !new_events.is_empty() || !churn_events.is_empty() {
        result.push_str("📈 <b>Cross-Bot Stats:</b>\n");
        if !new_events.is_empty() {
            let pct = (new_cross_bot as f64 / new_events.len() as f64 * 100.0) as i32;
            result.push_str(&format!(
                "• {}/{} new chats already used other bots ({}%)\n",
                new_cross_bot,
                new_events.len(),
                pct
            ));
        }
        if !churn_events.is_empty() {
            result.push_str(&format!(
                "• {}/{} churned chats still have other bots\n",
                churn_still_has,
                churn_events.len()
            ));
        }
        result.push('\n');
    }

    result
}
