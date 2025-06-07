use crate::bots::bot_service::{BotConfig, BotService, TelegramMessage};
use crate::utils::branch_filter::BranchFilter;
use crate::utils::telegram_admin::send_message_to_admin;
use crate::webhooks::github::webhook_handlers::{
    handle_check_run_event, handle_comment_event, handle_create_event, handle_delete_event,
    handle_issue_event, handle_ping_event, handle_pull_request_event, handle_push_event,
    handle_wiki_event, handle_workflow_run_event,
};
use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use notifine::{find_chat_by_id, find_webhook_by_webhook_url};
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    pub branch: Option<String>,
    pub exclude_branch: Option<String>,
}

#[post("/github/{webhook_url}")]
pub async fn handle_github_webhook(
    webhook_url: web::Path<String>,
    query: web::Query<QueryParams>,
    req: HttpRequest,
    body: web::Bytes,
) -> impl Responder {
    if let Some(event_name) = req.headers().get("x-github-event") {
        log::info!("Event name: {:?}", event_name);

        // Create branch filter from query parameters
        let branch_filter =
            match BranchFilter::new(query.branch.as_deref(), query.exclude_branch.as_deref()) {
                Ok(filter) => Some(filter),
                Err(e) => {
                    log::error!("Invalid branch filter pattern: {}", e);
                    return HttpResponse::BadRequest();
                }
            };

        let message = match event_name.to_str() {
            Ok("ping") => handle_ping_event(&body),
            Ok("push") => handle_push_event(&body, branch_filter.as_ref()),
            Ok("issues") => handle_issue_event(&body),
            Ok("pull_request") => handle_pull_request_event(&body, branch_filter.as_ref()),
            Ok("issue_comment") | Ok("pull_request_review_comment") | Ok("commit_comment") => {
                handle_comment_event(&body, false)
            }
            Ok("check_run") => handle_check_run_event(&body),
            Ok("create") => handle_create_event(&body, branch_filter.as_ref()),
            Ok("delete") => handle_delete_event(&body, branch_filter.as_ref()),
            Ok("gollum") => handle_wiki_event(&body),
            Ok("workflow_run") => handle_workflow_run_event(&body, branch_filter.as_ref()),
            _ => String::new(),
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

        let github_bot = BotService::new(BotConfig {
            bot_name: "Github".to_string(),
            token: env::var("GITHUB_TELOXIDE_TOKEN").expect("GITHUB_TELOXIDE_TOKEN must be set"),
        });

        log::info!("Sending message to chat_id: {}", chat_id);
        log::info!("Message: {}", message);
        // log gitlab bot
        log::info!("Github bot: {:?}", github_bot);

        let thread_id = chat.thread_id.and_then(|tid| tid.parse::<i32>().ok());

        let result = github_bot
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
            &github_bot.bot,
            format!("Event: {event_name:?}, Chat id: {chat_id}"),
            50,
        )
        .await
        .unwrap();

        HttpResponse::Ok()
    } else {
        HttpResponse::BadRequest()
    }
}
