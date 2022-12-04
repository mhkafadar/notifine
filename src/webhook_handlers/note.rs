use crate::http_server::GitlabEvent;

pub fn handle_note_event(gitlab_event: &GitlabEvent) -> String {
    let project = &gitlab_event.project;
    let project_name = &project.name;
    let project_url = &project.homepage;
    let user_name = &gitlab_event.user.as_ref().unwrap().name;
    let note = &gitlab_event.object_attributes.as_ref().unwrap();
    let note_url = &note.url;

    // TODO handle comments other than issue comments
    format!(
        "<b>{}</b> <a href=\"{}\">commented on an issue</a> on <a href=\"{}\">{}</a>\n",
        user_name, note_url, project_url, project_name
    )
}
