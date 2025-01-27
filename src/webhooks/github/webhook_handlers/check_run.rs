use super::utils::parse_webhook_payload;
use actix_web::web;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CheckRunEvent {
    action: String,
    check_run: CheckRun,
    repository: Repository,
    sender: Sender,
}

#[derive(Debug, Deserialize)]
struct CheckRun {
    name: String,
    html_url: String,
    status: String,
    conclusion: Option<String>,
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

pub fn handle_check_run_event(body: &web::Bytes) -> String {
    let check_event: CheckRunEvent = match parse_webhook_payload(body) {
        Ok(event) => event,
        Err(e) => {
            log::error!("Failed to parse check run event: {}", e);
            log::error!("Raw payload: {}", String::from_utf8_lossy(body));
            return String::new();
        }
    };

    let action = &check_event.action;
    let check_run = &check_event.check_run;
    let repository_name = &check_event.repository.name;
    let repository_url = &check_event.repository.html_url;
    let sender = &check_event.sender.login;
    let check_name = &check_run.name;
    let check_url = &check_run.html_url;

    match (action.as_str(), check_run.status.as_str()) {
        ("created", _) => format!(
            "<b>{sender}</b> created check <a href=\"{check_url}\">{check_name}</a> in <a href=\"{repository_url}\">{repository_name}</a>"
        ),
        ("completed", _) => {
            let conclusion = check_run.conclusion.as_deref().unwrap_or("unknown");
            let status_emoji = match conclusion {
                "success" => "✅",
                "failure" => "❌",
                "cancelled" => "⚠️",
                "skipped" => "⏭️",
                _ => "❓",
            };
            format!(
                "{status_emoji} Check <a href=\"{check_url}\">{check_name}</a> in <a href=\"{repository_url}\">{repository_name}</a> completed with status: {conclusion}"
            )
        }
        // We don't need to handle other actions like "requested_action", "rerequested"
        _ => String::new(),
    }
}
