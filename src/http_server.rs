use crate::webhooks::beep::http_server::handle_beep_webhook;
use crate::webhooks::github::http_server::handle_github_webhook;
use crate::webhooks::gitlab::http_server::handle_gitlab_webhook;
use actix_web::{get, middleware, App, HttpServer, Responder};
use std::env;

pub async fn run_http_server() -> std::io::Result<()> {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a valid number");

    log::info!("Starting HTTP server on port {}", port);

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            .service(health)
            .service(handle_gitlab_webhook)
            .service(handle_github_webhook)
            .service(handle_beep_webhook)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}

#[get("/health")]
async fn health() -> impl Responder {
    log::info!("Health check");
    "I'm ok"
}
