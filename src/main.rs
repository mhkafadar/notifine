use crate::bots::gitlab_bot::run_gitlab_bot;
use crate::bots::trello_bot::run_trello_bot;
use crate::http_server::run_http_server;
use dotenv::dotenv;
use tokio::task;

pub mod bots;
pub mod http_server;
pub mod webhooks;

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();

    task::spawn(run_gitlab_bot());
    task::spawn(run_trello_bot());
    run_http_server().await.expect("Http server error");

    log::info!("Main 2");
}
