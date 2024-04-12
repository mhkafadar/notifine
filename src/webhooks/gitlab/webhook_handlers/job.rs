use actix_web::web;
use log::error;
use serde::Deserialize;
use ureq::serde_json;

#[derive(Debug, Deserialize)]
struct JobEvent {
    build_id: u64,
    build_name: String,
    build_status: String,
    build_duration: Option<f64>,
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
    let job_event = match serde_json::from_slice::<JobEvent>(body) {
        Ok(event) => event,
        Err(e) => {
            error!("Failed to deserialize JobEvent: {}", e);
            error!("Request Body: {}", String::from_utf8_lossy(body));
            return String::from("Error processing the job event");
        }
    };

    // convert build_duration to seconds remove decimal places
    let build_duration = job_event.build_duration.unwrap_or(0.0) as u64; // Default to 0 if null
    let build_url = &format!(
        "{}/-/jobs/{}",
        job_event.repository.homepage, job_event.build_id
    );
    let build_name = &job_event.build_name;

    match job_event.build_status.as_str() {
        "success" => format!(
            "✅ CI: <a href=\"{build_url}\">{build_name}</a> succeeded after <b>{build_duration}</b> seconds"
        ),
        "failed" => format!(
            "❌ CI: <a href=\"{build_url}\">{build_name}</a> failed after <b>{build_duration}</b> seconds"
        ),
        "canceled" => {
            let user_name = &job_event.user.name;
            format!(
                "❌ CI: <a href=\"{build_url}\">{build_name}</a> was canceled by {user_name} after <b>{build_duration}</b> seconds"
            )
        },
        _ => String::new(), // Return an empty string for unknown statuses
    }
}
