use crate::bots::beep_bot::send_message_beep;
use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use notifine::{find_chat_by_id, find_webhook_by_webhook_url};
use std::env;

#[post("/beep/{webhook_url}")]
pub async fn handle_beep_webhook(
    webhook_url: web::Path<String>,
    req: HttpRequest,
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

    send_message_beep(
        chat.telegram_id
            .parse::<i64>()
            .expect("CHAT_ID must be an integer"),
        message,
    )
    .await
    .unwrap();

    // send message to telegram admin
    send_message_beep(
        env::var("TELEGRAM_ADMIN_CHAT_ID")
            .expect("TELEGRAM_ADMIN_CHAT_ID must be set")
            .parse::<i64>()
            .expect("Error parsing TELEGRAM_ADMIN_CHAT_ID"),
        format!("Event: {event_name:?}, Chat id: {chat_id}"),
    )
    .await
    .unwrap();

    log::info!("bot sent message");
    HttpResponse::Ok() //
}
