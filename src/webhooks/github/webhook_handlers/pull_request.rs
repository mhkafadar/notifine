use super::utils::parse_webhook_payload;
use actix_web::web;
use html_escape::encode_text;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PullRequestEvent {
    action: String,
    pull_request: PullRequest,
    repository: Repository,
    sender: Sender,
}

#[derive(Debug, Deserialize)]
struct PullRequest {
    html_url: String,
    number: i64,
    title: String,
    merged: bool,
    head: Branch,
    base: Branch,
}

#[derive(Debug, Deserialize)]
struct Branch {
    label: String,
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

pub fn handle_pull_request_event(body: &web::Bytes) -> String {
    let pr_event: PullRequestEvent = match parse_webhook_payload(body) {
        Ok(event) => event,
        Err(e) => {
            log::error!("Failed to parse pull request event: {}", e);
            log::error!("Raw payload: {}", String::from_utf8_lossy(body));
            return String::new();
        }
    };

    let action = &pr_event.action;
    let pr_title = encode_text(&pr_event.pull_request.title);
    let pr_url = &pr_event.pull_request.html_url;
    let pr_number = pr_event.pull_request.number;
    let repository_name = &pr_event.repository.name;
    let repository_url = &pr_event.repository.html_url;
    let sender = &pr_event.sender.login;
    let source_branch = &pr_event.pull_request.head.label;
    let target_branch = &pr_event.pull_request.base.label;

    match action.as_str() {
        "opened" => format!(
            "<b>{sender}</b> opened a new pull request <a href=\"{pr_url}\">#{pr_number}</a> in <a href=\"{repository_url}\">{repository_name}</a>:\n\
            {pr_title}\n\
            {source_branch} → {target_branch}"
        ),
        "closed" if pr_event.pull_request.merged => format!(
            "<b>{sender}</b> merged pull request <a href=\"{pr_url}\">#{pr_number}</a> in <a href=\"{repository_url}\">{repository_name}</a>"
        ),
        "closed" => format!(
            "<b>{sender}</b> closed pull request <a href=\"{pr_url}\">#{pr_number}</a> in <a href=\"{repository_url}\">{repository_name}</a>"
        ),
        "reopened" => format!(
            "<b>{sender}</b> reopened pull request <a href=\"{pr_url}\">#{pr_number}</a> in <a href=\"{repository_url}\">{repository_name}</a>"
        ),
        _ => String::new(),
    }
}
