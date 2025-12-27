use html_escape::encode_text;

use crate::webhooks::gitlab::http_server::GitlabEvent;

pub fn new_branch_push(gitlab_event: &GitlabEvent) -> String {
    let branch_ref = match gitlab_event.r#ref.as_ref() {
        Some(r) => r,
        None => {
            tracing::error!("Missing ref field in GitLab new branch push event");
            return String::new();
        }
    };
    let branch_name = branch_ref.split('/').next_back().unwrap_or(branch_ref);
    let project = &gitlab_event.project;

    let project_name = &project.name;
    let fallback_project_url = "empty_gitlab_project_url".to_owned();
    let project_url = project.homepage.as_ref().unwrap_or(&fallback_project_url);
    let user = match gitlab_event.user_name.as_ref() {
        Some(u) => u,
        None => {
            tracing::error!("Missing user_name field in GitLab new branch push event");
            return String::new();
        }
    };

    let mut commit_paragraph = format!(
        "<b>{user}</b> \
        pushed a new branch <a href=\"{project_url}/-/tree/\
        {branch_name}\">{branch_name}</a> to <a href=\"{project_url}\">\
        {project_name}</a>\n\n",
    );

    let commits = match gitlab_event.commits.as_ref() {
        Some(c) => c,
        None => {
            return commit_paragraph;
        }
    };

    for commit in commits.iter().rev().take(4) {
        tracing::info!("Commit: {}", commit.message);
        tracing::info!("Commit url: {}", commit.url);
        tracing::info!("Commit author: {}", commit.author.name);

        let commit_url = &commit.url;
        let commit_message = encode_text(commit.message.trim_end());
        let commit_author_name = encode_text(&commit.author.name);

        commit_paragraph.push_str(&format!(
            "<b>{commit_author_name}</b>: \
            <a href=\"{commit_url}\">{commit_message}</a> \
            to <a href=\"{project_url}\">{project_name}:{branch_name}</a>\n",
        ));
    }

    commit_paragraph
}
