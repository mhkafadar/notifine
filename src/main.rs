use tokio::task;
use dotenv::dotenv;

pub mod telegram_bot;
pub mod http_server;

use crate::{
    telegram_bot::run_telegram_bot,
    http_server::run_http_server,
};

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();

    task::spawn(run_telegram_bot());
    run_http_server().await.expect("Http server error");

    log::info!("Main 2");
}

