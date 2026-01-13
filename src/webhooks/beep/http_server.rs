use crate::bots::bot_service::{BotConfig, BotService, TelegramMessage};
use crate::observability::alerts::Severity;
use crate::observability::{ALERTS, METRICS};
use crate::utils::telegram_admin::send_message_to_admin;
use actix_web::{post, web, HttpResponse, Responder};
use notifine::db::DbPool;
use notifine::{find_chat_by_id, find_webhook_by_webhook_url};
use std::env;

#[post("/beep/{webhook_url}")]
pub async fn handle_beep_webhook(
    pool: web::Data<DbPool>,
    webhook_url: web::Path<String>,
    body: web::Bytes,
) -> impl Responder {
    METRICS.increment_webhooks("beep");

    let event_name = "beep";
    tracing::info!("Event name: {:?}", event_name);
    let message = match String::from_utf8(body.to_vec()) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Invalid UTF-8 in beep webhook body: {}", e);
            return HttpResponse::BadRequest();
        }
    };
    tracing::info!("Message: {}", message);
    if message.is_empty() {
        return HttpResponse::Ok();
    }

    let beep_token = match env::var("BEEP_TELOXIDE_TOKEN") {
        Ok(token) => token,
        Err(_) => {
            tracing::error!("BEEP_TELOXIDE_TOKEN not set");
            return HttpResponse::InternalServerError();
        }
    };
    let beep_bot = BotService::new(
        BotConfig {
            bot_name: "Beep".to_string(),
            token: beep_token,
            webhook_base_url: String::new(),
            admin_chat_id: None,
        },
        pool.get_ref().clone(),
    );

    let webhook_url = &webhook_url;
    tracing::info!("webhook_url: {}", webhook_url);
    let webhook = match find_webhook_by_webhook_url(&pool, webhook_url) {
        Ok(Some(w)) => w,
        Ok(None) => {
            tracing::error!("Webhook not found");
            return HttpResponse::NotFound();
        }
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    &beep_bot.bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find webhook: {}", e),
                )
                .await;
            return HttpResponse::InternalServerError();
        }
    };

    tracing::info!("Webhook: {}", webhook.webhook_url);
    let chat_id = match webhook.chat_id {
        Some(id) => id,
        None => {
            tracing::error!("Webhook {} has no chat_id", webhook.webhook_url);
            return HttpResponse::InternalServerError();
        }
    };
    tracing::info!("Chat id: {}", chat_id);

    let chat = match find_chat_by_id(&pool, chat_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::error!("Chat not found");
            return HttpResponse::NotFound();
        }
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    &beep_bot.bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find chat {}: {}", chat_id, e),
                )
                .await;
            return HttpResponse::InternalServerError();
        }
    };

    tracing::info!("Sending message to chat_id: {}", chat_id);
    tracing::info!("Message: {}", message);
    tracing::info!("Beep bot: {:?}", beep_bot);

    let thread_id = chat.thread_id.and_then(|tid| tid.parse::<i32>().ok());

    let telegram_chat_id = match chat.telegram_id.parse::<i64>() {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Invalid telegram_id '{}': {}", chat.telegram_id, e);
            return HttpResponse::InternalServerError();
        }
    };

    let result = beep_bot
        .send_telegram_message(TelegramMessage {
            chat_id: telegram_chat_id,
            thread_id,
            message,
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
                &webhook_url
            );
            METRICS.increment_errors();
        }
    }

    if let Err(e) = send_message_to_admin(
        &beep_bot.bot,
        format!("Event: {event_name:?}, Chat id: {chat_id}"),
        50,
    )
    .await
    {
        tracing::warn!("Failed to send admin notification: {}", e);
    }

    HttpResponse::Ok()
}
