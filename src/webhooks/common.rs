use crate::bots::bot_service::{BotConfig, BotService, TelegramMessage};
use crate::observability::alerts::Severity;
use crate::observability::telegram_errors::handle_telegram_error;
use crate::observability::{ALERTS, METRICS};
use crate::utils::telegram_admin::send_message_to_admin;
use actix_web::HttpResponse;
use notifine::db::DbPool;
use notifine::{find_chat_by_id, find_webhook_by_webhook_url};

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

    let result = bot
        .send_telegram_message(TelegramMessage {
            chat_id: telegram_chat_id,
            thread_id,
            message: ctx.message,
        })
        .await;

    match &result {
        Ok(_) => {
            METRICS.increment_messages_sent();
        }
        Err(e) => {
            tracing::error!(
                "Failed to send Telegram message: {} for webhook_url: {}",
                e,
                ctx.webhook_url
            );
            handle_telegram_error(
                &bot.bot,
                e,
                telegram_chat_id,
                "sending webhook notification",
            )
            .await;
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
