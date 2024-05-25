use crate::bots::bot_service::{StartCommand, TelegramMessage};
use crate::services::uptime_checker::check_health;
use crate::utils::telegram_admin::send_message_to_admin;
use notifine::{
    create_chat, create_health_url, find_chat_by_telegram_chat_id,
    get_health_url_by_chat_id_and_url, CreateChatInput,
};
use reqwest::Client;
use std::env;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree;
use teloxide::macros::BotCommands;
use teloxide::prelude::*;
use teloxide::types::{ChatMemberKind, ParseMode};
use url::Url;

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Starts the bot.")]
    Start,
    #[command(description = "Add a new health check endpoint.")]
    New(String),
    #[command(description = "List all health check endpoints.")]
    List,
    #[command(description = "Delete a health check endpoint.")]
    Delete,
    #[command(description = "Show help information.")]
    Help,
}

async fn command_handler(bot: Bot, msg: Message, command: Command) -> ResponseResult<()> {
    let inviter_username = match msg.from() {
        Some(user) => user.username.clone(),
        None => None,
    };

    let thread_id = msg.thread_id;
    match command {
        Command::Start => {
            handle_new_chat_and_start_command(StartCommand {
                chat_id: msg.chat.id.0,
                thread_id,
                inviter_username,
            })
            .await?
        }
        Command::New(health_url) => {
            handle_new_health_url(health_url, msg.chat.id.0, thread_id, inviter_username).await?
        }
        Command::List => {
            // Example response, implement your own logic here
            bot.send_message(msg.chat.id, "Listing all endpoints...")
                .await?;
        }
        Command::Delete => {
            // Example response, implement your own logic here
            bot.send_message(msg.chat.id, "An endpoint has been deleted.")
                .await?;
        }
        Command::Help => {
            let help_text = "Commands available:\n/start\n/new\n/list\n/delete\n/help";
            bot.send_message(msg.chat.id, help_text)
                .parse_mode(ParseMode::MarkdownV2)
                .await?;
        }
    };

    Ok(())
}

async fn handle_new_health_url(
    health_url: String,
    telegram_chat_id: i64,
    thread_id: Option<i32>,
    inviter_username: Option<String>,
) -> ResponseResult<()> {
    let parsed_url = Url::parse(&health_url);
    if parsed_url.is_err() || health_url.trim().is_empty() {
        send_telegram_message(TelegramMessage {
            chat_id: telegram_chat_id,
            thread_id,
            message: "Please provide a valid URL. Send a message like: '/new https://example.com"
                .to_string(),
        })
        .await?;
        return Ok(());
    }

    let chat = find_chat_by_telegram_chat_id(&telegram_chat_id.to_string());
    if chat.is_none() {
        send_telegram_message(TelegramMessage {
            chat_id: telegram_chat_id,
            thread_id,
            message: "You should call start command first to initialize the bot.".to_string(),
        })
        .await?;
        return Ok(());
    }

    let chat_id = chat.unwrap().id;

    if let Some(existing_health_url) =
        get_health_url_by_chat_id_and_url(chat_id as i64, &health_url)
    {
        send_telegram_message(TelegramMessage {
            chat_id: telegram_chat_id,
            thread_id,
            message: format!(
                "This endpoint has already been added: {}",
                existing_health_url.url
            ),
        })
        .await?;
        return Ok(());
    }

    let client = Client::new();
    let health_result = check_health(&client, &health_url).await;

    if !health_result.success {
        send_telegram_message(TelegramMessage {
            chat_id: telegram_chat_id,
            thread_id,
            message: format!(
                "Error! Endpoint should return success status code (2xx) on first check to be added.\n\n\
                Failed to add new health check endpoint: {}\nStatus code: {}\nResponse time: {:?}",
                health_url, health_result.status_code, health_result.duration
            ),
        })
        .await?;
        return Ok(());
    }

    let message = format!("New health check endpoint added: {}", health_url);

    let new_health_url = create_health_url(&health_url, chat_id, health_result.status_code as i32);

    send_telegram_message(TelegramMessage {
        chat_id: telegram_chat_id,
        thread_id,
        message,
    })
    .await?;

    let inviter_username = inviter_username.unwrap_or_else(|| "unknown".to_string());

    let token = env::var("UPTIME_TELOXIDE_TOKEN").expect("UPTIME_TELOXIDE_TOKEN must be set");
    let bot = Bot::new(token);

    send_message_to_admin(
        &bot,
        format!(
            "New health check endpoint added: {} by @{inviter_username}",
            new_health_url.url
        ),
        10,
    )
    .await?;

    Ok(())
}

async fn chat_member_handler(update: ChatMemberUpdated) -> ResponseResult<()> {
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
        handle_new_chat_and_start_command(StartCommand {
            chat_id,
            thread_id: None,
            inviter_username: update.from.username,
        })
        .await?
    }

    Ok(())
}

pub async fn send_telegram_message(message: TelegramMessage) -> ResponseResult<()> {
    let TelegramMessage {
        chat_id,
        thread_id,
        message,
    } = message;

    let token = env::var("UPTIME_TELOXIDE_TOKEN").expect("UPTIME_TELOXIDE_TOKEN must be set");

    let bot = Bot::new(token);
    let chat_id = ChatId(chat_id);

    let mut request = bot
        .send_message(chat_id, &message)
        .parse_mode(ParseMode::Html);

    if let Some(tid) = thread_id {
        request = request.message_thread_id(tid);
    }

    request.await?;

    Ok(())
}

async fn handle_new_chat_and_start_command(start_command: StartCommand) -> ResponseResult<()> {
    let StartCommand {
        chat_id,
        thread_id,
        inviter_username,
    } = start_command;
    let bot_name = "Uptime";

    let thread_id_str = thread_id.map(|tid| tid.to_string());
    let thread_id_ref = thread_id_str.as_ref().map(String::as_str);

    let existing_chat = find_chat_by_telegram_chat_id(&chat_id.to_string());

    if existing_chat.is_none() {
        let _chat = create_chat(CreateChatInput {
            name: "uptime",
            telegram_chat_id: &chat_id.to_string(),
            telegram_thread_id: thread_id_ref,
            webhook_url: "-", // make it optional in a later migration
        });
    }

    let message = format!(
        "Hi there!\
            \nI am the {bot_name} bot.\
            \nI can help you monitor your website uptime.\
            \n\nHere are the available commands:\
            \n/new - Add a new health check endpoint\
            \n/list - List all health check endpoints\
            \n/delete - Delete a health check endpoint\
            \n/help - Show this help message",
        bot_name = bot_name
    );

    send_telegram_message(TelegramMessage {
        chat_id,
        thread_id,
        message,
    })
    .await?;

    let inviter_username = inviter_username.unwrap_or_else(|| "unknown".to_string());

    let token = env::var("UPTIME_TELOXIDE_TOKEN").expect("UPTIME_TELOXIDE_TOKEN must be set");

    let bot = Bot::new(token);

    if existing_chat.is_none() {
        send_message_to_admin(
            &bot,
            format!("New {bot_name} /start command: {chat_id} by @{inviter_username}"),
            10,
        )
        .await?;
    }

    Ok(())
}
pub async fn run_bot() {
    log::info!("Starting bot...");

    let token = env::var("UPTIME_TELOXIDE_TOKEN").expect("UPTIME_TELOXIDE_TOKEN must be set");
    let bot = Bot::new(token);

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(command_handler),
        )
        .branch(Update::filter_my_chat_member().endpoint(chat_member_handler));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
