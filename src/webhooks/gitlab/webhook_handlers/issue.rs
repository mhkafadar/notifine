use actix_web::web;
use serde::Deserialize;
use ureq::serde_json;

#[derive(Debug, Deserialize)]
struct IssueEvent {
    user: User,
    object_attributes: IssueDetails,
}

#[derive(Debug, Deserialize)]
struct IssueDetails {
    title: String,
    url: String,
    state: String,
    action: String,
}

#[derive(Debug, Deserialize)]
struct User {
    name: String,
}

pub fn handle_issue_event(body: &web::Bytes) -> String {
    let issue_event: IssueEvent = serde_json::from_slice(body).unwrap();

    let issue_details = &issue_event.object_attributes;
    let url = &issue_details.url;
    let title = &issue_details.title;
    let action = &issue_details.action;
    let user_name = &issue_event.user.name;

    if action == "open" {
        format!("<b>{user_name}</b> opened a new issue <a href=\"{url}\">{title}</a>\n",)
    } else if action == "update" {
        format!("<b>{user_name}</b> updated issue <a href=\"{url}\">{title}</a>\n",)
    } else if action == "close" {
        format!("<b>{user_name}</b> closed issue <a href=\"{url}\">{title}</a>\n",)
    } else if action == "reopen" {
        format!("<b>{user_name}</b> reopened issue <a href=\"{url}\">{title}</a>\n",)
    } else {
        format!("<b>{user_name}</b> {action} issue <a href=\"{url}\">{title}</a>\n",)
    }
}
