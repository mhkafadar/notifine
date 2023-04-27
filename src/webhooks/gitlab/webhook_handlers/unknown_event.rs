use crate::webhooks::gitlab::http_server::GitlabEvent;

pub fn handle_unknown_event(event_name: String) -> String {
    log::info!("Unknown event: {}", event_name);

    String::new()
}
