use actix_web::{get, middleware, post, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use telegram_gitlab::{find_chat_by_id, find_webhook_by_webhook_url};

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
    title: String,
    message: String,
    author: Author,
    timestamp: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct Author {
    name: String,
    email: String,
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

#[derive(Deserialize)]
struct GitlabWebhook {
    webhook_url: String,
}

#[post("/gitlab/{webhook_url}")]
async fn handle_gitlab_webhook(
    gitlab_webhook: web::Path<GitlabWebhook>,
    gitlab_event: web::Json<GitlabEvent>,
) -> impl Responder {
    let branch_ref = &gitlab_event.r#ref;
    let branch_name = branch_ref.split('/').last().unwrap();
    let project = &gitlab_event.project;

    // replace - with \- to avoid error in telegram markdown
    let project_name = &project.name;
    let project_url = &project.homepage;

    // create a paragprah with all commits, include committer name, commit message and commit url
    let mut commit_paragraph = String::new();
    for commit in &gitlab_event.commits {
        log::info!("Commit: {}", commit.message);
        log::info!("Commit url: {}", commit.url);
        log::info!("Commit author: {}", commit.author.name);

        let commit_url = &commit.url;
        let commit_message = &commit.message.trim_end();
        let commit_author_name = &commit.author.name;

        // commit_paragraph.push_str(&format!("{}: [{}]({}) to [{}:{}]({})\n", commit_author_name, commit_message, commit_url, project_name, branch_name, project_url));
        commit_paragraph.push_str(&format!(
            "{}: <a href=\"{}\">{}</a> to <a href=\"{}\">{}:{}</a>\n",
            commit_author_name, commit_url, commit_message, project_url, project_name, branch_name
        ));
    }

    let webhook_url = &gitlab_webhook.webhook_url;
    log::info!("webhook_url: {}", webhook_url);
    let webhook = find_webhook_by_webhook_url(webhook_url);

    if webhook.is_none() {
        log::error!("Webhook not found");
        return HttpResponse::NotFound();
    }
    let webhook = webhook.unwrap();

    // log chat_id
    log::info!("Webhook: {}", webhook.webhook_url);
    let chat_id = webhook.chat_id.expect("Chat id must be set");
    log::info!("Chat id: {}", chat_id);

    let chat = find_chat_by_id(webhook.chat_id.expect("Chat id must be set"));

    if chat.is_none() {
        log::error!("Chat not found");
        return HttpResponse::NotFound();
    }
    let chat = chat.unwrap();

    crate::telegram_bot::send_message(
        chat.telegram_id
            .parse::<i64>()
            .expect("CHAT_ID must be an integer"),
        commit_paragraph,
    )
    .await
    .unwrap();

    log::info!("bot sent message");
    HttpResponse::Ok() // <- send response
}
