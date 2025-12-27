use actix_web::web;
use html_escape::encode_text;
use serde::Deserialize;
use ureq::serde_json;

#[derive(Debug)]
enum IssueAction {
    Open,
    Update,
    Close,
    Reopen,
    Other(String),
}

#[derive(Debug, Deserialize)]
struct IssueEvent {
    user: User,
    object_attributes: IssueDetails,
}

#[derive(Debug, Deserialize)]
struct IssueDetails {
    title: String,
    url: String,
    action: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct User {
    name: String,
}

pub fn handle_issue_event(body: &web::Bytes) -> String {
    let issue_event: IssueEvent = match serde_json::from_slice(body) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to parse GitLab issue event: {}", e);
            return String::new();
        }
    };

    let issue_details = &issue_event.object_attributes;
    let url = &issue_details.url;
    let title = &issue_details.title;
    let action = &issue_details.action;
    let user_name = &issue_event.user.name;
    let description = encode_text(&issue_details.description);

    let action = match action.as_str() {
        "open" => IssueAction::Open,
        "update" => IssueAction::Update,
        "close" => IssueAction::Close,
        "reopen" => IssueAction::Reopen,
        other => IssueAction::Other(other.to_string()),
    };

    match action {
        IssueAction::Open => format!(
            "<b>{user_name}</b> opened a new issue <a href=\"{url}\">{title}</a>\n{description}\n",
        ),
        IssueAction::Update => format!(
            "<b>{user_name}</b> updated issue <a href=\"{url}\">{title}</a>\n{description}\n",
        ),
        IssueAction::Close => {
            format!("<b>{user_name}</b> closed issue <a href=\"{url}\">{title}</a>\n",)
        }
        IssueAction::Reopen => {
            format!("<b>{user_name}</b> reopened issue <a href=\"{url}\">{title}</a>\n",)
        }
        IssueAction::Other(action) => {
            format!("<b>{user_name}</b> {action} issue <a href=\"{url}\">{title}</a>\n",)
        }
    }
}
