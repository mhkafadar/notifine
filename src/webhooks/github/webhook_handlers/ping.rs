use actix_web::web;
use serde::Deserialize;
use ureq::serde_json;

#[derive(Debug, Deserialize)]
pub struct PingEvent {
    zen: String,
    repository: Repository,
    sender: Sender,
}

#[derive(Debug, Deserialize)]
struct Repository {
    html_url: String,
}

#[derive(Debug, Deserialize)]
struct Sender {
    login: String,
}

pub fn handle_ping_event(body: &web::Bytes) -> String {
    let ping_event: PingEvent = serde_json::from_slice(body).unwrap();
    log::info!("Ping event");
    log::info!("Zen: {}", ping_event.zen);

    format!(
        "Congratulations! A new webhook has been successfully configured for the \
        repository: {repository_url}. This webhook was set up by \
        <a href=\"https://github.com/{sender}\">{sender}</a>. Enjoy using your webhook!",
        sender = ping_event.sender.login,
        repository_url = ping_event.repository.html_url
    )
}
