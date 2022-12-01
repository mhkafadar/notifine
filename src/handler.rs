use teloxide::{
    prelude::*,
    types::MessageKind::Common,
    utils::command::BotCommands
};

pub async fn handle(bot: Bot, message: Message) -> ResponseResult<()> {
    let chat_id = message.chat.id.0;
    log::info!("Received a message from {}: {}", chat_id, message.text().unwrap());
    Ok(())
}
