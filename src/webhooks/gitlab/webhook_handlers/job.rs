use actix_web::web;
use serde::Deserialize;
use ureq::serde_json;

#[derive(Debug, Deserialize)]
struct JobEvent {
    build_id: u64,
    build_name: String,
    build_status: String,
    build_duration: f64,
    repository: Repository,
    user: User,
}

#[derive(Debug, Deserialize)]
struct User {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Repository {
    homepage: String,
}

// TODO also implement environment name

pub fn handle_job_event(body: &web::Bytes) -> String {
    let job_event: JobEvent = serde_json::from_slice(body).unwrap();
    // convert build_duration to seconds remove decimal places
    let build_duration = job_event.build_duration as u64;
    let build_url = &format!(
        "{}/-/jobs/{}",
        job_event.repository.homepage, job_event.build_id
    );
    let build_name = &job_event.build_name;

    if job_event.build_status == "success" {
        format!(
            "✅ CI: <a href=\"{build_url}\">{build_name}</a> succeeded after <b>{build_duration}</b> seconds"
        )
    } else if job_event.build_status == "failed" {
        format!(
            "❌ CI: <a href=\"{build_url}\">{build_name}</a> failed after <b>{build_duration}</b> seconds"
        )
    } else if job_event.build_status == "canceled" {
        let user_name = &job_event.user.name;
        format!(
            "❌ CI: <a href=\"{build_url}\">{build_name}</a> was canceled by {user_name} after <b>{build_duration}</b> seconds"
        )
    } else {
        String::new()
    }
}
