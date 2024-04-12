// use crate::utils::telegram_admin::send_message_to_admin;
// use notifine::get_webhook_url_or_create;
// use std::env;
// use teloxide::dispatching::dialogue;
// use teloxide::dispatching::dialogue::InMemStorage;
// use teloxide::dptree::case;
// use teloxide::filter_command;
// use teloxide::prelude::*;
// use teloxide::types::{ChatMemberKind, ParseMode};
// use teloxide::utils::command::BotCommands;
//
// type MyDialogue = Dialogue<State, InMemStorage<State>>;
//
// pub async fn run_github_bot() {
//     log::info!("Starting bot...");
//
//     let bot = create_new_bot();
//
//     let command_handler =
//         filter_command::<Command, _>().branch(case![Command::Start].endpoint(handle_start_command));
//
//     let message_handler = dptree::entry()
//         .branch(
//             Update::filter_message()
//                 .branch(command_handler)
//                 .branch(case![State::ReceiveBotReview].endpoint(handle_bot_review))
//                 .branch(dptree::endpoint(handle_new_message)),
//         )
//         .branch(Update::filter_my_chat_member().endpoint(handle_my_chat_member_update));
//
//     let handler =
//         dialogue::enter::<Update, InMemStorage<State>, State, _>().branch(message_handler);
//
//     Dispatcher::builder(bot, handler)
//         .dependencies(dptree::deps![InMemStorage::<State>::new()])
//         .enable_ctrlc_handler()
//         .build()
//         .dispatch()
//         .await;
//
//     log::info!("Closing bot... Goodbye!");
// }
//
// #[derive(Clone, Default)]
// pub enum State {
//     #[default]
//     Start,
//     ReceiveBotReview,
// }
//
// #[derive(BotCommands, Clone)]
// #[command(
//     rename_rule = "lowercase",
//     description = "These commands are supported:"
// )]
// enum Command {
//     #[command(description = "starts!")]
//     Start,
// }
//
// // async fn handle_start_command(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
// //     log::info!("Start command received");
// //     bot.send_message(msg.chat.id, "What do you think about our bot?")
// //         .await?;
// //     dialogue.update(State::ReceiveBotReview).await.unwrap();
// //     Ok(())
// // }
//
// async fn handle_start_command(msg: Message) -> ResponseResult<()> {
//     log::info!("Start command received");
//     handle_new_chat_and_start_command(
//         msg.chat
//             .id
//             .to_string()
//             .parse::<i64>()
//             .expect("Error parsing chat id"),
//     )
//     .await?;
//
//     Ok(())
// }
//
// async fn handle_bot_review(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
//     log::info!("Bot review received");
//     let chat_id = msg.chat.id;
//     // let message = msg.text().unwrap();
//     bot.send_message(chat_id, "Thanks.").await?;
//     dialogue.exit().await.unwrap();
//
//     Ok(())
// }
//
// async fn handle_new_message(message: Message) -> ResponseResult<()> {
//     let chat_id = message.chat.id.0;
//
//     if let Some(text) = message.text() {
//         log::info!("Received message from {}: {}", chat_id, text);
//     }
//
//     log::warn!("{:#?}", message.via_bot);
//     Ok(())
// }
//
// async fn handle_my_chat_member_update(update: ChatMemberUpdated) -> ResponseResult<()> {
//     let chat_id = update.chat.id.0;
//
//     log::info!(
//         "Received chat member update from {}: {:#?} {:#?}",
//         chat_id,
//         update.old_chat_member,
//         update.new_chat_member
//     );
//
//     // bot joining a group or a new private chat
//     if update.old_chat_member.kind == ChatMemberKind::Left
//         && update.new_chat_member.kind == ChatMemberKind::Member
//     {
//         handle_new_chat_and_start_command(chat_id).await?
//     }
//
//     log::info!(
//         "Received a chat member update from {}: {:?}",
//         chat_id,
//         update.new_chat_member
//     );
//     Ok(())
// }
//
// pub async fn send_message_github(chat_id: i64, message: String) -> ResponseResult<()> {
//     log::info!("Sending message to {}: {}", chat_id, message);
//     let bot = create_new_bot();
//
//     let chat_id = ChatId(chat_id);
//
//     bot.send_message(chat_id, message)
//         .disable_web_page_preview(true)
//         .parse_mode(ParseMode::Html)
//         .await?;
//     Ok(())
// }
//
// async fn handle_new_chat_and_start_command(telegram_chat_id: i64) -> ResponseResult<()> {
//     let webhook_info = get_webhook_url_or_create(telegram_chat_id);
//
//     let message = if webhook_info.webhook_url.is_empty() {
//         log::error!("Error creating or getting webhook: {:?}", webhook_info);
//         "Hi there!\
//                       Our bot is curently has some problems \
//                       Please create a github issue here: \
//                       https://github.com/mhkafadar/notifine/issues/new"
//             .to_string()
//     } else {
//         format!(
//             "Hi there! \
//                       To setup notifications for \
//                       this chat your Github project(repo), \
//                       open Settings -> Webhooks and add this \
//                       URL: {}/github/{}",
//             env::var("WEBHOOK_BASE_URL").expect("WEBHOOK_BASE_URL must be set"),
//             webhook_info.webhook_url
//         )
//     };
//
//     send_message_github(telegram_chat_id, message).await?;
//
//     if webhook_info.is_new {
//         // send message to admin on telegram and inform new install
//         send_message_to_admin(
//             &create_new_bot(),
//             format!("New github webhook added: {telegram_chat_id}"),
//         )
//         .await?;
//     }
//
//     Ok(())
// }
//
// pub fn create_new_bot() -> Bot {
//     Bot::new(env::var("GITHUB_TELOXIDE_TOKEN").expect("GITHUB_TELOXIDE_TOKEN must be set"))
// }
