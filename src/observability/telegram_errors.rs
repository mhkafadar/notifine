use super::alerts::Severity;
use super::{ALERTS, METRICS};
use teloxide::prelude::*;
use teloxide::RequestError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TelegramErrorKind {
    RateLimited,
    BotBlocked,
    ChatNotFound,
    NetworkError,
    Other,
}

pub fn classify_telegram_error(error: &RequestError) -> TelegramErrorKind {
    match error {
        RequestError::RetryAfter(_) => TelegramErrorKind::RateLimited,
        RequestError::Api(api_error) => {
            let error_str = api_error.to_string().to_lowercase();
            if error_str.contains("blocked") || error_str.contains("bot was blocked") {
                TelegramErrorKind::BotBlocked
            } else if error_str.contains("chat not found")
                || error_str.contains("chat_not_found")
                || error_str.contains("user not found")
            {
                TelegramErrorKind::ChatNotFound
            } else {
                TelegramErrorKind::Other
            }
        }
        RequestError::Network(_) => TelegramErrorKind::NetworkError,
        _ => TelegramErrorKind::Other,
    }
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
