use actix_web::web;
use html_escape::encode_text;
use serde::Deserialize;
use ureq::serde_json;

#[derive(Debug, Deserialize)]
pub struct PushEvent {
    repository: Repository,
    sender: Sender,
    forced: bool,
    commits: Vec<Commit>,
    #[serde(rename = "ref")]
    pub ref_field: String,
    before: String,
    after: String,
}

#[derive(Debug, Deserialize)]
struct Commit {
    message: String,
    url: String,
    author: Author,
}

#[derive(Debug, Deserialize)]
struct Author {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Repository {
    html_url: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct Sender {
    login: String,
}

pub fn handle_push_event(body: &web::Bytes) -> String {
    let push_event: PushEvent = serde_json::from_slice(body).unwrap();
    log::info!("Push event");

    let CreateFirstRow {
        first_row,
        delete_branch_event,
    } = create_first_row(&push_event);

    if delete_branch_event {
        return first_row;
    }

    let mut commit_paragraph = first_row;

    for commit in push_event.commits.iter().rev() {
        log::info!("Commit: {}", commit.message);
        log::info!("Commit url: {}", commit.url);
        log::info!("Commit author: {}", commit.author.name);

        let commit_url = &commit.url;
        let commit_message = encode_text(commit.message.trim_end());
        let commit_author_name = encode_text(&commit.author.name);

        commit_paragraph.push_str(&format!(
            "<b>{commit_author_name}</b>: \
            <a href=\"{commit_url}\">{commit_message}</a>\n",
        ));
    }

    commit_paragraph
}

struct CreateFirstRow {
    first_row: String,
    delete_branch_event: bool,
}

fn create_first_row(push_event: &PushEvent) -> CreateFirstRow {
    let branch_name = push_event.ref_field.split("refs/heads/").last().unwrap();
    let project_name = &push_event.repository.name;
    let project_url = &push_event.repository.html_url;
    let branch_url = format!("{project_url}/tree/{branch_name}");
    let sender = &push_event.sender.login;
    let mut delete_branch_event = false;
    let commits_length = push_event.commits.len();
    let commit_or_commits = if push_event.commits.len() > 1 {
        "commits"
    } else {
        "commit"
    };

    let first_row = if push_event.forced {
        format!(
            "<b>{sender}</b> force pushed to <a href=\"{branch_url}\">{project_name}:{branch_name}</a>\n\n"
        )
    } else if push_event.before == "0000000000000000000000000000000000000000" {
        format!(
            "<b>{sender}</b> created branch <a href=\"{branch_url}\">{branch_name}</a> \
              and pushed {commits_length} {commit_or_commits} to \
            <a href=\"{branch_url}\">{project_name}:{branch_name}</a>\n\n"
        )
    } else if push_event.after == "0000000000000000000000000000000000000000" {
        delete_branch_event = true;
        format!(
            "<b>{sender}</b> deleted branch <a href=\"{branch_url}\">{project_name}:{branch_name}</a>\n\n"
        )
    } else {
        format!(
            "<b>{sender}</b> pushed {commits_length} {commit_or_commits} to \
            <a href=\"{branch_url}\">{project_name}:{branch_name}</a>\n\n"
        )
    };

    CreateFirstRow {
        first_row,
        delete_branch_event,
    }
}
