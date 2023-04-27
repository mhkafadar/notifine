use actix_web::web;
use serde::Deserialize;
use ureq::serde_json;

#[derive(Debug, serde::Deserialize)]
struct NoteEvent {
    user: User,
    object_attributes: NoteDetails,
}

#[derive(Debug, Deserialize)]
struct User {
    name: String,
}

#[derive(Debug, Deserialize)]
struct NoteDetails {
    url: String,
    noteable_type: String,
}

pub fn handle_note_event(body: &web::Bytes) -> String {
    let note_event: NoteEvent = serde_json::from_slice(body).unwrap();

    let user_name = &note_event.user.name;
    let note_details = &note_event.object_attributes;
    let url = &note_details.url;
    let noteable_type = &note_details.noteable_type;

    if noteable_type == "Issue" {
        format!("<b>{user_name}</b> commented on an <a href=\"{url}\">issue</a>\n")
    } else if noteable_type == "MergeRequest" {
        format!("<b>{user_name}</b> commented on a <a href=\"{url}\">merge request </a>\n")
    } else if noteable_type == "Commit" {
        format!("<b>{user_name}</b> commented on a <a href=\"{url}\">commit</a>\n")
    } else if noteable_type == "Snippet" {
        format!("<b>{user_name}</b> commented on a <a href=\"{url}\">snippet</a>\n")
    } else {
        format!("<b>{user_name}</b> commented on a  <a href=\"{url}\">{noteable_type}</a>\n")
    }
}
