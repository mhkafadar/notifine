use crate::utils::branch_filter::BranchFilter;
use crate::webhooks::common::{process_webhook, WebhookContext};
use crate::webhooks::github::webhook_handlers::{
    handle_check_run_event, handle_comment_event, handle_create_event, handle_delete_event,
    handle_issue_event, handle_ping_event, handle_pull_request_event, handle_push_event,
    handle_wiki_event, handle_workflow_run_event,
};
use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use notifine::db::DbPool;
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    pub branch: Option<String>,
    pub exclude_branch: Option<String>,
}

#[post("/github/{webhook_url}")]
pub async fn handle_github_webhook(
    pool: web::Data<DbPool>,
    webhook_url: web::Path<String>,
    query: web::Query<QueryParams>,
    req: HttpRequest,
    body: web::Bytes,
) -> impl Responder {
    if let Some(event_name) = req.headers().get("x-github-event") {
        tracing::info!("Event name: {:?}", event_name);

        let branch_filter =
            match BranchFilter::new(query.branch.as_deref(), query.exclude_branch.as_deref()) {
                Ok(filter) => Some(filter),
                Err(e) => {
                    tracing::error!("Invalid branch filter pattern: {}", e);
                    return HttpResponse::BadRequest().finish();
                }
            };

        let event_str = event_name.to_str().unwrap_or("unknown");

        let message = match event_str {
            "ping" => handle_ping_event(&body),
            "push" => handle_push_event(&body, branch_filter.as_ref()),
            "issues" => handle_issue_event(&body),
            "pull_request" => handle_pull_request_event(&body, branch_filter.as_ref()),
            "issue_comment" | "pull_request_review_comment" | "commit_comment" => {
                handle_comment_event(&body, false)
            }
            "check_run" => handle_check_run_event(&body),
            "create" => handle_create_event(&body, branch_filter.as_ref()),
            "delete" => handle_delete_event(&body, branch_filter.as_ref()),
            "gollum" => handle_wiki_event(&body),
            "workflow_run" => handle_workflow_run_event(&body, branch_filter.as_ref()),
            _ => String::new(),
        };
        tracing::info!("Message: {}", message);

        process_webhook(WebhookContext {
            pool: pool.get_ref(),
            webhook_url: &webhook_url,
            message,
            bot_name: "Github",
            token: env::var("GITHUB_TELOXIDE_TOKEN").expect("GITHUB_TELOXIDE_TOKEN must be set"),
            event_name: event_str,
            source: "github",
        })
        .await
    } else {
        HttpResponse::BadRequest().finish()
    }
}
