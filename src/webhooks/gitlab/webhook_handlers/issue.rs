use crate::webhooks::gitlab::http_server::GitlabEvent;

pub fn handle_issue_event(gitlab_event: &GitlabEvent) -> String {
    let project = &gitlab_event.project;
    let project_name = &project.name;
    let project_url = &project.homepage.as_ref().unwrap();
    let user_name = &gitlab_event.user.as_ref().unwrap().name;
    let issue = &gitlab_event.object_attributes.as_ref().unwrap();
    let issue_url = &issue.url.as_ref().unwrap();
    let issue_title = &issue.title.as_ref().unwrap();

    if issue.action.is_none() {
        return "".to_string();
    }

    let issue_action = &issue.action.as_ref().unwrap();

    // TODO handle open+ed close+d grammar
    format!(
        "<b>{}</b> {} issue <a href=\"{}\">{}</a> on <a href=\"{}\">{}</a>\n",
        user_name, issue_action, issue_url, issue_title, project_url, project_name
    )
}
