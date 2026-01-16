use super::alerts::Severity;
use super::{ALERTS, METRICS};
use teloxide::prelude::*;
use teloxide::RequestError;

#[derive(Debug, Clone, PartialEq)]
pub enum TelegramErrorKind {
    RateLimited,
    BotBlocked,
    ChatNotFound,
    GroupMigrated { new_chat_id: i64 },
    NotEnoughRights,
    NetworkError,
    Other,
}

pub fn classify_telegram_error(error: &RequestError) -> TelegramErrorKind {
    match error {
        RequestError::RetryAfter(_) => TelegramErrorKind::RateLimited,
        RequestError::MigrateToChatId(new_id) => TelegramErrorKind::GroupMigrated {
            new_chat_id: *new_id,
        },
        RequestError::Api(api_error) => {
            let error_str = api_error.to_string();
            let error_lower = error_str.to_lowercase();

            if error_lower.contains("blocked") || error_lower.contains("bot was blocked") {
                TelegramErrorKind::BotBlocked
            } else if error_lower.contains("chat not found")
                || error_lower.contains("chat_not_found")
                || error_lower.contains("user not found")
            {
                TelegramErrorKind::ChatNotFound
            } else if error_lower.contains("migrated") || error_lower.contains("migrate_to_chat_id")
            {
                if let Some(new_id) = extract_migrated_chat_id(&error_str) {
                    TelegramErrorKind::GroupMigrated {
                        new_chat_id: new_id,
                    }
                } else {
                    TelegramErrorKind::Other
                }
            } else if error_lower.contains("not enough rights")
                || error_lower.contains("have no rights")
                || error_lower.contains("need administrator rights")
            {
                TelegramErrorKind::NotEnoughRights
            } else {
                TelegramErrorKind::Other
            }
        }
        RequestError::Network(_) => TelegramErrorKind::NetworkError,
        _ => TelegramErrorKind::Other,
    }
}

pub fn extract_migrated_chat_id(error_str: &str) -> Option<i64> {
    for part in error_str.split(|c: char| !c.is_ascii_digit() && c != '-') {
        if part.len() >= 10 {
            if let Ok(id) = part.parse::<i64>() {
                return Some(id);
            }
        }
    }
    None
}

pub fn get_retry_after_seconds(error: &RequestError) -> Option<u64> {
    match error {
        RequestError::RetryAfter(duration) => Some(duration.as_secs()),
        _ => None,
    }
}

pub async fn handle_telegram_error(bot: &Bot, error: &RequestError, chat_id: i64, context: &str) {
    let error_kind = classify_telegram_error(error);

    match error_kind {
        TelegramErrorKind::RateLimited => {
            let retry_after = get_retry_after_seconds(error).unwrap_or(0);
            tracing::warn!(
                "Telegram rate limit hit for chat {}: retry after {}s",
                chat_id,
                retry_after
            );
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Warning,
                    "Telegram-RateLimit",
                    &format!(
                        "Rate limited while {}: chat {}, retry after {}s",
                        context, chat_id, retry_after
                    ),
                )
                .await;
        }
        TelegramErrorKind::BotBlocked => {
            tracing::info!("Bot blocked by user in chat {}", chat_id);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Info,
                    "Telegram-Blocked",
                    &format!("Bot blocked by user in chat {} while {}", chat_id, context),
                )
                .await;
        }
        TelegramErrorKind::ChatNotFound => {
            tracing::warn!("Chat {} not found on Telegram", chat_id);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Warning,
                    "Telegram-ChatNotFound",
                    &format!("Chat {} not found while {}", chat_id, context),
                )
                .await;
        }
        TelegramErrorKind::GroupMigrated { new_chat_id } => {
            tracing::info!("Chat {} migrated to supergroup {}", chat_id, new_chat_id);
            ALERTS
                .send_alert(
                    bot,
                    Severity::Info,
                    "Telegram-Migrated",
                    &format!(
                        "Chat {} migrated to supergroup {} while {}",
                        chat_id, new_chat_id, context
                    ),
                )
                .await;
        }
        TelegramErrorKind::NotEnoughRights => {
            tracing::warn!(
                "Bot has insufficient rights to send messages to chat {}",
                chat_id
            );
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Warning,
                    "Telegram-NoRights",
                    &format!(
                        "Not enough rights to send to chat {} while {}",
                        chat_id, context
                    ),
                )
                .await;
        }
        TelegramErrorKind::NetworkError => {
            tracing::error!("Network error sending to chat {}: {}", chat_id, error);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Telegram-Network",
                    &format!(
                        "Network error while {} to chat {}: {}",
                        context, chat_id, error
                    ),
                )
                .await;
        }
        TelegramErrorKind::Other => {
            tracing::error!("Telegram error for chat {}: {}", chat_id, error);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Warning,
                    "Telegram",
                    &format!("Error while {} to chat {}: {}", context, chat_id, error),
                )
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_classify_rate_limited() {
        let error = RequestError::RetryAfter(Duration::from_secs(30));
        assert_eq!(
            classify_telegram_error(&error),
            TelegramErrorKind::RateLimited
        );
    }

    #[test]
    fn test_classify_migrate_to_chat_id() {
        let error = RequestError::MigrateToChatId(-1003300345700);
        assert_eq!(
            classify_telegram_error(&error),
            TelegramErrorKind::GroupMigrated {
                new_chat_id: -1003300345700
            }
        );
    }

    #[test]
    fn test_extract_migrated_chat_id() {
        assert_eq!(
            extract_migrated_chat_id("migrated to a supergroup with ID #-1003300345700"),
            Some(-1003300345700)
        );
        assert_eq!(
            extract_migrated_chat_id("migrate_to_chat_id: -1001234567890"),
            Some(-1001234567890)
        );
        assert_eq!(extract_migrated_chat_id("no id here"), None);
        assert_eq!(extract_migrated_chat_id("short -123"), None);
    }

    #[test]
    fn test_get_retry_after() {
        let error = RequestError::RetryAfter(Duration::from_secs(60));
        assert_eq!(get_retry_after_seconds(&error), Some(60));

        let other_error = RequestError::InvalidJson {
            source: serde_json::from_str::<serde_json::Value>("invalid").unwrap_err(),
            raw: "test".into(),
        };
        assert_eq!(get_retry_after_seconds(&other_error), None);
    }
}
