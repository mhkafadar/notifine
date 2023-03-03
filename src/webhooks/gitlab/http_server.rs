use crate::bots::gitlab_bot::send_message_gitlab;
use crate::webhooks::gitlab::webhook_handlers::{
    issue::handle_issue_event, note::handle_note_event, pipeline::handle_pipeline_event,
    push::handle_push_event, tag_push::handle_tag_push_event, unknown_event::handle_unknown_event,
};
use actix_web::{post, web, HttpResponse, Responder};
use notifine::{find_chat_by_id, find_webhook_by_webhook_url};
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize)]
pub struct GitlabEvent {
    pub object_kind: String,
    pub event_name: Option<String>,
    pub before: Option<String>,
    pub r#ref: Option<String>,
    pub checkout_sha: Option<String>,
    pub message: Option<String>,
    pub user_id: Option<u32>,
    pub user_name: Option<String>,
    pub user_username: Option<String>,
    pub user_email: Option<String>,
    pub user_avatar: Option<String>,
    pub user: Option<User>,
    pub project_id: Option<u32>,
    pub project: Project,
    pub repository: Option<Repository>,
    pub commits: Option<Vec<Commit>>,
    pub object_attributes: Option<ObjectAttributes>,
    pub total_commits_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub name: String,
    pub username: String,
    pub email: String,
    pub avatar_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Project {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub web_url: String,
    pub avatar_url: Option<String>,
    pub git_ssh_url: String,
    pub git_http_url: String,
    pub namespace: String,
    pub visibility_level: u32,
    pub path_with_namespace: String,
    pub default_branch: String,
    pub ci_config_path: Option<String>,
    pub homepage: Option<String>,
    pub url: Option<String>,
    pub ssh_url: Option<String>,
    pub http_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Repository {
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub homepage: String,
}

#[derive(Debug, Deserialize)]
pub struct Commit {
    pub id: String,
    pub title: String,
    pub message: String,
    pub author: Author,
    pub timestamp: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct ObjectAttributes {
    pub id: u32,
    pub title: Option<String>,
    pub action: Option<String>,
    pub url: Option<String>,
    pub source: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
}

#[derive(Deserialize)]
pub struct GitlabWebhook {
    webhook_url: String,
}

#[post("/gitlab/{webhook_url}")]
pub async fn handle_gitlab_webhook(
    gitlab_webhook: web::Path<GitlabWebhook>,
    gitlab_event: web::Json<GitlabEvent>,
) -> impl Responder {
    log::info!("Event details: {:?}", &gitlab_event);
    let event_name = &gitlab_event.object_kind;

    // handle push, tag_push, issue, merge_request, note, pipeline, wiki_page, build
    let message = match event_name.as_str() {
        "push" => handle_push_event(&gitlab_event),
        "tag_push" => handle_tag_push_event(&gitlab_event),
        "issue" => handle_issue_event(&gitlab_event),
        // "merge_request" => handle_merge_request_event(), // TODO implement
        "note" => handle_note_event(&gitlab_event), // TODO implement
        "pipeline" => handle_pipeline_event(&gitlab_event), // TODO implement
        // "wiki_page" => handle_wiki_page_event(), // TODO implement
        // "build" => handle_build_event(), // TODO implement
        // TODO implement work_item (issue task list)
        _ => handle_unknown_event(&gitlab_event),
    };

    // if message is empty, then we don't need to send it to telegram
    if message.is_empty() {
        return HttpResponse::Ok();
    }

    let webhook_url = &gitlab_webhook.webhook_url;
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

    send_message_gitlab(
        chat.telegram_id
            .parse::<i64>()
            .expect("CHAT_ID must be an integer"),
        message,
    )
    .await
    .unwrap();

    // send message to telegram admin
    send_message_gitlab(
        env::var("TELEGRAM_ADMIN_CHAT_ID")
            .expect("TELEGRAM_ADMIN_CHAT_ID must be set")
            .parse::<i64>()
            .expect("Error parsing TELEGRAM_ADMIN_CHAT_ID"),
        format!("Event: {event_name}, Chat id: {chat_id}"),
    )
    .await
    .unwrap();

    log::info!("bot sent message");
    HttpResponse::Ok() //
}
