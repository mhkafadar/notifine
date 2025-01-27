use crate::bots::bot_service::{BotConfig, BotService, TelegramMessage};
use crate::utils::telegram_admin::send_message_to_admin;
use actix_web::{post, web, HttpResponse, Responder};
use notifine::{find_chat_by_id, find_webhook_by_webhook_url};
use std::env;

#[post("/beep/{webhook_url}")]
pub async fn handle_beep_webhook(
    webhook_url: web::Path<String>,
    body: web::Bytes,
) -> impl Responder {
    let event_name = "beep";
    log::info!("Event name: {:?}", event_name);
    // create a message from request body. json stringified
    let message = String::from_utf8(body.to_vec()).unwrap();
    log::info!("Message: {}", message);
    // if message is empty, then we don't need to send it to telegram
    if message.is_empty() {
        return HttpResponse::Ok();
    }

    let webhook_url = &webhook_url;
    log::info!("webhook_url: {}", webhook_url);
    let webhook = find_webhook_by_webhook_url(webhook_url);

    if webhook.is_none() {
        log::error!("Webhook not found");
        return HttpResponse::NotFound();
    }
    let webhook = webhook.unwrap();

    // log chat_id
    log::info!("Webhook: {}", webhook.webhook_url);
    let chat_id = webhook.chat_id.expect("Chat id must be set");
    log::info!("Chat id: {}", chat_id);

    let chat = find_chat_by_id(webhook.chat_id.expect("Chat id must be set"));

    if chat.is_none() {
        log::error!("Chat not found");
        return HttpResponse::NotFound();
    }
    let chat = chat.unwrap();

    let beep_bot = BotService::new(BotConfig {
        bot_name: "Beep".to_string(),
        token: env::var("BEEP_TELOXIDE_TOKEN").expect("BEEP_TELOXIDE_TOKEN must be set"),
    });

    log::info!("Sending message to chat_id: {}", chat_id);
    log::info!("Message: {}", message);
    // log gitlab bot
    log::info!("Beep bot: {:?}", beep_bot);

    let thread_id = chat.thread_id.and_then(|tid| tid.parse::<i32>().ok());

    let result = beep_bot
        .send_telegram_message(TelegramMessage {
            chat_id: chat
                .telegram_id
                .parse::<i64>()
                .expect("CHAT_ID must be an integer"),
            thread_id,
            message,
        })
        .await;

    if let Err(e) = result {
        log::error!(
            "Failed to send Telegram message: {} for webhook_url: {}",
            e,
            &webhook_url
        );
    }

    send_message_to_admin(
        &beep_bot.bot,
        format!("Event: {event_name:?}, Chat id: {chat_id}"),
        50,
    )
    .await
    .unwrap();

    HttpResponse::Ok()
}
