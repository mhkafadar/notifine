use dotenv::dotenv;
use notifine::{create_chat, create_webhook};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::env;
use std::ops::Add;
use teloxide::dispatching::dialogue;
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::dptree::case;
use teloxide::filter_command;
use teloxide::prelude::*;
use teloxide::types::{ChatMemberKind, ParseMode};
use teloxide::utils::command::BotCommands;

type MyDialogue = Dialogue<State, InMemStorage<State>>;

pub async fn run_telegram_bot() {
    log::info!("Starting bot...");

    let bot = Bot::from_env();

    let command_handler =
        filter_command::<Command, _>().branch(case![Command::Start].endpoint(handle_start_command));

    let message_handler = dptree::entry()
        .branch(
            Update::filter_message()
                .branch(command_handler)
                .branch(case![State::ReceiveBotReview].endpoint(handle_bot_review))
                .branch(dptree::endpoint(handle_new_message)),
        )
        .branch(Update::filter_my_chat_member().endpoint(handle_my_chat_member_update));

    let handler =
        dialogue::enter::<Update, InMemStorage<State>, State, _>().branch(message_handler);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![InMemStorage::<State>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    log::info!("Closing bot... Goodbye!");
}

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    ReceiveBotReview,
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "starts!")]
    Start,
}

// async fn handle_start_command(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
//     log::info!("Start command received");
//     bot.send_message(msg.chat.id, "What do you think about our bot?")
//         .await?;
//     dialogue.update(State::ReceiveBotReview).await.unwrap();
//     Ok(())
// }

async fn handle_start_command(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
    log::info!("Start command received");
    let random_string = create_random_string();
    let chat = create_chat(msg.chat.id.to_string().as_str(), "new_chat", &random_string);
    create_webhook(&random_string, &random_string, chat.id);
    dotenv().ok();

    send_message(
        msg.chat
            .id
            .to_string()
            .parse::<i64>()
            .expect("Error parsing chat id"),
        format!(
            "Hi there! \
                      To setup notifications for \
                      this chat your GitLab project(repo), \
                      open Settings -> Webhooks and add this \
                      URL: {}/gitlab/{}",
            env::var("WEBHOOK_BASE_URL").expect("WEBHOOK_BASE_URL must be set"),
            random_string
        ),
    )
    .await?;
    Ok(())
}

async fn handle_bot_review(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
    log::info!("Bot review received");
    let chat_id = msg.chat.id;
    // let message = msg.text().unwrap();
    bot.send_message(chat_id, "Thanks.").await?;
    dialogue.exit().await.unwrap();

    Ok(())
}

async fn handle_new_message(bot: Bot, message: Message) -> ResponseResult<()> {
    let chat_id = message.chat.id.0;

    if let Some(text) = message.text() {
        log::info!("Received message from {}: {}", chat_id, text);
    }

    log::warn!("{:#?}", message.via_bot);
    Ok(())
}

async fn handle_my_chat_member_update(bot: Bot, update: ChatMemberUpdated) -> ResponseResult<()> {
    let chat_id = update.chat.id.0;

    log::info!(
        "Received chat member update from {}: {:#?} {:#?}",
        chat_id,
        update.old_chat_member,
        update.new_chat_member
    );

    // bot joining a group or a new private chat
    if update.old_chat_member.kind == ChatMemberKind::Left
        && update.new_chat_member.kind == ChatMemberKind::Member
    {
        let random_string = create_random_string();
        let chat = create_chat(chat_id.to_string().as_str(), "new_chat", &random_string);
        create_webhook(&random_string, &random_string, chat.id);
        dotenv().ok();

        send_message(
            chat_id,
            format!(
                "Hi there!\
                      To setup notifications for \
                      this chat your GitLab project(repo), \
                      open Settings -> Webhooks and add this \
                      URL: {}/gitlab/{}",
                env::var("WEBHOOK_BASE_URL").expect("WEBHOOK_BASE_URL must be set"),
                random_string
            ),
        )
        .await?;
    }

    log::info!(
        "Received a chat member update from {}: {:?}",
        chat_id,
        update.new_chat_member
    );
    Ok(())
}

pub async fn send_message(chat_id: i64, message: String) -> ResponseResult<()> {
    log::info!("Sending message to {}: {}", chat_id, message);
    let bot = Bot::from_env();

    let chat_id = ChatId(chat_id);

    bot.send_message(chat_id, message)
        .parse_mode(ParseMode::Html)
        .send()
        .await?;
    Ok(())
}

fn create_random_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}
