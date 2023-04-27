use crate::bots::github_bot::send_message_github;
use crate::webhooks::github::webhook_handlers::{ping::handle_ping_event, push::handle_push_event};
use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use notifine::{find_chat_by_id, find_webhook_by_webhook_url};
use std::env;

#[post("/github/{webhook_url}")]
pub async fn handle_github_webhook(
    webhook_url: web::Path<String>,
    req: HttpRequest,
    body: web::Bytes,
) -> impl Responder {
    if let Some(event_name) = req.headers().get("x-github-event") {
        log::info!("Event name: {:?}", event_name);
        let message = match event_name.to_str() {
            Ok("ping") => handle_ping_event(&body),
            Ok("push") => handle_push_event(&body),
            // _ => handle_unknown_event(&gitlab_event),
            _ => "".to_string(),
        };
        log::info!("Message: {}", message);

        // if message is empty, then we don't need to send it to telegram
        if message.is_empty() {
            return HttpResponse::Ok();
        }

        log::info!("webhook_url: {}", &webhook_url);
        let webhook = find_webhook_by_webhook_url(&webhook_url);

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

        send_message_github(
            chat.telegram_id
                .parse::<i64>()
                .expect("CHAT_ID must be an integer"),
            message,
        )
        .await
        .unwrap();

        // send message to telegram admin
        send_message_github(
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
    } else {
        HttpResponse::BadRequest()
    }
}
