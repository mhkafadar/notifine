use crate::bots::bot_service::{BotConfig, BotService, TelegramMessage};
use crate::observability::alerts::Severity;
use crate::observability::telegram_errors::{
    classify_telegram_error, handle_telegram_error, TelegramErrorKind,
};
use crate::observability::{ALERTS, METRICS};
use crate::services::broadcast::db::{
    handle_bot_removed, mark_all_bots_unreachable_and_deactivate, migrate_chat_id,
    upsert_chat_bot_subscription,
};
use crate::services::broadcast::types::BotType;
use crate::utils::telegram_admin::send_message_to_admin;
use actix_web::HttpResponse;
use notifine::db::DbPool;
use notifine::{find_chat_by_id, find_webhook_by_webhook_url};

const TELEGRAM_MAX_MESSAGE_BYTES: usize = 4096;
const TRUNCATION_SUFFIX: &str = "\n\n... (truncated)";
// Reserve space for closing tags and suffix
const SAFE_MARGIN: usize = 200;

fn truncate_message(message: String) -> String {
    if message.len() <= TELEGRAM_MAX_MESSAGE_BYTES {
        return message;
    }

    let max_content_bytes = TELEGRAM_MAX_MESSAGE_BYTES - TRUNCATION_SUFFIX.len() - SAFE_MARGIN;

    // Truncate by bytes, but ensure we don't cut in the middle of a UTF-8 character
    let mut truncated = String::new();
    for ch in message.chars() {
        if truncated.len() + ch.len_utf8() > max_content_bytes {
            break;
        }
        truncated.push(ch);
    }

    // Remove incomplete HTML tag at the end (e.g., "<a href="..." without closing >)
    if let Some(last_open) = truncated.rfind('<') {
        if truncated[last_open..].find('>').is_none() {
            truncated.truncate(last_open);
        }
    }

    // Close any unclosed HTML tags
    truncated = close_unclosed_tags(&truncated);

    truncated.push_str(TRUNCATION_SUFFIX);
    truncated
}

fn close_unclosed_tags(html: &str) -> String {
    let mut result = html.to_string();
    let tags = ["a", "b", "i", "u", "s", "code", "pre"];

    for tag in tags {
        let open_pattern = format!("<{}", tag);
        let close_pattern = format!("</{}>", tag);

        let open_count = result.matches(&open_pattern).count();
        let close_count = result.matches(&close_pattern).count();

        for _ in 0..(open_count.saturating_sub(close_count)) {
            result.push_str(&format!("</{}>", tag));
        }
    }

    result
}

pub struct WebhookContext<'a> {
    pub pool: &'a DbPool,
    pub webhook_url: &'a str,
    pub message: String,
    pub bot_name: &'a str,
    pub token: String,
    pub event_name: &'a str,
    pub source: &'a str,
}

