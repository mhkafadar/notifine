// use crate::webhooks::gitlab::webhook_handlers::new_branch_push::new_branch_push;
// use actix_web::web;
// use serde::Deserialize;
// use ureq::serde_json;
//
// #[derive(Debug, Deserialize)]
// struct PipelineEvent {
//     #[serde(rename = "ref")]
//     pub ref_field: String,
//     project: Project,
//     user: User,
// }
//
// #[derive(Debug, Deserialize)]
// struct User {
//     name: String,
// }
//
// #[derive(Debug, Deserialize)]
// struct Project {
//     name: String,
//     homepage: String,
// }
//
// pub fn handle_pipeline_event(body: &web::Bytes) -> String {
//     let pipeline_event: PipelineEvent = serde_json::from_slice(body).unwrap();
//
//     let tag_name = tag_push_event.ref_field.trim_start_matches("refs/tags/");
//     let project_name = &tag_push_event.project.name;
//     let project_url = &tag_push_event.project.homepage;
//
//     let tag_url = &format!("{}/-/tree/{}", project_url, tag_name);
//     let sender = &tag_push_event.user_name;
//
//     format!(
//         "<b>{sender}</b> pushed a new tag <a href=\"{tag_url}\">{tag_name}</a> to <a href=\"{project_url}\">{project_name}</a>\n",
//     )
// }
//
// // use crate::webhooks::gitlab::http_server::GitlabEvent;
// //
// // pub fn handle_pipeline_event(gitlab_event: &GitlabEvent) -> String {
// //     let project = &gitlab_event.project;
// //     let project_name = &project.name;
// //     let project_url = &project.web_url;
// //     let user_name = &gitlab_event.user.as_ref().unwrap().username;
// //     let pipeline = &gitlab_event.object_attributes.as_ref().unwrap();
// //     let pipeline_status = &pipeline.status.as_ref().unwrap();
// //     let pipeline_source = &pipeline.source.as_ref().unwrap();
// //     let pipeline_id = &pipeline.id.to_string();
// //
// //     format!(
// //         "<b>{user_name}</b> {pipeline_status} \
// //         pipeline <a href=\"{project_url}/-/pipelines/{pipeline_id}\">\
// //         {pipeline_id}</a> on <a href=\"{project_url}\">{project_name}</a>\n",
// //     )
// // }
