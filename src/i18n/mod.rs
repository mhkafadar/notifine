use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;
use teloxide::types::Message;

// Global static for translations using OnceLock for thread-safe lazy initialization
static TRANSLATIONS: OnceLock<HashMap<String, Value>> = OnceLock::new();

fn load_translations() -> HashMap<String, Value> {
    let mut translations = HashMap::new();

    // Load English translations
    let en_json = include_str!("en.json");
    if let Ok(en_value) = serde_json::from_str(en_json) {
        translations.insert("en".to_string(), en_value);
    } else {
        log::error!("Failed to parse en.json");
    }

    // Load Turkish translations
    let tr_json = include_str!("tr.json");
    if let Ok(tr_value) = serde_json::from_str(tr_json) {
        translations.insert("tr".to_string(), tr_value);
    } else {
        log::error!("Failed to parse tr.json");
    }

    translations
}

fn get_nested_value<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = value;

    for part in parts {
        match current {
            Value::Object(map) => {
                current = map.get(part)?;
            }
            _ => return None,
        }
    }

    Some(current)
}

pub struct I18n {
    // This struct is kept for compatibility but now uses the JSON-based approach
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

impl I18n {
    pub fn new() -> Self {
        Self {}
    }

    pub fn detect_language(message: &Message) -> String {
        // Check if user has language preference from Telegram
        if let Some(user) = message.from() {
            if let Some(lang_code) = &user.language_code {
                // Support Turkish and English
                match lang_code.as_str() {
                    "tr" | "tr-TR" => return "tr".to_string(),
                    "en" | "en-US" | "en-GB" => return "en".to_string(),
                    _ => {}
                }
            }
        }

        // Fallback to text analysis if Telegram language is not available
        if let Some(text) = message.text() {
            Self::detect_language_from_text(text)
        } else {
            "en".to_string() // Default to English
        }
    }

    pub fn detect_language_from_text(text: &str) -> String {
        let turkish_words = [
            "merhaba",
            "selam",
            "nasılsın",
            "teşekkür",
            "ederim",
            "günaydın",
            "iyi",
            "akşamlar",
            "geceler",
            "hoşça",
            "kal",
            "görüşürüz",
            "evet",
            "hayır",
            "tamam",
            "peki",
            "lütfen",
            "özür",
            "dilerim",
            "araba",
            "sipariş",
            "durum",
            "teslimat",
            "araç",
            "giriş",
            "çıkış",
            "yap",
            "kontrol",
            "et",
            "olan",
            "var",
            "yok",
            "olan",
            "şu",
            "bu",
            "o",
            "ben",
            "sen",
            "biz",
            "siz",
            "onlar",
            "için",
            "ile",
            "ve",
            "veya",
            "ama",
            "fakat",
            "çünkü",
            "ki",
        ];

        let text_lower = text.to_lowercase();
        let turkish_count = turkish_words
            .iter()
            .filter(|&word| text_lower.contains(word))
            .count();

        // If we find 2 or more Turkish words, consider it Turkish
        if turkish_count >= 2 {
            "tr".to_string()
        } else {
            "en".to_string()
        }
    }

    pub fn get_user_language(&self, chat_id: i64) -> String {
        // Try to get language from database
        use crate::find_chat_by_telegram_chat_id;

        if let Some(chat) = find_chat_by_telegram_chat_id(&chat_id.to_string()) {
            return chat.language;
        }

        // Default to English if no chat found
        "en".to_string()
    }

    pub fn save_user_language(&self, chat_id: i64, language: &str) {
        use crate::{establish_connection, find_chat_by_telegram_chat_id, schema::chats};
        use diesel::prelude::*;

        if let Some(chat) = find_chat_by_telegram_chat_id(&chat_id.to_string()) {
            let mut conn = establish_connection();

            diesel::update(chats::table.filter(chats::id.eq(chat.id)))
                .set(chats::language.eq(language))
                .execute(&mut conn)
                .ok();
        }
    }
}

// Global instance
lazy_static::lazy_static! {
    pub static ref I18N: I18n = I18n::new();
}

// Main translation function using JSON traversal
pub fn t(language: &str, path: &str) -> String {
    let translations = TRANSLATIONS.get_or_init(load_translations);

    // Get the translation for the specified language
    if let Some(translation) = translations
        .get(language)
        .or_else(|| translations.get("en"))
        .and_then(|value| get_nested_value(value, path))
        .and_then(|value| value.as_str())
    {
        translation.to_string()
    } else {
        format!("Message not found: {}", path)
    }
}

// Translation function with arguments support
pub fn t_with_args(language: &str, path: &str, args: &[&str]) -> String {
    let mut message = t(language, path);

    // Replace {} placeholders with arguments
    for arg in args {
        if message.contains("{}") {
            message = message.replacen("{}", arg, 1);
        }
    }

    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_message() {
        let msg = t("en", "common.success");
        assert!(msg.contains("Success"));

        let msg_tr = t("tr", "common.success");
        assert!(msg_tr.contains("Başarılı"));
    }

    #[test]
    fn test_get_message_with_args() {
        let msg = t_with_args("en", "common.error", &["test error"]);
        assert!(msg.contains("test error"));
    }

    #[test]
    fn test_missing_message() {
        let msg = t("en", "nonexistent.key");
        assert!(msg.contains("Message not found"));
    }

    #[test]
    fn test_language_fallback() {
        let msg = t("de", "common.success");
        assert!(msg.contains("Success"));
    }
}
