use crate::webhooks::gitlab::http_server::GitlabEvent;
use crate::webhooks::gitlab::webhook_handlers::new_branch_push::new_branch_push;

pub fn handle_push_event(gitlab_event: &GitlabEvent) -> String {
    // check if it is a new branch push
    if gitlab_event.before.as_ref().unwrap() == "0000000000000000000000000000000000000000" {
        return new_branch_push(&gitlab_event);
    }

    let branch_ref = &gitlab_event.r#ref.as_ref().unwrap();
    let branch_name = branch_ref.split('/').last().unwrap();
    let project = &gitlab_event.project;

    // replace - with \- to avoid error in telegram markdown
    let project_name = &project.name;
    // set project_url to project.homepage if not none set "dummy_url" otherwise
    let fallback_project_url = "empty_gitlab_project_url".to_owned();
    let project_url = &project.homepage.as_ref().unwrap_or(&fallback_project_url);

    let mut commit_paragraph = String::new();

    for commit in gitlab_event.commits.as_ref().unwrap().iter().rev() {
        log::info!("Commit: {}", commit.message);
        log::info!("Commit url: {}", commit.url);
        log::info!("Commit author: {}", commit.author.name);

        let commit_url = &commit.url;
        let commit_message = &commit.message.trim_end();
        let commit_author_name = &commit.author.name;

        commit_paragraph.push_str(&format!(
            "<b>{commit_author_name}</b>: \
            <a href=\"{commit_url}\">{commit_message}</a> \
            to <a href=\"{project_url}\">{project_name}:{branch_name}</a>\n",
        ));
    }

    commit_paragraph
}
