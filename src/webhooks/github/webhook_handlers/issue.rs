use actix_web::web;
use html_escape::encode_text;
use serde::Deserialize;
use super::utils::parse_webhook_payload;

#[derive(Debug, Deserialize)]
pub struct IssueEvent {
    action: String,
    issue: Issue,
    repository: Repository,
    sender: Sender,
}

#[derive(Debug, Deserialize)]
struct Issue {
    html_url: String,
    number: i64,
    title: String,
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

pub fn handle_issue_event(body: &web::Bytes) -> String {
    let issue_event: IssueEvent = match parse_webhook_payload(body) {
        Ok(event) => event,
        Err(e) => {
            log::error!("Failed to parse issue event: {}", e);
            log::error!("Raw payload: {}", String::from_utf8_lossy(body));
            return String::new();
        }
    };

    let action = &issue_event.action;
    let issue_title = encode_text(&issue_event.issue.title);
    let issue_url = &issue_event.issue.html_url;
    let issue_number = issue_event.issue.number;
    let repository_name = &issue_event.repository.name;
    let repository_url = &issue_event.repository.html_url;
    let sender = &issue_event.sender.login;

    match action.as_str() {
        "opened" => format!(
            "<b>{sender}</b> opened a new issue <a href=\"{issue_url}\">#{issue_number}</a> in <a href=\"{repository_url}\">{repository_name}</a>:\n{issue_title}"
        ),
        "closed" => format!(
            "<b>{sender}</b> closed issue <a href=\"{issue_url}\">#{issue_number}</a> in <a href=\"{repository_url}\">{repository_name}</a>"
        ),
        "reopened" => format!(
            "<b>{sender}</b> reopened issue <a href=\"{issue_url}\">#{issue_number}</a> in <a href=\"{repository_url}\">{repository_name}</a>"
        ),
        _ => String::new(),
    }
} 