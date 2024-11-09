use actix_web::web;
use html_escape::encode_text;
use serde::Deserialize;
use ureq::serde_json;

#[derive(Debug)]
enum NoteableType {
    Issue,
    MergeRequest,
    Commit,
    Snippet,
    Other(String),
}

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
    note: String,
}

pub fn handle_note_event(body: &web::Bytes) -> String {
    let note_event: NoteEvent = serde_json::from_slice(body).unwrap();

    let user_name = &note_event.user.name;
    let note_details = &note_event.object_attributes;
    let url = &note_details.url;
    let noteable_type = &note_details.noteable_type;
    let note = encode_text(&note_details.note);

    let noteable_type = match noteable_type.as_str() {
        "Issue" => NoteableType::Issue,
        "MergeRequest" => NoteableType::MergeRequest,
        "Commit" => NoteableType::Commit,
        "Snippet" => NoteableType::Snippet,
        other => NoteableType::Other(other.to_string()),
    };

    match noteable_type {
        NoteableType::Issue => {
            format!("<b>{user_name}</b> commented on an <a href=\"{url}\">issue</a>\n{note}\n")
        }
        NoteableType::MergeRequest => {
            format!(
                "<b>{user_name}</b> commented on a <a href=\"{url}\">merge request</a>\n{note}\n"
            )
        }
        NoteableType::Commit => {
            format!("<b>{user_name}</b> commented on a <a href=\"{url}\">commit</a>\n{note}\n")
        }
        NoteableType::Snippet => {
            format!("<b>{user_name}</b> commented on a <a href=\"{url}\">snippet</a>\n{note}\n")
        }
        NoteableType::Other(type_name) => {
            format!("<b>{user_name}</b> commented on a <a href=\"{url}\">{type_name}</a>\n{note}\n")
        }
    }
}
