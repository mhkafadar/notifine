pub fn handle_unknown_event(event_name: String) -> String {
    tracing::info!("Unknown event: {}", event_name);

    String::new()
}
