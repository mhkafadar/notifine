use actix_web::{get, post, App, HttpResponse, HttpServer, middleware, Responder, web};
use serde::{Deserialize};

#[derive(Debug, Deserialize)]
struct GitlabEvent {
    object_kind: String,
    event_name: String,
    before: String,
    after: String,
    r#ref: String,
    checkout_sha: String,
    message: Option<String>,
    user_id: u32,
    user_name: String,
    user_username: String,
    user_email: String,
    user_avatar: String,
    project_id: u32,
    project: Project,
    repository: Repository,
    commits: Vec<Commit>,
    total_commits_count: u32,
}

#[derive(Debug, Deserialize)]
struct Project {
    id: u32,
    name: String,
    description: String,
    web_url: String,
    avatar_url: Option<String>,
    git_ssh_url: String,
    git_http_url: String,
    namespace: String,
    visibility_level: u32,
    path_with_namespace: String,
    default_branch: String,
    ci_config_path: Option<String>,
    homepage: String,
    url: String,
    ssh_url: String,
    http_url: String,
}

#[derive(Debug, Deserialize)]
struct Repository {
    name: String,
    url: String,
    description: String,
    homepage: String,
}

#[derive(Debug, Deserialize)]
struct Commit {
    id: String,
    message: String,
    timestamp: String,
    url: String,
}

pub async fn run_http_server() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            .service(health)
            .service(handle_gitlab_webhook)
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}

#[get("/health")]
async fn health() -> impl Responder {
    log::info!("Health check");
    "I'm ok"
}

#[post("/gitlab")]
async fn handle_gitlab_webhook(gitlab_event: web::Json<GitlabEvent>) -> impl Responder {
    let branch_ref = &gitlab_event.r#ref;
    let branch_name = branch_ref.split('/').last().unwrap();
    let project_name = &gitlab_event.project.name;
    let commit_message = &gitlab_event.commits[0].message;
    let commit_url = &gitlab_event.commits[0].url;

    let message = format!("{}: {} - {} - {}", project_name, branch_name, commit_message, commit_url);

    // TODO: chat_id should be dynamic (after adding database)
    let chat_id = std::env::var("CHAT_ID").expect("CHAT_ID must be set.");

    crate::telegram_bot::send_message(chat_id.parse::<i64>().expect("CHAT_ID must be an integer"), message).await.unwrap();

    log::info!("bot sent message");
    HttpResponse::Ok() // <- send response
}
