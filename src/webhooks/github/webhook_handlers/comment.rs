use super::utils::parse_webhook_payload;
use actix_web::web;
use html_escape::encode_text;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CommentEvent {
    action: String,
    comment: Comment,
    #[serde(default)]
    issue: Option<Issue>,
    #[serde(default)]
    pull_request: Option<PullRequest>,
    repository: Repository,
    sender: Sender,
}

#[derive(Debug, Deserialize)]
struct Comment {
    html_url: String,
    body: String,
    commit_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Issue {
    html_url: String,
    number: i64,
}

#[derive(Debug, Deserialize)]
struct PullRequest {
    html_url: String,
    number: i64,
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

trait ProcessComment {
    fn process(&self, full_message: bool) -> String;
}

impl ProcessComment for CommentEvent {
    fn process(&self, full_message: bool) -> String {
        let comment = encode_text(&self.comment.body);
        let char_count = comment.chars().count();

        if full_message || char_count <= 100 {
            comment.into_owned()
        } else {
            let mut truncated: String = comment.chars().take(100).collect();
            truncated.push_str(&format!("<a href=\"{}\">...</a>", self.comment.html_url));
            truncated
        }
    }
}

pub fn handle_comment_event(body: &web::Bytes, full_message: bool) -> String {
    let comment_event: CommentEvent = match parse_webhook_payload(body) {
        Ok(event) => event,
        Err(e) => {
            tracing::error!("Failed to parse comment event: {}", e);
            tracing::error!("Raw payload: {}", String::from_utf8_lossy(body));
            return String::new();
        }
    };

    if comment_event.action != "created" {
        return String::new();
    }

    let sender = encode_text(&comment_event.sender.login);
    let repository_name = encode_text(&comment_event.repository.name);
    let repository_url = &comment_event.repository.html_url;
    let comment = comment_event.process(full_message);

    if let Some(issue) = &comment_event.issue {
        format!(
            "<b>{sender}</b> commented on issue <a href=\"{url}\">#{number}</a> in <a href=\"{repository_url}\">{repository_name}</a>:\n{comment}",
            url = issue.html_url,
            number = issue.number
        )
    } else if let Some(pr) = &comment_event.pull_request {
        format!(
            "<b>{sender}</b> commented on pull request <a href=\"{url}\">#{number}</a> in <a href=\"{repository_url}\">{repository_name}</a>:\n{comment}",
            url = pr.html_url,
            number = pr.number
        )
    } else if let Some(commit_id) = &comment_event.comment.commit_id {
        format!(
            "<b>{sender}</b> commented on commit <a href=\"{url}\">#{commit_id}</a> in <a href=\"{repository_url}\">{repository_name}</a>:\n{comment}",
            url = comment_event.comment.html_url,
        )
    } else {
        String::new()
    }
}
