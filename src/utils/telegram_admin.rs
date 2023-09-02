use std::env;
use teloxide::prelude::*;
use teloxide::{Bot, RequestError};

pub async fn send_message_to_admin(bot: Bot, message: String) -> Result<(), RequestError> {
    if env::var("ADMIN_LOGS").expect("ADMIN_LOGS must be set") != "ACTIVE" {
        return Ok(());
    }

    let admin_chat_id: i64 = env::var("TELEGRAM_ADMIN_CHAT_ID")
        .expect("TELEGRAM_ADMIN_CHAT_ID must be set")
        .parse::<i64>()
        .expect("Error parsing TELEGRAM_ADMIN_CHAT_ID");

    bot.send_message(ChatId(admin_chat_id), message).await?;

    Ok(())
}
