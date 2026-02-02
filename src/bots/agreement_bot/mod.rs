use notifine::db::DbPool;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree;
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, ChatMemberUpdated};

mod flows;
mod handlers;
mod keyboards;
mod types;
pub mod utils;

use handlers::{callback_handler, chat_member_handler, command_handler, message_handler, Command};

pub async fn run_bot(pool: DbPool, token: String, admin_chat_id: Option<i64>) {
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
                        async move { command_handler(bot, msg, cmd, pool, admin_chat_id).await }
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
        }))
        .branch(Update::filter_my_chat_member().endpoint({
            let pool = pool.clone();
            move |bot: Bot, update: ChatMemberUpdated| {
                let pool = pool.clone();
                async move { chat_member_handler(bot, update, pool).await }
            }
        }));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
