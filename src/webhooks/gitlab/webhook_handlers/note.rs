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

trait ProcessNote {
    fn process(&self, full_message: bool) -> String;
}

impl ProcessNote for NoteEvent {
    fn process(&self, full_message: bool) -> String {
        let note = encode_text(&self.object_attributes.note);
        let char_count = note.chars().count();

        if full_message || char_count <= 100 {
            note.into_owned()
        } else {
            let mut truncated: String = note.chars().take(100).collect();
            truncated.push_str(&format!(
                "<a href=\"{}\">...</a>",
                self.object_attributes.url
            ));
            truncated
        }
    }
}

pub fn handle_note_event(body: &web::Bytes, full_message: bool) -> String {
    let note_event: NoteEvent = match serde_json::from_slice(body) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to parse GitLab note event: {}", e);
            return String::new();
        }
    };

    let user_name = encode_text(&note_event.user.name);
    let note_details = &note_event.object_attributes;
    let url = &note_details.url;
    let noteable_type = &note_details.noteable_type;
    let note = note_event.process(full_message);

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

#[cfg(test)]
mod tests {
    use super::*;
    use html_escape::encode_text;

    const TEST_LONG_NOTE: &str = "This is a very long test note that should definitely exceed the 100 character limit. \
        We want to make sure the truncation is working correctly. This text should be cut off in the truncated version.";

    #[test]
    fn test_process_note_full_message() {
        let note_event = NoteEvent {
            user: User {
                name: String::from("test_user"),
            },
            object_attributes: NoteDetails {
                url: String::from("http://example.com"),
                noteable_type: String::from("Issue"),
                note: String::from(TEST_LONG_NOTE),
            },
        };

        let result = note_event.process(true);
        assert_eq!(result, encode_text(TEST_LONG_NOTE).into_owned());
    }

    #[test]
    fn test_process_note_truncated_message() {
        let note_event = NoteEvent {
            user: User {
                name: String::from("test_user"),
            },
            object_attributes: NoteDetails {
                url: String::from("http://example.com"),
                noteable_type: String::from("Issue"),
                note: String::from(TEST_LONG_NOTE),
            },
        };

        let result = note_event.process(false);
        let expected_truncated = format!(
            "{}<a href=\"http://example.com\">...</a>",
            encode_text(TEST_LONG_NOTE)
                .chars()
                .take(100)
                .collect::<String>()
        );
        assert_eq!(result, expected_truncated);
    }

    #[test]
    fn test_process_note_short_message() {
        let short_note = "This is a short note that doesn't need truncation.";
        let note_event = NoteEvent {
            user: User {
                name: String::from("test_user"),
            },
            object_attributes: NoteDetails {
                url: String::from("http://example.com"),
                noteable_type: String::from("Issue"),
                note: String::from(short_note),
            },
        };

        let result = note_event.process(false);
        assert_eq!(result, encode_text(short_note).into_owned());
        assert!(!result.contains("..."));
    }
}
