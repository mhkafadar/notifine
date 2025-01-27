use super::utils::parse_webhook_payload;
use actix_web::web;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PingEvent {
    zen: String,
    #[serde(default)]
    repository: Option<Repository>,
    sender: Option<Sender>,
    hook: Hook,
}

#[derive(Debug, Deserialize)]
struct Repository {
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct Sender {
    login: String,
}

#[derive(Debug, Deserialize)]
struct Hook {
    config: HookConfig,
}

#[derive(Debug, Deserialize)]
struct HookConfig {
    url: String,
}

pub fn handle_ping_event(body: &web::Bytes) -> String {
    let ping_event: PingEvent = match parse_webhook_payload(body) {
        Ok(event) => event,
        Err(e) => {
            log::error!("Failed to parse ping event: {}", e);
            log::error!("Raw payload: {}", String::from_utf8_lossy(body));
            return "Received ping event, but failed to parse payload.".to_string();
        }
    };

    log::info!("Ping event received with zen: {}", ping_event.zen);

    // For organization webhooks, repository might be None
    let repo_info = ping_event.repository.as_ref().map_or_else(
        || "your organization".to_string(),
        |repo| format!("repository: {}", repo.full_name),
    );

    let setup_by = ping_event.sender.as_ref().map_or_else(
        || "".to_string(),
        |sender| {
            format!(
                " This webhook was set up by <a href=\"https://github.com/{}\">{}</a>.",
                sender.login, sender.login
            )
        },
    );

    format!(
        "🎉 Congratulations! A new webhook has been successfully configured for {}. \
        The webhook URL is configured to: {}.{} \
        \nZen message from GitHub: {}",
        repo_info, ping_event.hook.config.url, setup_by, ping_event.zen
    )
}
