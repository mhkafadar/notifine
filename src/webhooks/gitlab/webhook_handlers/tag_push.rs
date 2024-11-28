use actix_web::web;
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
    let tag_push_event: TagPushEvent = serde_json::from_slice(body).unwrap();

    let tag_name = tag_push_event.ref_field.trim_start_matches("refs/tags/");
    let project_name = &tag_push_event.project.name;
    let project_url = &tag_push_event.project.homepage;

    let tag_url = &format!("{}/-/tree/{}", project_url, tag_name);
    let sender = &tag_push_event.user_name;

    format!(
        "<b>{sender}</b> pushed a new tag <a href=\"{tag_url}\">{tag_name}</a> to <a href=\"{project_url}\">{project_name}</a>\n",
    )
}
