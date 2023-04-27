use notifine::get_webhook_url_or_create;
use std::env;
use teloxide::dispatching::dialogue;
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::dptree::case;
use teloxide::prelude::*;
use teloxide::types::{ChatMemberKind, ParseMode};
use teloxide::utils::command::BotCommands;
use teloxide::{filter_command};

type MyDialogue = Dialogue<State, InMemStorage<State>>;

pub async fn run_gitlab_bot() {
    log::info!("Starting bot...");

    let bot = create_new_bot();

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

async fn handle_start_command(_bot: Bot, _dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
    log::info!("Start command received");
    let inviter_username = match msg.from() {
        Some(user) => user.username.clone(),
        None => None,
    };

    handle_new_chat_and_start_command(
        msg.chat
            .id
            .to_string()
            .parse::<i64>()
            .expect("Error parsing chat id"),
        inviter_username,
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

async fn handle_new_message(_bot: Bot, message: Message) -> ResponseResult<()> {
    let chat_id = message.chat.id.0;

    if let Some(text) = message.text() {
        log::info!("Received message from {}: {}", chat_id, text);
    }

    log::warn!("{:#?}", message.via_bot);
    Ok(())
}

async fn handle_my_chat_member_update(_bot: Bot, update: ChatMemberUpdated) -> ResponseResult<()> {
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
        handle_new_chat_and_start_command(chat_id, update.from.username).await?
    }

    log::info!(
        "Received a chat member update from {}: {:?}",
        chat_id,
        update.new_chat_member
    );
    Ok(())
}

pub async fn send_message_gitlab(chat_id: i64, message: String) -> ResponseResult<()> {
    log::info!("Sending message to {}: {}", chat_id, message);
    let bot = create_new_bot();

    let chat_id = ChatId(chat_id);

    bot.send_message(chat_id, message)
        .parse_mode(ParseMode::Html)
        .send()
        .await?;
    Ok(())
}

async fn handle_new_chat_and_start_command(
    telegram_chat_id: i64,
    inviter_username: Option<String>,
) -> ResponseResult<()> {
    let webhook_url = get_webhook_url_or_create(telegram_chat_id);

    let message = if webhook_url.0.is_empty() {
        log::error!("Error creating or getting webhook: {:?}", webhook_url);
        "Hi there!\
                      Our bot is curently has some problems \
                      Please create a github issue here: \
                      https://github.com/mhkafadar/notifine/issues/new"
            .to_string()
    } else {
        format!(
            "Hi there! \
                      To setup notifications for \
                      this chat your GitLab project(repo), \
                      open Settings -> Webhooks and add this \
                      URL: {}/gitlab/{}",
            env::var("WEBHOOK_BASE_URL").expect("WEBHOOK_BASE_URL must be set"),
            webhook_url.0
        )
    };

    send_message_gitlab(telegram_chat_id, message).await?;

    if webhook_url.1 {
        let inviter_username = match inviter_username {
            Some(username) => username,
            None => "unknown".to_string(),
        };

        // send message to admin on telegram and inform new install
        send_message_gitlab(
            env::var("TELEGRAM_ADMIN_CHAT_ID")
                .expect("TELEGRAM_ADMIN_CHAT_ID must be set")
                .parse::<i64>()
                .expect("Error parsing TELEGRAM_ADMIN_CHAT_ID"),
            format!("New gitlab webhook added: {telegram_chat_id} by @{inviter_username}"),
        )
        .await?;
    }

    Ok(())
}

fn create_new_bot() -> Bot {
    Bot::new(env::var("GITLAB_TELOXIDE_TOKEN").expect("GITLAB_TELOXIDE_TOKEN must be set"))
}
