use teloxide::prelude::*;
use teloxide::types::Message;

use notifine::db::DbPool;

use super::db::{
    approve_all_deactivations, create_broadcast_job, get_all_broadcast_targets, get_job_by_id,
    get_pending_deactivations, get_recent_jobs, reject_all_deactivations, update_job_status,
};
use super::types::BroadcastStatus;

const DEFAULT_RATE_LIMIT: i32 = 10;

pub async fn handle_broadcast(
    bot: &Bot,
    msg: &Message,
    pool: &DbPool,
    admin_chat_id: Option<i64>,
) -> ResponseResult<()> {
    if !is_admin(msg, admin_chat_id) {
        bot.send_message(
            msg.chat.id,
            "This command is only available to administrators.",
        )
        .await?;
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let args: Vec<&str> = text.split_whitespace().skip(1).collect();

    let discovery_mode = args
        .first()
        .map(|&s| {
            let cleaned = s.trim_start_matches(['-', '—', '–']);
            cleaned.eq_ignore_ascii_case("discover")
        })
        .unwrap_or(false);

    let message_start = if discovery_mode { 2 } else { 1 };
    let broadcast_message: String = text
        .split_whitespace()
        .skip(message_start)
        .collect::<Vec<&str>>()
        .join(" ");

    if broadcast_message.is_empty() {
        bot.send_message(
            msg.chat.id,
            "Usage: /broadcast [--discover] <message>\n\n\
            --discover: Try all bots to discover which ones can reach each chat\n\n\
            Example:\n\
            /broadcast Hello everyone!\n\
            /broadcast --discover Hello everyone!",
        )
        .await?;
        return Ok(());
    }

    match create_broadcast_job(
        pool,
        &broadcast_message,
        msg.chat.id.0,
        DEFAULT_RATE_LIMIT,
        discovery_mode,
    ) {
        Ok(job) => {
            let mode_str = if discovery_mode {
                "Discovery"
            } else {
                "Normal"
            };
            bot.send_message(
                msg.chat.id,
                format!(
                    "📢 Broadcast job #{} created!\n\
                    Mode: {}\n\
                    Message: {}\n\
                    Status: Pending\n\
                    Rate: {} msg/sec\n\n\
                    The worker will pick it up shortly.\n\
                    Use /broadcaststatus to check progress.",
                    job.id,
                    mode_str,
                    truncate(&broadcast_message, 100),
                    job.rate_limit_per_sec
                ),
            )
            .await?;
        }
        Err(e) => {
            tracing::error!("Failed to create broadcast job: {:?}", e);
            bot.send_message(msg.chat.id, "Failed to create broadcast job.")
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_broadcast_test(
    bot: &Bot,
    msg: &Message,
    pool: &DbPool,
    admin_chat_id: Option<i64>,
) -> ResponseResult<()> {
    if !is_admin(msg, admin_chat_id) {
        bot.send_message(
            msg.chat.id,
            "This command is only available to administrators.",
        )
        .await?;
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let args: Vec<&str> = text.split_whitespace().skip(1).collect();

    let discovery_mode = args
        .first()
        .map(|&s| {
            let cleaned = s.trim_start_matches(['-', '—', '–']);
            cleaned.eq_ignore_ascii_case("discover")
        })
        .unwrap_or(false);

    let message_start = if discovery_mode { 2 } else { 1 };
    let broadcast_message: String = text
        .split_whitespace()
        .skip(message_start)
        .collect::<Vec<&str>>()
        .join(" ");

    if broadcast_message.is_empty() {
        bot.send_message(
            msg.chat.id,
            "Usage: /broadcasttest [--discover] <message>\n\n\
            This is a dry run - no messages will be sent.\n\
            Shows target count and message preview.",
        )
        .await?;
        return Ok(());
    }

    let targets = match get_all_broadcast_targets(pool) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Failed to get broadcast targets: {:?}", e);
            bot.send_message(msg.chat.id, "Failed to get target count.")
                .await?;
            return Ok(());
        }
    };

    let mode_str = if discovery_mode {
        "Discovery (try all bots)"
    } else {
        "Normal (stop at first success)"
    };

    let estimated_time = targets.len() as f64 / DEFAULT_RATE_LIMIT as f64;
    let time_str = if estimated_time < 60.0 {
        format!("{:.0} seconds", estimated_time)
    } else {
        format!("{:.1} minutes", estimated_time / 60.0)
    };

    bot.send_message(
        msg.chat.id,
        format!(
            "🧪 <b>BROADCAST TEST (Dry Run)</b>\n\n\
            <b>Mode:</b> {}\n\
            <b>Target chats:</b> {}\n\
            <b>Rate:</b> {} msg/sec\n\
            <b>Estimated time:</b> {}\n\n\
            <b>Message preview:</b>\n{}\n\n\
            ⚠️ No messages were sent. Use /broadcast to send for real.",
            mode_str,
            targets.len(),
            DEFAULT_RATE_LIMIT,
            time_str,
            broadcast_message
        ),
    )
    .parse_mode(teloxide::types::ParseMode::Html)
    .await?;

    Ok(())
}

pub async fn handle_broadcast_status(
    bot: &Bot,
    msg: &Message,
    pool: &DbPool,
    admin_chat_id: Option<i64>,
) -> ResponseResult<()> {
    if !is_admin(msg, admin_chat_id) {
        bot.send_message(
            msg.chat.id,
            "This command is only available to administrators.",
        )
        .await?;
        return Ok(());
    }

    match get_recent_jobs(pool, 5) {
        Ok(jobs) => {
            if jobs.is_empty() {
                bot.send_message(msg.chat.id, "No broadcast jobs found.")
                    .await?;
                return Ok(());
            }

            let mut response = String::from("📊 Recent Broadcast Jobs:\n\n");

            for job in jobs {
                let progress_pct = if job.total_chats > 0 {
                    (job.processed_count as f64 / job.total_chats as f64 * 100.0) as u32
                } else {
                    0
                };

                let status_emoji = match job.status.as_str() {
                    "pending" => "⏳",
                    "running" => "🔄",
                    "completed" => "✅",
                    "cancelled" => "🚫",
                    "failed" => "❌",
                    "paused" => "⏸️",
                    _ => "❓",
                };

                let mode_indicator = if job.discovery_mode { " 🔍" } else { "" };
                response.push_str(&format!(
                    "{} Job #{}{} - {}\n\
                    Progress: {}/{} ({}%)\n\
                    ✅ {} | ❌ {} | 🚫 {}\n\
                    Message: {}\n\n",
                    status_emoji,
                    job.id,
                    mode_indicator,
                    job.status,
                    job.processed_count,
                    job.total_chats,
                    progress_pct,
                    job.success_count,
                    job.failed_count,
                    job.unreachable_count,
                    truncate(&job.message, 50)
                ));
            }

            bot.send_message(msg.chat.id, response).await?;
        }
        Err(e) => {
            tracing::error!("Failed to get jobs: {:?}", e);
            bot.send_message(msg.chat.id, "Failed to retrieve jobs.")
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_broadcast_cancel(
    bot: &Bot,
    msg: &Message,
    pool: &DbPool,
    admin_chat_id: Option<i64>,
) -> ResponseResult<()> {
    if !is_admin(msg, admin_chat_id) {
        bot.send_message(
            msg.chat.id,
            "This command is only available to administrators.",
        )
        .await?;
        return Ok(());
    }

    let job_id: Option<i32> = msg
        .text()
        .and_then(|text| text.split_once(' '))
        .and_then(|(_, id)| id.trim().parse().ok());

    let job_id = match job_id {
        Some(id) => id,
        None => {
            bot.send_message(
                msg.chat.id,
                "Usage: /broadcastcancel <job_id>\nExample: /broadcastcancel 5",
            )
            .await?;
            return Ok(());
        }
    };

    match get_job_by_id(pool, job_id) {
        Ok(Some(job)) => {
            if job.status == "completed" || job.status == "cancelled" || job.status == "failed" {
                bot.send_message(
                    msg.chat.id,
                    format!("Job #{} is already {}.", job_id, job.status),
                )
                .await?;
                return Ok(());
            }

            match update_job_status(pool, job_id, BroadcastStatus::Cancelled) {
                Ok(_) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("🚫 Broadcast job #{} has been cancelled.", job_id),
                    )
                    .await?;
                }
                Err(e) => {
                    tracing::error!("Failed to cancel job: {:?}", e);
                    bot.send_message(msg.chat.id, "Failed to cancel job.")
                        .await?;
                }
            }
        }
        Ok(None) => {
            bot.send_message(msg.chat.id, format!("Job #{} not found.", job_id))
                .await?;
        }
        Err(e) => {
            tracing::error!("Failed to get job: {:?}", e);
            bot.send_message(msg.chat.id, "Failed to retrieve job.")
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_pending_list(
    bot: &Bot,
    msg: &Message,
    pool: &DbPool,
    admin_chat_id: Option<i64>,
) -> ResponseResult<()> {
    if !is_admin(msg, admin_chat_id) {
        bot.send_message(
            msg.chat.id,
            "This command is only available to administrators.",
        )
        .await?;
        return Ok(());
    }

    match get_pending_deactivations(pool) {
        Ok(pending) => {
            if pending.is_empty() {
                bot.send_message(msg.chat.id, "No pending deactivations.")
                    .await?;
                return Ok(());
            }

            let mut response = format!("🚫 Pending Deactivations: {} chats\n\n", pending.len());

            for (i, p) in pending.iter().take(20).enumerate() {
                let bots: Vec<String> = p.failed_bots.iter().filter_map(|b| b.clone()).collect();

                response.push_str(&format!(
                    "{}. Chat: {}\n   Tried: {}\n   Error: {}\n\n",
                    i + 1,
                    p.telegram_chat_id,
                    bots.join(", "),
                    p.last_error.as_deref().unwrap_or("N/A")
                ));
            }

            if pending.len() > 20 {
                response.push_str(&format!("... and {} more\n\n", pending.len() - 20));
            }

            response.push_str("Use /approveall to deactivate these chats.\nUse /rejectall to clear this list without deactivating.");

            bot.send_message(msg.chat.id, response).await?;
        }
        Err(e) => {
            tracing::error!("Failed to get pending deactivations: {:?}", e);
            bot.send_message(msg.chat.id, "Failed to retrieve pending deactivations.")
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_approve_all(
    bot: &Bot,
    msg: &Message,
    pool: &DbPool,
    admin_chat_id: Option<i64>,
) -> ResponseResult<()> {
    if !is_admin(msg, admin_chat_id) {
        bot.send_message(
            msg.chat.id,
            "This command is only available to administrators.",
        )
        .await?;
        return Ok(());
    }

    match approve_all_deactivations(pool, msg.chat.id.0) {
        Ok(count) => {
            bot.send_message(
                msg.chat.id,
                format!(
                    "✅ Approved and deactivated {} chats.\n\
                    These chats will now receive HTTP 400 on webhook requests.",
                    count
                ),
            )
            .await?;
        }
        Err(e) => {
            tracing::error!("Failed to approve deactivations: {:?}", e);
            bot.send_message(msg.chat.id, "Failed to approve deactivations.")
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_reject_all(
    bot: &Bot,
    msg: &Message,
    pool: &DbPool,
    admin_chat_id: Option<i64>,
) -> ResponseResult<()> {
    if !is_admin(msg, admin_chat_id) {
        bot.send_message(
            msg.chat.id,
            "This command is only available to administrators.",
        )
        .await?;
        return Ok(());
    }

    match reject_all_deactivations(pool, msg.chat.id.0) {
        Ok(count) => {
            bot.send_message(
                msg.chat.id,
                format!(
                    "🚫 Rejected {} pending deactivations.\n\
                    These chats remain active.",
                    count
                ),
            )
            .await?;
        }
        Err(e) => {
            tracing::error!("Failed to reject deactivations: {:?}", e);
            bot.send_message(msg.chat.id, "Failed to reject deactivations.")
                .await?;
        }
    }

    Ok(())
}

fn is_admin(msg: &Message, admin_chat_id: Option<i64>) -> bool {
    admin_chat_id == Some(msg.chat.id.0)
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_len).collect::<String>())
    }
}
