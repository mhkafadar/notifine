use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;
use teloxide::types::Message;

use crate::db::PgPool;
use crate::find_chat_by_telegram_chat_id;

static TRANSLATIONS: OnceLock<HashMap<String, Value>> = OnceLock::new();

fn load_translations() -> HashMap<String, Value> {
    let mut translations = HashMap::new();

    let en_json = include_str!("en.json");
    if let Ok(en_value) = serde_json::from_str(en_json) {
        translations.insert("en".to_string(), en_value);
    } else {
        tracing::error!("Failed to parse en.json");
    }

    let tr_json = include_str!("tr.json");
    if let Ok(tr_value) = serde_json::from_str(tr_json) {
        translations.insert("tr".to_string(), tr_value);
    } else {
        tracing::error!("Failed to parse tr.json");
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

pub struct I18n {}

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
        if let Some(user) = message.from() {
            if let Some(lang_code) = &user.language_code {
                match lang_code.as_str() {
                    "tr" | "tr-TR" => return "tr".to_string(),
                    "en" | "en-US" | "en-GB" => return "en".to_string(),
                    _ => {}
                }
            }
        }

        if let Some(text) = message.text() {
            Self::detect_language_from_text(text)
        } else {
            "en".to_string()
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

        if turkish_count >= 2 {
            "tr".to_string()
        } else {
            "en".to_string()
        }
    }

    pub fn get_user_language(&self, pool: &PgPool, chat_id: i64) -> String {
        match find_chat_by_telegram_chat_id(pool, &chat_id.to_string()) {
            Ok(Some(chat)) => chat.language,
            Ok(None) => "en".to_string(),
            Err(e) => {
                tracing::error!("Failed to get user language: {:?}", e);
                "en".to_string()
            }
        }
    }

    pub fn save_user_language(&self, pool: &PgPool, chat_id: i64, language: &str) {
        use crate::schema::chats;
        use diesel::prelude::*;

        match find_chat_by_telegram_chat_id(pool, &chat_id.to_string()) {
            Ok(Some(chat)) => {
                if let Ok(mut conn) = pool.get() {
                    diesel::update(chats::table.filter(chats::id.eq(chat.id)))
                        .set(chats::language.eq(language))
                        .execute(&mut conn)
                        .ok();
                }
            }
            Ok(None) => {
                tracing::warn!("No chat found for chat_id: {}", chat_id);
            }
            Err(e) => {
                tracing::error!("Database error in save_user_language: {:?}", e);
            }
        }
    }
}

lazy_static::lazy_static! {
    pub static ref I18N: I18n = I18n::new();
}

pub fn t(language: &str, path: &str) -> String {
    let translations = TRANSLATIONS.get_or_init(load_translations);

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

pub fn t_with_args(language: &str, path: &str, args: &[&str]) -> String {
    let mut message = t(language, path);

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
