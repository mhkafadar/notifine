use std::env;
use teloxide::prelude::*;
use teloxide::{Bot, RequestError};

pub async fn send_message_to_admin(
    bot: &Bot,
    message: String,
    level: u8,
) -> Result<(), RequestError> {
    if env::var("ADMIN_LOGS").expect("ADMIN_LOGS must be set") != "ACTIVE" {
        return Ok(());
    }

    let admin_chat_id: i64 = env::var("TELEGRAM_ADMIN_CHAT_ID")
        .expect("TELEGRAM_ADMIN_CHAT_ID must be set")
        .parse::<i64>()
        .expect("Error parsing TELEGRAM_ADMIN_CHAT_ID");

    let log_level_threshold: u8 = env::var("ADMIN_LOG_LEVEL")
        .expect("ADMIN_LOG_LEVEL must be set")
        .parse::<u8>()
        .expect("Error parsing ADMIN_LOG_LEVEL");

    // 50 is the most verbose log level 0 is the least verbose
    if level <= log_level_threshold {
        bot.send_message(ChatId(admin_chat_id), message).await?;
    }

    Ok(())
}
