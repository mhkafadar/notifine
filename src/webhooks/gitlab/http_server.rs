use crate::utils::branch_filter::BranchFilter;
use crate::webhooks::common::{process_webhook, WebhookContext};
use crate::webhooks::gitlab::webhook_handlers::job::handle_job_event;
use crate::webhooks::gitlab::webhook_handlers::merge_request::handle_merge_request_event;
use crate::webhooks::gitlab::webhook_handlers::{
    issue::handle_issue_event, note::handle_note_event, push::handle_push_event,
    tag_push::handle_tag_push_event, unknown_event::handle_unknown_event,
};
use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use notifine::db::DbPool;
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    pub branch: Option<String>,
    pub exclude_branch: Option<String>,
    pub full_message: Option<String>,
}

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

#[post("/gitlab/{webhook_url}")]
pub async fn handle_gitlab_webhook(
    pool: web::Data<DbPool>,
    webhook_url: web::Path<String>,
    query: web::Query<QueryParams>,
    req: HttpRequest,
    body: web::Bytes,
) -> impl Responder {
    if let Some(event_name) = req.headers().get("x-gitlab-event") {
        let full_message = query.full_message.as_deref() == Some("true");

        let branch_filter =
            match BranchFilter::new(query.branch.as_deref(), query.exclude_branch.as_deref()) {
                Ok(filter) => Some(filter),
                Err(e) => {
                    tracing::error!("Invalid branch filter pattern: {}", e);
                    return HttpResponse::BadRequest().finish();
                }
            };

        let event_str = event_name.to_str().unwrap_or("unknown");
        tracing::info!("Event: {}", event_str);

        let message = match event_str {
            "Push Hook" => handle_push_event(&body, branch_filter.as_ref()),
            "Tag Push Hook" => handle_tag_push_event(&body),
            "Issue Hook" => handle_issue_event(&body),
            "Note Hook" => handle_note_event(&body, full_message),
            "Merge Request Hook" => handle_merge_request_event(&body, branch_filter.as_ref()),
            "Job Hook" => handle_job_event(&body),
            name => handle_unknown_event(name.to_string()),
        };

        let gitlab_token = match env::var("GITLAB_TELOXIDE_TOKEN") {
            Ok(token) => token,
            Err(_) => {
                tracing::error!("GITLAB_TELOXIDE_TOKEN not set");
                return HttpResponse::InternalServerError().finish();
            }
        };

        process_webhook(WebhookContext {
            pool: pool.get_ref(),
            webhook_url: &webhook_url,
            message,
            bot_name: "Gitlab",
            token: gitlab_token,
            event_name: event_str,
            source: "gitlab",
        })
        .await
    } else {
        HttpResponse::BadRequest().finish()
    }
}
