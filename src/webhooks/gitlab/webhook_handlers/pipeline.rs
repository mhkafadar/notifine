use crate::http_server::GitlabEvent;

pub fn handle_pipeline_event(gitlab_event: &GitlabEvent) -> String {
    let project = &gitlab_event.project;
    let project_name = &project.name;
    let project_url = &project.web_url;
    let user_name = &gitlab_event.user.as_ref().unwrap().username;
    let pipeline = &gitlab_event.object_attributes.as_ref().unwrap();
    let pipeline_status = &pipeline.status.as_ref().unwrap();
    let pipeline_source = &pipeline.source.as_ref().unwrap();
    let pipeline_id = &pipeline.id.to_string();

    format!(
        "<b>{user_name}</b> {pipeline_status} \
        pipeline <a href=\"{project_url}/-/pipelines/{pipeline_id}\">\
        {pipeline_id}</a> on <a href=\"{project_url}\">{project_name}</a>\n",
    )
}
