use crate::webhook_handlers::issue::handle_issue_event;
use crate::webhook_handlers::note::handle_note_event;
use crate::webhook_handlers::push::handle_push_event;
use crate::webhook_handlers::tag_push::handle_tag_push_event;
use crate::webhook_handlers::unknown_event::handle_unknown_event;
use actix_web::{
    get, middleware, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use serde::Deserialize;
use telegram_gitlab::{find_chat_by_id, find_webhook_by_webhook_url};

#[derive(Debug, Deserialize)]
pub struct GitlabEvent {
    pub object_kind: String,
    pub event_name: Option<String>,
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
    pub repository: Repository,
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
    pub homepage: String,
    pub url: String,
    pub ssh_url: String,
    pub http_url: String,
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
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
}

pub async fn run_http_server() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            .service(health)
            .service(handle_gitlab_webhook)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[get("/health")]
async fn health() -> impl Responder {
    log::info!("Health check");
    "I'm ok"
}

#[derive(Deserialize)]
struct GitlabWebhook {
    webhook_url: String,
}

#[post("/gitlab/{webhook_url}")]
async fn handle_gitlab_webhook(
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
        // "pipeline" => handle_pipeline_event(), // TODO implement
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

    crate::telegram_bot::send_message(
        chat.telegram_id
            .parse::<i64>()
            .expect("CHAT_ID must be an integer"),
        message,
    )
    .await
    .unwrap();

    log::info!("bot sent message");
    HttpResponse::Ok() //
}
