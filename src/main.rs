use teloxide::prelude::*;

pub mod handler;

use crate::{
    handler::handle,
};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    run().await;
}

async fn run() {
    log::info!("Starting bot...");

    let bot = Bot::from_env();

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    log::info!("Closing bot... Goodbye!");
}
