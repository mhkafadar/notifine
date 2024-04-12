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
// pub async fn run_gitlab_bot() {
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
// async fn handle_start_command(
//     _bot: Bot,
//     _dialogue: MyDialogue,
//     msg: Message,
// ) -> ResponseResult<()> {
//     log::info!("Start command received");
//     let inviter_username = match msg.from() {
//         Some(user) => user.username.clone(),
//         None => None,
//     };
//
//     let thread_id = msg.thread_id;
//
//     log::info!("Topic id: {:?}", thread_id);
//
//     handle_new_chat_and_start_command(StartCommand {
//         chat_id: msg.chat.id.0,
//         thread_id,
//         inviter_username,
//     })
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
// async fn handle_new_message(_bot: Bot, message: Message) -> ResponseResult<()> {
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
// async fn handle_my_chat_member_update(_bot: Bot, update: ChatMemberUpdated) -> ResponseResult<()> {
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
//         handle_new_chat_and_start_command(StartCommand {
//             chat_id,
//             thread_id: None,
//             inviter_username: update.from.username,
//         })
//         .await?
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
// pub struct TelegramMessage {
//     pub chat_id: i64,
//     pub thread_id: Option<i32>,
//     pub message: String,
// }
//
// pub async fn send_message_gitlab(message: TelegramMessage) -> ResponseResult<()> {
//     let TelegramMessage { chat_id, thread_id, message } = message;
//
//     log::info!("Sending message to {}: {}", chat_id, message);
//     let bot = create_new_bot();
//     let chat_id = ChatId(chat_id);
//
//     let mut request = bot.send_message(chat_id, &message).parse_mode(ParseMode::Html);
//
//     if let Some(tid) = thread_id {
//         request = request.message_thread_id(tid);
//     }
//
//     request.await?;
//
//     Ok(())
// }
//
// pub struct StartCommand {
//     pub chat_id: i64,
//     pub thread_id: Option<i32>,
//     pub inviter_username: Option<String>,
// }
//
// async fn handle_new_chat_and_start_command(
//     start_command: StartCommand
// ) -> ResponseResult<()> {
//     let StartCommand {
//         chat_id,
//         thread_id,
//         inviter_username,
//     } = start_command;
//
//     let webhook_info = get_webhook_url_or_create(chat_id);
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
//                       this chat your GitLab project(repo), \
//                       open Settings -> Webhooks and add this \
//                       URL: {}/gitlab/{}",
//             env::var("WEBHOOK_BASE_URL").expect("WEBHOOK_BASE_URL must be set"),
//             webhook_info.webhook_url
//         )
//     };
//
//      send_message_gitlab(TelegramMessage {
//           chat_id,
//           thread_id,
//           message,
//      }).await?;
//
//     if webhook_info.is_new {
//         let inviter_username = match inviter_username {
//             Some(username) => username,
//             None => "unknown".to_string(),
//         };
//
//         // send message to admin on telegram and inform new install
//         send_message_to_admin(
//             &create_new_bot(),
//             format!("New gitlab webhook added: {chat_id} by @{inviter_username}"),
//         )
//         .await?;
//     }
//
//     Ok(())
// }
//
// pub fn create_new_bot() -> Bot {
//     Bot::new(env::var("GITLAB_TELOXIDE_TOKEN").expect("GITLAB_TELOXIDE_TOKEN must be set"))
// }
