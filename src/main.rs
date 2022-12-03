use dotenv::dotenv;
use tokio::task;

pub mod http_server;
pub mod telegram_bot;
pub mod webhook_handlers;

use crate::{http_server::run_http_server, telegram_bot::run_telegram_bot};

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();

    task::spawn(run_telegram_bot());
    run_http_server().await.expect("Http server error");

    log::info!("Main 2");
}
