use notifine::db::DbPool;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree;
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;

mod flows;
mod handlers;
mod keyboards;
mod types;
mod utils;

use handlers::{callback_handler, command_handler, message_handler, Command};

pub async fn run_bot(pool: DbPool, token: String) {
    tracing::info!("Starting Agreement bot...");

    let bot = Bot::new(token);

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint({
                    let pool = pool.clone();
                    move |bot: Bot, msg: Message, cmd: Command| {
                        let pool = pool.clone();
                        async move { command_handler(bot, msg, cmd, pool).await }
                    }
                }),
        )
        .branch(Update::filter_message().endpoint({
            let pool = pool.clone();
            move |bot: Bot, msg: Message| {
                let pool = pool.clone();
                async move { message_handler(bot, msg, pool).await }
            }
        }))
        .branch(Update::filter_callback_query().endpoint({
            let pool = pool.clone();
            move |bot: Bot, q: CallbackQuery| {
                let pool = pool.clone();
                async move { callback_handler(bot, q, pool).await }
            }
        }));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
