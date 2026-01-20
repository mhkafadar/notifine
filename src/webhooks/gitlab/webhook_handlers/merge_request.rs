use crate::utils::branch_filter::BranchFilter;
use actix_web::web;
use html_escape::encode_text;
use serde::Deserialize;
use ureq::serde_json;

#[derive(Debug, Deserialize)]
struct MergeRequestEvent {
    user: User,
    object_attributes: MergeRequestDetails,
}

#[derive(Debug, Deserialize)]
struct MergeRequestDetails {
    title: String,
    url: String,
    source_branch: String,
    target_branch: String,
    action: Option<String>,
}

#[derive(Debug, Deserialize)]
struct User {
    name: String,
}

// TODO also implement environment name

pub fn handle_merge_request_event(
    body: &web::Bytes,
    branch_filter: Option<&BranchFilter>,
) -> String {
    let merge_request_event: MergeRequestEvent = match serde_json::from_slice(body) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to parse GitLab merge request event: {}", e);
            return String::new();
        }
    };
    let merge_request_details = &merge_request_event.object_attributes;
    let url = &merge_request_details.url;
    let title = encode_text(&merge_request_details.title);
    let source_branch = encode_text(&merge_request_details.source_branch);
    let target_branch_raw = &merge_request_details.target_branch;
    let target_branch = encode_text(target_branch_raw);
    let sender = encode_text(&merge_request_event.user.name);

    // Apply branch filter if provided (filter based on target branch)
    if let Some(filter) = branch_filter {
        if !filter.should_process(target_branch_raw) {
            tracing::info!(
                "Filtered out GitLab merge request event for target branch: {}",
                target_branch_raw
            );
            return String::new();
        }
    }

    let action = match &merge_request_details.action {
        Some(action) => action,
        None => "none",
    };

    if action == "open" {
        format!(
            "<b>{sender}</b> opened a new merge request <a href=\"{url}\">{title}</a> \
             from <code>{source_branch}</code> to <code>{target_branch}</code>\n",
        )
    } else if action == "update" {
        format!(
            "<b>{sender}</b> updated merge request <a href=\"{url}\">{title}</a> \
             from <code>{source_branch}</code> to <code>{target_branch}</code>\n",
        )
    } else if action == "merge" {
        format!(
            "<b>{sender}</b> merged merge request <a href=\"{url}\">{title}</a> \
             from <code>{source_branch}</code> to <code>{target_branch}</code>\n",
        )
    } else if action == "close" {
        format!(
            "<b>{sender}</b> closed merge request <a href=\"{url}\">{title}</a> \
             from <code>{source_branch}</code> to <code>{target_branch}</code>\n",
        )
    } else if action == "reopen" {
        format!(
            "<b>{sender}</b> reopened merge request <a href=\"{url}\">{title}</a> \
             from <code>{source_branch}</code> to <code>{target_branch}</code>\n",
        )
    } else if action == "none" {
        format!(
            "<b>{sender}</b> opened a new merge request <a href=\"{url}\">{title}</a> \
             from <code>{source_branch}</code> to <code>{target_branch}</code> (Merge Request without any action)\n",
        )
    } else {
        String::from("Unknown merge request action")
    }
}
