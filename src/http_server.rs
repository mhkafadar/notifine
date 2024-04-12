use crate::webhooks::beep::http_server::handle_beep_webhook;
use crate::webhooks::github::http_server::handle_github_webhook;
use crate::webhooks::gitlab::http_server::handle_gitlab_webhook;
// use crate::webhooks::trello::http_server::handle_trello_callback;
use actix_web::{get, middleware, App, HttpServer, Responder};

pub async fn run_http_server() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            .service(health)
            .service(handle_gitlab_webhook)
            .service(handle_github_webhook)
            .service(handle_beep_webhook)
            // .service(handle_trello_callback)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[get("/health")]
async fn health() -> impl Responder {
    log::info!("Health check");
    "I'm ok"
}
