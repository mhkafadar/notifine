use super::utils::parse_webhook_payload;
use crate::utils::branch_filter::BranchFilter;
use actix_web::web;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateDeleteEvent {
    #[serde(rename = "ref")]
    ref_name: String,
    ref_type: String,
    repository: Repository,
    sender: Sender,
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

pub fn handle_create_event(body: &web::Bytes, branch_filter: Option<&BranchFilter>) -> String {
    let create_event: CreateDeleteEvent = match parse_webhook_payload(body) {
        Ok(event) => event,
        Err(e) => {
            tracing::error!("Failed to parse create event: {}", e);
            tracing::error!("Raw payload: {}", String::from_utf8_lossy(body));
            return String::new();
        }
    };

    // We only care about branch and tag events
    if !["branch", "tag"].contains(&create_event.ref_type.as_str()) {
        return String::new();
    }

    let ref_type = &create_event.ref_type;
    let ref_name = &create_event.ref_name;
    let repository_name = &create_event.repository.name;
    let repository_url = &create_event.repository.html_url;
    let sender = &create_event.sender.login;
    let ref_url = format!("{}/tree/{}", repository_url, ref_name);

    // Apply branch filter if provided and this is a branch event
    if ref_type == "branch" {
        if let Some(filter) = branch_filter {
            if !filter.should_process(ref_name) {
                tracing::info!("Filtered out create branch event for branch: {}", ref_name);
                return String::new();
            }
        }
    }

    format!(
        "<b>{sender}</b> created {ref_type} <a href=\"{ref_url}\">{ref_name}</a> in <a href=\"{repository_url}\">{repository_name}</a>"
    )
}

pub fn handle_delete_event(body: &web::Bytes, branch_filter: Option<&BranchFilter>) -> String {
    let delete_event: CreateDeleteEvent = match parse_webhook_payload(body) {
        Ok(event) => event,
        Err(e) => {
            tracing::error!("Failed to parse delete event: {}", e);
            tracing::error!("Raw payload: {}", String::from_utf8_lossy(body));
            return String::new();
        }
    };

    // We only care about branch and tag events
    if !["branch", "tag"].contains(&delete_event.ref_type.as_str()) {
        return String::new();
    }

    let ref_type = &delete_event.ref_type;
    let ref_name = &delete_event.ref_name;
    let repository_name = &delete_event.repository.name;
    let repository_url = &delete_event.repository.html_url;
    let sender = &delete_event.sender.login;

    // Apply branch filter if provided and this is a branch event
    if ref_type == "branch" {
        if let Some(filter) = branch_filter {
            if !filter.should_process(ref_name) {
                tracing::info!("Filtered out delete branch event for branch: {}", ref_name);
                return String::new();
            }
        }
    }

    format!(
        "<b>{sender}</b> deleted {ref_type} {ref_name} in <a href=\"{repository_url}\">{repository_name}</a>"
    )
}
