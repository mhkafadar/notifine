use teloxide::prelude::*;

pub async fn run_telegram_bot() {
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


async fn handle(bot: Bot, message: Message) -> ResponseResult<()> {
    let chat_id = message.chat.id.0;
    log::info!("Received a message from {}: {}", chat_id, message.text().unwrap());
    Ok(())
}

pub async fn send_message(chat_id: i64, message: String) -> ResponseResult<()> {
    log::info!("Sending message to {}: {}", chat_id, message);
    let bot = Bot::from_env();

    let chat_id = ChatId(chat_id);

    bot.send_message(chat_id, message)
        .send()
        .await?;
    Ok(())
}

