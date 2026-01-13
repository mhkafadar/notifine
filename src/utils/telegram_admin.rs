use std::env;
use teloxide::prelude::*;
use teloxide::{Bot, RequestError};

pub async fn send_message_to_admin(
    bot: &Bot,
    message: String,
    level: u8,
) -> Result<(), RequestError> {
    let admin_logs = env::var("ADMIN_LOGS").unwrap_or_default();
    if admin_logs != "ACTIVE" {
        return Ok(());
    }

    let admin_chat_id: i64 = match env::var("TELEGRAM_ADMIN_CHAT_ID")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        Some(id) => id,
        None => {
            tracing::warn!("TELEGRAM_ADMIN_CHAT_ID not set or invalid, skipping admin message");
            return Ok(());
        }
    };

    let log_level_threshold: u8 = env::var("ADMIN_LOG_LEVEL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);

    if level <= log_level_threshold {
        bot.send_message(ChatId(admin_chat_id), message).await?;
    }

    Ok(())
}
