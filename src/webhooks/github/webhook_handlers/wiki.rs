use actix_web::web;
use html_escape::encode_text;
use serde::Deserialize;
use super::utils::parse_webhook_payload;

#[derive(Debug, Deserialize)]
pub struct GollumEvent {
    pages: Vec<WikiPage>,
    repository: Repository,
    sender: Sender,
}

#[derive(Debug, Deserialize)]
struct WikiPage {
    title: String,
    action: String,
    html_url: String,
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

pub fn handle_wiki_event(body: &web::Bytes) -> String {
    let wiki_event: GollumEvent = match parse_webhook_payload(body) {
        Ok(event) => event,
        Err(e) => {
            log::error!("Failed to parse wiki event: {}", e);
            log::error!("Raw payload: {}", String::from_utf8_lossy(body));
            return String::new();
        }
    };
    
    if wiki_event.pages.is_empty() {
        return String::new();
    }

    let repository_name = &wiki_event.repository.name;
    let repository_url = &wiki_event.repository.html_url;
    let sender = &wiki_event.sender.login;
    
    // GitHub can send multiple page updates in a single event
    let mut messages = Vec::new();
    for page in &wiki_event.pages {
        let page_title = encode_text(&page.title);
        let action = &page.action;
        let url = &page.html_url;

        messages.push(format!(
            "<b>{sender}</b> {action} wiki page <a href=\"{url}\">{page_title}</a> in <a href=\"{repository_url}\">{repository_name}</a>"
        ));
    }

    messages.join("\n")
} 