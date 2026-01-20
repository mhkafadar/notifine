use super::utils::parse_webhook_payload;
use crate::utils::branch_filter::BranchFilter;
use actix_web::web;
use html_escape::encode_text;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct WorkflowRunEvent {
    action: String,
    workflow_run: WorkflowRun,
    repository: Repository,
    sender: Sender,
}

#[derive(Debug, Deserialize)]
struct WorkflowRun {
    name: Option<String>,
    html_url: String,
    status: String,
    conclusion: Option<String>,
    head_branch: String,
    run_number: i64,
}

#[derive(Debug, Deserialize)]
struct Repository {
    name: String,
    html_url: String,
}

#[derive(Debug, Deserialize)]
struct Sender {
    login: String,
}

pub fn handle_workflow_run_event(
    body: &web::Bytes,
    branch_filter: Option<&BranchFilter>,
) -> String {
    let workflow_event: WorkflowRunEvent = match parse_webhook_payload(body) {
        Ok(event) => event,
        Err(e) => {
            tracing::error!("Failed to parse workflow run event: {}", e);
            tracing::error!("Raw payload: {}", String::from_utf8_lossy(body));
            return String::new();
        }
    };

    let action = &workflow_event.action;
    let workflow_run = &workflow_event.workflow_run;
    let repository_url = &workflow_event.repository.html_url;
    let run_url = &workflow_run.html_url;
    let branch_raw = &workflow_run.head_branch;
    let run_number = workflow_run.run_number;

    // Apply branch filter if provided
    if let Some(filter) = branch_filter {
        if !filter.should_process(branch_raw) {
            tracing::info!("Filtered out workflow run event for branch: {}", branch_raw);
            return String::new();
        }
    }

    let repository_name = encode_text(&workflow_event.repository.name);
    let sender = encode_text(&workflow_event.sender.login);
    let branch = encode_text(branch_raw);
    let workflow_name = encode_text(workflow_run.name.as_deref().unwrap_or("workflow"));

    match (action.as_str(), workflow_run.status.as_str()) {
        ("requested", _) => format!(
            "<b>{sender}</b> triggered {workflow_name} <a href=\"{run_url}\">#{run_number}</a> on branch {branch} in <a href=\"{repository_url}\">{repository_name}</a>"
        ),
        ("completed", _) => {
            let conclusion = workflow_run.conclusion.as_deref().unwrap_or("unknown");
            let status_emoji = match conclusion {
                "success" => "✅",
                "failure" => "❌",
                "cancelled" => "⚠️",
                "skipped" => "⏭️",
                _ => "❓",
            };
            format!(
                "{status_emoji} {workflow_name} <a href=\"{run_url}\">#{run_number}</a> on branch {branch} in <a href=\"{repository_url}\">{repository_name}</a> completed with status: {conclusion}"
            )
        }
        _ => String::new(),
    }
}
