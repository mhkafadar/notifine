use crate::http_server::GitlabEvent;

pub fn handle_push_event(gitlab_event: &GitlabEvent) -> String {
    let branch_ref = &gitlab_event.r#ref.as_ref().unwrap();
    let branch_name = branch_ref.split('/').last().unwrap();
    let project = &gitlab_event.project;

    // replace - with \- to avoid error in telegram markdown
    let project_name = &project.name;
    let project_url = &project.homepage;

    let mut commit_paragraph = String::new();

    for commit in gitlab_event.commits.as_ref().unwrap() {
        log::info!("Commit: {}", commit.message);
        log::info!("Commit url: {}", commit.url);
        log::info!("Commit author: {}", commit.author.name);

        let commit_url = &commit.url;
        let commit_message = &commit.message.trim_end();
        let commit_author_name = &commit.author.name;

        // commit_paragraph.push_str(&format!("{}: [{}]({}) to [{}:{}]({})\n", commit_author_name, commit_message, commit_url, project_name, branch_name, project_url));
        commit_paragraph.push_str(&format!(
            "<b>{}</b>: <a href=\"{}\">{}</a> to <a href=\"{}\">{}:{}</a>\n",
            commit_author_name, commit_url, commit_message, project_url, project_name, branch_name
        ));
    }

    commit_paragraph
}
