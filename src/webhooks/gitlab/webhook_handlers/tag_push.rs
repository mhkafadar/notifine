use actix_web::web;
use html_escape::encode_text;
use serde::Deserialize;
use ureq::serde_json;

#[derive(Debug, Deserialize)]
struct TagPushEvent {
    #[serde(rename = "ref")]
    ref_field: String,
    project: Project,
    user_name: String,
}

#[derive(Debug, Deserialize)]
struct Project {
    name: String,
    homepage: String,
}

pub fn handle_tag_push_event(body: &web::Bytes) -> String {
    let tag_push_event: TagPushEvent = match serde_json::from_slice(body) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to parse GitLab tag push event: {}", e);
            return String::new();
        }
    };

    let tag_name_raw = tag_push_event.ref_field.trim_start_matches("refs/tags/");
    let project_url = &tag_push_event.project.homepage;
    let tag_url = format!("{}/-/tree/{}", project_url, tag_name_raw);

    let tag_name = encode_text(tag_name_raw);
    let project_name = encode_text(&tag_push_event.project.name);
    let sender = encode_text(&tag_push_event.user_name);

    format!(
        "<b>{sender}</b> pushed a new tag <a href=\"{tag_url}\">{tag_name}</a> to <a href=\"{project_url}\">{project_name}</a>\n",
    )
}
