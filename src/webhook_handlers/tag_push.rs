use crate::http_server::GitlabEvent;

pub fn handle_tag_push_event(gitlab_event: &GitlabEvent) -> String {
    let tag_ref = &gitlab_event.r#ref.as_ref().unwrap();
    let tag_name = tag_ref.split("refs/tags/").last().unwrap();
    let project = &gitlab_event.project;

    // replace - with \- to avoid error in telegram markdown
    let project_name = &project.name;
    let project_url = &project.homepage;
    let tag_url = &format!("{}/-/tree/{}", project_url, tag_name);
    let user_name = &gitlab_event.user_name.as_ref().unwrap();

    format!(
        "<b>{}</b> pushed a new tag <a href=\"{}\">{}</a> to <a href=\"{}\">{}</a>\n",
        user_name, tag_url, tag_name, project_url, project_name
    )
}
