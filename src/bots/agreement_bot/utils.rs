use crate::bots::bot_service::TelegramMessage;
use crate::observability::METRICS;
use chrono::NaiveDate;
use notifine::db::DbPool;
use notifine::find_agreement_user_by_telegram_id;
use notifine::i18n::{t, t_with_args};
use notifine::models::{Agreement, Reminder};
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, ParseMode};

use super::types::DEFAULT_LANGUAGE;

pub fn get_user_language(pool: &DbPool, user_id: i64) -> String {
    match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(user)) => user.language,
        _ => DEFAULT_LANGUAGE.to_string(),
    }
}

pub fn detect_language_from_telegram(msg: &Message) -> Option<String> {
    if let Some(user) = msg.from() {
        if let Some(lang_code) = &user.language_code {
            let code = lang_code.to_lowercase();
            if code == "tr" || code.starts_with("tr-") {
                return Some("tr".to_string());
            }
            if code == "en" || code.starts_with("en-") {
                return Some("en".to_string());
            }
        }
    }
    None
}

pub fn detect_language_from_text(text: &str) -> Option<String> {
    const TURKISH_CHARS: &[char] = &['ç', 'ğ', 'ı', 'İ', 'ö', 'ş', 'ü', 'Ç', 'Ğ', 'Ö', 'Ş', 'Ü'];
    if text.chars().any(|c| TURKISH_CHARS.contains(&c)) {
        return Some("tr".to_string());
    }
    None
}

pub fn detect_language(msg: &Message) -> String {
    if let Some(lang) = detect_language_from_telegram(msg) {
        return lang;
    }
    if let Some(text) = msg.text() {
        if let Some(lang) = detect_language_from_text(text) {
            return lang;
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

pub async fn confirm_selection_and_send_next(
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    message_id: teloxide::types::MessageId,
    selection_text: &str,
    next_message: &str,
    next_keyboard: InlineKeyboardMarkup,
) -> ResponseResult<()> {
    bot.edit_message_text(ChatId(chat_id), message_id, selection_text)
        .await?;

    send_message_with_keyboard(bot, chat_id, thread_id, next_message, next_keyboard).await?;

    Ok(())
}

pub fn generate_reminder_title(
    reminder: &Reminder,
    agreement: &Agreement,
    language: &str,
) -> String {
    if !reminder.title.is_empty() {
        return reminder.title.clone();
    }

    let is_landlord = agreement.user_role.as_deref() == Some("landlord");

    match reminder.reminder_type.as_str() {
        "due_day" | "pre_notify" => {
            if is_landlord {
                t(language, "agreement.rent.success.collection_title")
            } else {
                t(language, "agreement.rent.success.payment_title")
            }
        }
        "yearly_increase" => {
            if is_landlord {
                t(language, "agreement.rent.yearly_increase.landlord_title")
            } else {
                t(language, "agreement.rent.yearly_increase.tenant_title")
            }
        }
        "five_year_notice" => {
            let years = calculate_years_since(agreement.start_date, reminder.due_date);
            if is_landlord {
                t_with_args(
                    language,
                    "agreement.rent.five_year.landlord_title",
                    &[&years.to_string()],
                )
            } else {
                t_with_args(
                    language,
                    "agreement.rent.five_year.tenant_title",
                    &[&years.to_string()],
                )
            }
        }
        "ten_year_notice" => {
            if is_landlord {
                t(language, "agreement.rent.ten_year.landlord_6_months_title")
            } else {
                t(language, "agreement.rent.ten_year.tenant_6_months_title")
            }
        }
        _ => reminder.title.clone(),
    }
}

fn calculate_years_since(start_date: Option<NaiveDate>, due_date: NaiveDate) -> u32 {
    match start_date {
        Some(start) => due_date.years_since(start).unwrap_or(0),
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_from_text_turkish_chars() {
        assert_eq!(detect_language_from_text("merhaba"), None);
        assert_eq!(
            detect_language_from_text("günaydın"),
            Some("tr".to_string())
        );
        assert_eq!(
            detect_language_from_text("çok güzel"),
            Some("tr".to_string())
        );
        assert_eq!(
            detect_language_from_text("nasılsın"),
            Some("tr".to_string())
        );
        assert_eq!(
            detect_language_from_text("İstanbul"),
            Some("tr".to_string())
        );
        assert_eq!(detect_language_from_text("TÜRKÇE"), Some("tr".to_string()));
        assert_eq!(detect_language_from_text("hello world"), None);
        assert_eq!(detect_language_from_text("bonjour"), None);
    }

    #[test]
    fn test_detect_language_from_text_empty() {
        assert_eq!(detect_language_from_text(""), None);
        assert_eq!(detect_language_from_text("   "), None);
    }

    #[test]
    fn test_detect_language_from_text_mixed() {
        assert_eq!(
            detect_language_from_text("hello dünya"),
            Some("tr".to_string())
        );
        assert_eq!(
            detect_language_from_text("123 öğrenci"),
            Some("tr".to_string())
        );
    }
}
