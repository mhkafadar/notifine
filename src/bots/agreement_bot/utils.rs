use crate::bots::bot_service::TelegramMessage;
use crate::observability::METRICS;
use notifine::db::DbPool;
use notifine::find_agreement_user_by_telegram_id;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, ParseMode};

use super::types::DEFAULT_LANGUAGE;

pub fn get_user_language(pool: &DbPool, user_id: i64) -> String {
    match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(user)) => user.language,
        _ => DEFAULT_LANGUAGE.to_string(),
    }
}

pub fn detect_language_from_message(msg: &Message) -> String {
    if let Some(user) = msg.from() {
        if let Some(lang_code) = &user.language_code {
            match lang_code.as_str() {
                "tr" | "tr-TR" => return "tr".to_string(),
                "en" | "en-US" | "en-GB" => return "en".to_string(),
                _ => {}
            }
        }
    }
    DEFAULT_LANGUAGE.to_string()
}

pub async fn send_telegram_message(bot: &Bot, message: TelegramMessage) -> ResponseResult<()> {
    let TelegramMessage {
        chat_id,
        thread_id,
        message,
    } = message;

    let chat_id = ChatId(chat_id);

    let mut request = bot
        .send_message(chat_id, &message)
        .parse_mode(ParseMode::Html);

    if let Some(tid) = thread_id {
        request = request.message_thread_id(tid);
    }

    request.await?;

    METRICS.increment_messages_sent_for_bot("agreement");

    Ok(())
}

pub async fn send_message_with_keyboard(
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    message: &str,
    keyboard: InlineKeyboardMarkup,
) -> ResponseResult<()> {
    let mut request = bot
        .send_message(ChatId(chat_id), message)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard);

    if let Some(tid) = thread_id {
        request = request.message_thread_id(tid);
    }

    request.await?;

    METRICS.increment_messages_sent_for_bot("agreement");

    Ok(())
}
