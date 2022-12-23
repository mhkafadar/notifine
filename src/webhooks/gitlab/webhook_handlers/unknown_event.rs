use crate::webhooks::gitlab::http_server::GitlabEvent;

pub fn handle_unknown_event(gitlab_event: &GitlabEvent) -> String {
    log::info!("Unknown event");
    let project = &gitlab_event.project;
    let project_name = &project.name;
    let project_url = &project.web_url;

    format!(
        "Unknown event has triggered: {} on <a href=\"{}\">{}</a>\n",
        &gitlab_event.object_kind, project_url, project_name
    )
}