pub async fn process_webhook(ctx: WebhookContext<'_>) -> HttpResponse {
    METRICS.increment_webhooks(ctx.source);

    if ctx.message.is_empty() {
        return HttpResponse::Ok().finish();
    }

    let bot = BotService::new(
        BotConfig {
            bot_name: ctx.bot_name.to_string(),
            token: ctx.token.clone(),
            webhook_base_url: String::new(),
            admin_chat_id: None,
        },
        ctx.pool.clone(),
    );

    tracing::info!("webhook_url: {}", ctx.webhook_url);
    let webhook = match find_webhook_by_webhook_url(ctx.pool, ctx.webhook_url) {
        Ok(Some(w)) => w,
        Ok(None) => {
            tracing::error!("Webhook not found");
            return HttpResponse::NotFound().finish();
        }
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    &bot.bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find webhook: {}", e),
                )
                .await;
            return HttpResponse::InternalServerError().finish();
        }
    };

    tracing::info!("Webhook: {}", webhook.webhook_url);
    let chat_id = match webhook.chat_id {
        Some(id) => id,
        None => {
            tracing::error!("Webhook {} has no chat_id", webhook.webhook_url);
            return HttpResponse::InternalServerError().finish();
        }
    };
    tracing::info!("Chat id: {}", chat_id);

    let chat = match find_chat_by_id(ctx.pool, chat_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::error!("Chat not found");
            return HttpResponse::NotFound().finish();
        }
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    &bot.bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find chat {}: {}", chat_id, e),
                )
                .await;
            return HttpResponse::InternalServerError().finish();
        }
    };

    if !chat.is_active {
        tracing::info!(
            "Webhook rejected for deactivated chat {}: {}",
            chat_id,
            ctx.webhook_url
        );
        return HttpResponse::BadRequest()
            .content_type("application/json")
            .body(r#"{"error":"chat_deactivated","message":"Re-add the bot to receive notifications"}"#);
    }

    tracing::info!("Sending message to chat_id: {}", chat_id);
    tracing::info!("Message: {}", ctx.message);

    let thread_id = chat.thread_id.and_then(|tid| tid.parse::<i32>().ok());

    let telegram_chat_id = match chat.telegram_id.parse::<i64>() {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Invalid telegram_id '{}': {}", chat.telegram_id, e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let message = truncate_message(ctx.message);

    let result = bot
        .send_telegram_message(TelegramMessage {
            chat_id: telegram_chat_id,
            thread_id,
            message: message.clone(),
        })
        .await;

    let bot_type = BotType::parse(ctx.bot_name);

    match &result {
        Ok(_) => {
            METRICS.increment_messages_sent_for_bot(ctx.source);
            if let Some(bt) = bot_type {
                if let Err(e) = upsert_chat_bot_subscription(ctx.pool, telegram_chat_id, bt, true) {
                    tracing::warn!("Failed to track subscription for {:?}: {:?}", bt, e);
                }
            }
        }
        Err(e) => {
            tracing::error!(
                "Failed to send Telegram message: {} for webhook_url: {}",
                e,
                ctx.webhook_url
            );

            let error_kind = classify_telegram_error(e);
            let mut recovery_succeeded = false;

            match &error_kind {
                TelegramErrorKind::GroupMigrated { new_chat_id } => {
                    tracing::info!(
                        "Chat {} migrated to {}, updating database and retrying",
                        telegram_chat_id,
                        new_chat_id
                    );
                    match migrate_chat_id(ctx.pool, telegram_chat_id, *new_chat_id) {
                        Ok(true) => {
                            tracing::info!(
                                "Successfully migrated chat {} to {}",
                                telegram_chat_id,
                                new_chat_id
                            );
                            let retry_result = bot
                                .send_telegram_message(TelegramMessage {
                                    chat_id: *new_chat_id,
                                    thread_id,
                                    message: message.clone(),
                                })
                                .await;

                            if retry_result.is_ok() {
                                METRICS.increment_messages_sent_for_bot(ctx.source);
                                recovery_succeeded = true;
                                if let Some(bt) = bot_type {
                                    if let Err(sub_err) = upsert_chat_bot_subscription(
                                        ctx.pool,
                                        *new_chat_id,
                                        bt,
                                        true,
                                    ) {
                                        tracing::warn!(
                                            "Failed to track subscription for {:?}: {:?}",
                                            bt,
                                            sub_err
                                        );
                                    }
                                }
                            } else if let Err(retry_err) = &retry_result {
                                tracing::error!(
                                    "Retry to new chat ID {} failed after migration from {} (webhook: {}): {:?}",
                                    new_chat_id,
                                    telegram_chat_id,
                                    ctx.webhook_url,
                                    retry_err
                                );
                                handle_telegram_error(
                                    &bot.bot,
                                    retry_err,
                                    *new_chat_id,
                                    "retrying after migration",
                                )
                                .await;
                                recovery_succeeded = true;
                            }
                        }
                        Ok(false) => {
                            tracing::warn!(
                                "Chat {} not found in database during migration (possibly already migrated)",
                                telegram_chat_id
                            );
                            let retry_result = bot
                                .send_telegram_message(TelegramMessage {
                                    chat_id: *new_chat_id,
                                    thread_id,
                                    message: message.clone(),
                                })
                                .await;

                            if retry_result.is_ok() {
                                METRICS.increment_messages_sent_for_bot(ctx.source);
                                recovery_succeeded = true;
                                if let Some(bt) = bot_type {
                                    if let Err(sub_err) = upsert_chat_bot_subscription(
                                        ctx.pool,
                                        *new_chat_id,
                                        bt,
                                        true,
                                    ) {
                                        tracing::warn!(
                                            "Failed to track subscription for {:?}: {:?}",
                                            bt,
                                            sub_err
                                        );
                                    }
                                }
                            }
                        }
                        Err(db_err) => {
                            tracing::error!("Failed to migrate chat ID in database: {:?}", db_err);
                            METRICS.increment_errors();
                            ALERTS
                                .send_alert(
                                    &bot.bot,
                                    Severity::Error,
                                    "Database-Migration",
                                    &format!(
                                        "Failed to migrate chat {} to {}: {}",
                                        telegram_chat_id, new_chat_id, db_err
                                    ),
                                )
                                .await;
                        }
                    }
                }
                TelegramErrorKind::ChatNotFound => {
                    tracing::warn!(
                        "Chat {} not found, marking all bots unreachable and deactivating",
                        telegram_chat_id
                    );
                    if let Err(db_err) =
                        mark_all_bots_unreachable_and_deactivate(ctx.pool, telegram_chat_id)
                    {
                        tracing::error!(
                            "Failed to deactivate chat {}: {:?}",
                            telegram_chat_id,
                            db_err
                        );
                    }
                }
                TelegramErrorKind::BotBlocked | TelegramErrorKind::NotEnoughRights => {
                    if let Some(bt) = bot_type {
                        tracing::info!(
                            "Bot {:?} blocked/no rights for chat {}, marking as unreachable",
                            bt,
                            telegram_chat_id
                        );
                        if let Err(db_err) = handle_bot_removed(ctx.pool, telegram_chat_id, bt) {
                            tracing::error!(
                                "Failed to mark bot as unreachable for chat {}: {:?}",
                                telegram_chat_id,
                                db_err
                            );
                        }
                    }
                }
                TelegramErrorKind::NetworkError | TelegramErrorKind::RateLimited => {}
                TelegramErrorKind::Other => {
                    if let Some(bt) = bot_type {
                        if let Err(sub_err) =
                            upsert_chat_bot_subscription(ctx.pool, telegram_chat_id, bt, false)
                        {
                            tracing::warn!(
                                "Failed to track subscription failure for {:?}: {:?}",
                                bt,
                                sub_err
                            );
                        }
                    }
                }
            }

            if !recovery_succeeded {
                handle_telegram_error(
                    &bot.bot,
                    e,
                    telegram_chat_id,
                    "sending webhook notification",
                )
                .await;
            }
        }
    }

    if let Err(e) = send_message_to_admin(
        &bot.bot,
        format!("Event: {}, Chat id: {}", ctx.event_name, chat_id),
        50,
    )
    .await
    {
        tracing::warn!("Failed to send admin notification: {}", e);
    }

    HttpResponse::Ok().finish()
}
