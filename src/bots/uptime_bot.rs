use crate::bots::bot_service::{StartCommand, TelegramMessage};
use crate::observability::alerts::Severity;
use crate::observability::{ALERTS, METRICS};
use crate::services::uptime_checker::check_health;
use crate::utils::telegram_admin::send_message_to_admin;
use notifine::db::DbPool;
use notifine::{
    create_chat, create_health_url, deactivate_chat, delete_health_url_by_id,
    find_chat_by_telegram_chat_id, get_health_url_by_chat_id_and_url, get_health_urls_by_chat_id,
    CreateChatInput,
};
use reqwest::Client;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree;
use teloxide::macros::BotCommands;
use teloxide::prelude::*;
use teloxide::types::{
    CallbackQuery, ChatMemberKind, InlineKeyboardButton, InlineKeyboardMarkup, ParseMode,
};
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
    #[command(description = "Delete a health check endpoint by ID.")]
    Delete(String),
    #[command(description = "Show help information.")]
    Help,
}

async fn command_handler(
    bot: Bot,
    msg: Message,
    command: Command,
    pool: DbPool,
) -> ResponseResult<()> {
    let inviter_username = match msg.from() {
        Some(user) => user.username.clone(),
        None => None,
    };

    let thread_id = msg.thread_id;
    match command {
        Command::Start => {
            handle_new_chat_and_start_command(
                &pool,
                &bot,
                StartCommand {
                    chat_id: msg.chat.id.0,
                    thread_id,
                    inviter_username,
                },
            )
            .await?
        }
        Command::New(health_url) => {
            handle_new_health_url(
                &pool,
                &bot,
                health_url,
                msg.chat.id.0,
                thread_id,
                inviter_username,
            )
            .await?
        }
        Command::List => {
            handle_list_endpoints(&pool, &bot, msg.chat.id.0, thread_id).await?;
        }
        Command::Delete(id_str) => {
            handle_delete_endpoint(&pool, &bot, msg.chat.id.0, thread_id, id_str).await?;
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
    pool: &DbPool,
    bot: &Bot,
    health_url: String,
    telegram_chat_id: i64,
    thread_id: Option<i32>,
    inviter_username: Option<String>,
) -> ResponseResult<()> {
    let parsed_url = Url::parse(&health_url);
    if parsed_url.is_err() || health_url.trim().is_empty() {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id: telegram_chat_id,
                thread_id,
                message:
                    "Please provide a valid URL. Send a message like: '/new https://example.com"
                        .to_string(),
            },
        )
        .await?;
        return Ok(());
    }

    let chat = match find_chat_by_telegram_chat_id(pool, &telegram_chat_id.to_string()) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find chat: {}", e),
                )
                .await;
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id: telegram_chat_id,
                    thread_id,
                    message: "Database error occurred. Please try again.".to_string(),
                },
            )
            .await?;
            return Ok(());
        }
    };

    let chat = match chat {
        Some(c) => c,
        None => {
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id: telegram_chat_id,
                    thread_id,
                    message: "You should call start command first to initialize the bot."
                        .to_string(),
                },
            )
            .await?;
            return Ok(());
        }
    };

    let existing_health_url =
        match get_health_url_by_chat_id_and_url(pool, chat.id as i64, &health_url) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("Database error: {:?}", e);
                METRICS.increment_errors();
                ALERTS
                    .send_alert(
                        bot,
                        Severity::Error,
                        "Database",
                        &format!("Failed to check existing health URL: {}", e),
                    )
                    .await;
                None
            }
        };

    if let Some(existing) = existing_health_url {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id: telegram_chat_id,
                thread_id,
                message: format!("This endpoint has already been added: {}", existing.url),
            },
        )
        .await?;
        return Ok(());
    }

    let client = Client::new();
    let health_result = check_health(&client, &health_url).await;

    if !health_result.success {
        send_telegram_message(bot, TelegramMessage {
            chat_id: telegram_chat_id,
            thread_id,
            message: format!(
                "Error! Endpoint should return success status code (2xx) on first check to be added.\n\n\
                Failed to add new health check endpoint: {}\nStatus code: {}\nResponse time: {:.2}s",
                health_url, health_result.status_code, health_result.duration.as_secs_f64()
            ),
        })
        .await?;
        return Ok(());
    }

    let message = format!("New health check endpoint added: {}", health_url);

    let new_health_url =
        match create_health_url(pool, &health_url, chat.id, health_result.status_code as i32) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("Failed to create health URL: {:?}", e);
                METRICS.increment_errors();
                ALERTS
                    .send_alert(
                        bot,
                        Severity::Error,
                        "Database",
                        &format!("Failed to create health URL: {}", e),
                    )
                    .await;
                send_telegram_message(
                    bot,
                    TelegramMessage {
                        chat_id: telegram_chat_id,
                        thread_id,
                        message: "Failed to save the health check endpoint.".to_string(),
                    },
                )
                .await?;
                return Ok(());
            }
        };

    send_telegram_message(
        bot,
        TelegramMessage {
            chat_id: telegram_chat_id,
            thread_id,
            message,
        },
    )
    .await?;

    let inviter_username = inviter_username.unwrap_or_else(|| "unknown".to_string());

    send_message_to_admin(
        bot,
        format!(
            "New health check endpoint added: {} by @{inviter_username}",
            new_health_url.url
        ),
        10,
    )
    .await?;

    Ok(())
}

async fn handle_list_endpoints(
    pool: &DbPool,
    bot: &Bot,
    telegram_chat_id: i64,
    thread_id: Option<i32>,
) -> ResponseResult<()> {
    let chat = match find_chat_by_telegram_chat_id(pool, &telegram_chat_id.to_string()) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find chat in list endpoints: {}", e),
                )
                .await;
            let mut request = bot.send_message(
                ChatId(telegram_chat_id),
                "Database error occurred. Please try again later.",
            );
            if let Some(tid) = thread_id {
                request = request.message_thread_id(tid);
            }
            request.await?;
            return Ok(());
        }
    };

    let chat = match chat {
        Some(c) => c,
        None => {
            let mut request = bot.send_message(
                ChatId(telegram_chat_id),
                "You should call /start command first to initialize the bot.",
            );
            if let Some(tid) = thread_id {
                request = request.message_thread_id(tid);
            }
            request.await?;
            return Ok(());
        }
    };

    let health_urls = match get_health_urls_by_chat_id(pool, chat.id as i64) {
        Ok(urls) => urls,
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to get health URLs: {}", e),
                )
                .await;
            let mut request = bot.send_message(
                ChatId(telegram_chat_id),
                "Database error occurred while fetching endpoints. Please try again later.",
            );
            if let Some(tid) = thread_id {
                request = request.message_thread_id(tid);
            }
            request.await?;
            return Ok(());
        }
    };

    if health_urls.is_empty() {
        let mut request = bot.send_message(
            ChatId(telegram_chat_id),
            "No health check endpoints found. Use /new <url> to add one.",
        );
        if let Some(tid) = thread_id {
            request = request.message_thread_id(tid);
        }
        request.await?;
        return Ok(());
    }

    let (message, keyboard) = build_endpoint_list(&health_urls);

    let mut request = bot
        .send_message(ChatId(telegram_chat_id), message)
        .parse_mode(ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new(keyboard));
    if let Some(tid) = thread_id {
        request = request.message_thread_id(tid);
    }
    request.await?;

    Ok(())
}

fn build_endpoint_list(
    health_urls: &[notifine::models::HealthUrl],
) -> (String, Vec<Vec<InlineKeyboardButton>>) {
    let mut message = String::from("<b>Health Check Endpoints:</b>\n\n");
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    for health_url in health_urls {
        let status_emoji = if (200..300).contains(&health_url.status_code) {
            "✅"
        } else {
            "❌"
        };
        message.push_str(&format!(
            "{} <b>Status:</b> {} | {}\n",
            status_emoji, health_url.status_code, health_url.url
        ));
        keyboard.push(vec![InlineKeyboardButton::callback(
            format!("🗑️ Delete {}", health_url.url),
            format!("delete:{}", health_url.id),
        )]);
    }

    (message, keyboard)
}

async fn handle_delete_endpoint(
    pool: &DbPool,
    bot: &Bot,
    telegram_chat_id: i64,
    thread_id: Option<i32>,
    id_str: String,
) -> ResponseResult<()> {
    let id_str = id_str.trim();

    if id_str.is_empty() {
        let mut request = bot.send_message(
            ChatId(telegram_chat_id),
            "Please provide an ID. Use /list to see available endpoints and their IDs.",
        );
        if let Some(tid) = thread_id {
            request = request.message_thread_id(tid);
        }
        request.await?;
        return Ok(());
    }

    let chat = match find_chat_by_telegram_chat_id(pool, &telegram_chat_id.to_string()) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find chat in delete endpoint: {}", e),
                )
                .await;
            let mut request = bot.send_message(
                ChatId(telegram_chat_id),
                "Database error occurred. Please try again later.",
            );
            if let Some(tid) = thread_id {
                request = request.message_thread_id(tid);
            }
            request.await?;
            return Ok(());
        }
    };

    let chat = match chat {
        Some(c) => c,
        None => {
            let mut request = bot.send_message(
                ChatId(telegram_chat_id),
                "You should call /start command first to initialize the bot.",
            );
            if let Some(tid) = thread_id {
                request = request.message_thread_id(tid);
            }
            request.await?;
            return Ok(());
        }
    };

    let id: i32 = match id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            let mut request = bot.send_message(
                ChatId(telegram_chat_id),
                "Invalid ID. Please provide a numeric ID.",
            );
            if let Some(tid) = thread_id {
                request = request.message_thread_id(tid);
            }
            request.await?;
            return Ok(());
        }
    };

    let health_urls = match get_health_urls_by_chat_id(pool, chat.id as i64) {
        Ok(urls) => urls,
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to get health URLs in delete endpoint: {}", e),
                )
                .await;
            let mut request = bot.send_message(
                ChatId(telegram_chat_id),
                "Database error occurred while fetching endpoints. Please try again later.",
            );
            if let Some(tid) = thread_id {
                request = request.message_thread_id(tid);
            }
            request.await?;
            return Ok(());
        }
    };
    let health_url = health_urls.iter().find(|h| h.id == id);

    if health_url.is_none() {
        let mut request = bot.send_message(
            ChatId(telegram_chat_id),
            "Endpoint not found. Use /list to see available endpoints.",
        );
        if let Some(tid) = thread_id {
            request = request.message_thread_id(tid);
        }
        request.await?;
        return Ok(());
    }

    let deleted = match delete_health_url_by_id(pool, id, chat.id) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to delete health URL {}: {}", id, e),
                )
                .await;
            let mut request = bot.send_message(
                ChatId(telegram_chat_id),
                "Database error occurred while deleting endpoint. Please try again later.",
            );
            if let Some(tid) = thread_id {
                request = request.message_thread_id(tid);
            }
            request.await?;
            return Ok(());
        }
    };
    let message = if deleted {
        format!("Endpoint with ID {} has been deleted.", id)
    } else {
        "Failed to delete endpoint.".to_string()
    };

    let mut request = bot.send_message(ChatId(telegram_chat_id), message);
    if let Some(tid) = thread_id {
        request = request.message_thread_id(tid);
    }
    request.await?;

    Ok(())
}

async fn callback_handler(bot: Bot, q: CallbackQuery, pool: DbPool) -> ResponseResult<()> {
    if let Some(data) = q.data {
        if let Some(id_str) = data.strip_prefix("delete:") {
            if let Ok(health_url_id) = id_str.parse::<i32>() {
                if let Some(msg) = q.message {
                    let telegram_chat_id = msg.chat.id.0;
                    let chat =
                        match find_chat_by_telegram_chat_id(&pool, &telegram_chat_id.to_string()) {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::error!("Database error: {:?}", e);
                                METRICS.increment_errors();
                                ALERTS
                                    .send_alert(
                                        &bot,
                                        Severity::Error,
                                        "Database",
                                        &format!("Failed to find chat in callback handler: {}", e),
                                    )
                                    .await;
                                bot.answer_callback_query(&q.id)
                                    .text("Database error occurred")
                                    .await?;
                                return Ok(());
                            }
                        };

                    if let Some(chat) = chat {
                        let deleted = match delete_health_url_by_id(&pool, health_url_id, chat.id) {
                            Ok(d) => d,
                            Err(e) => {
                                tracing::error!("Database error: {:?}", e);
                                METRICS.increment_errors();
                                ALERTS
                                    .send_alert(
                                        &bot,
                                        Severity::Error,
                                        "Database",
                                        &format!(
                                            "Failed to delete health URL {} in callback: {}",
                                            health_url_id, e
                                        ),
                                    )
                                    .await;
                                bot.answer_callback_query(&q.id)
                                    .text("Database error while deleting")
                                    .await?;
                                return Ok(());
                            }
                        };

                        if deleted {
                            bot.answer_callback_query(&q.id).await?;
                            let health_urls = match get_health_urls_by_chat_id(
                                &pool,
                                chat.id as i64,
                            ) {
                                Ok(urls) => urls,
                                Err(e) => {
                                    tracing::error!("Database error: {:?}", e);
                                    METRICS.increment_errors();
                                    ALERTS
                                        .send_alert(
                                            &bot,
                                            Severity::Error,
                                            "Database",
                                            &format!(
                                                "Failed to get health URLs in callback: {}",
                                                e
                                            ),
                                        )
                                        .await;
                                    bot.edit_message_text(
                                        msg.chat.id,
                                        msg.id,
                                        "Endpoint deleted but failed to refresh list. Use /list to see current endpoints.",
                                    )
                                    .await?;
                                    return Ok(());
                                }
                            };

                            if health_urls.is_empty() {
                                bot.edit_message_text(
                                    msg.chat.id,
                                    msg.id,
                                    "All endpoints deleted. Use /new <url> to add one.",
                                )
                                .await?;
                            } else {
                                let (new_message, keyboard) = build_endpoint_list(&health_urls);
                                bot.edit_message_text(msg.chat.id, msg.id, new_message)
                                    .parse_mode(ParseMode::Html)
                                    .reply_markup(InlineKeyboardMarkup::new(keyboard))
                                    .await?;
                            }
                        } else {
                            bot.answer_callback_query(&q.id)
                                .text("Endpoint not found")
                                .await?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

async fn chat_member_handler(
    bot: Bot,
    update: ChatMemberUpdated,
    pool: DbPool,
) -> ResponseResult<()> {
    let chat_id = update.chat.id.0;
    let bot_name = "Uptime";

    tracing::info!(
        "Received chat member update from {}: {:#?} {:#?}",
        chat_id,
        update.old_chat_member,
        update.new_chat_member
    );

    let old_kind = &update.old_chat_member.kind;
    let new_kind = &update.new_chat_member.kind;

    if *old_kind == ChatMemberKind::Left && *new_kind == ChatMemberKind::Member {
        handle_new_chat_and_start_command(
            &pool,
            &bot,
            StartCommand {
                chat_id,
                thread_id: None,
                inviter_username: update.from.username,
            },
        )
        .await?
    } else if matches!(
        old_kind,
        ChatMemberKind::Member | ChatMemberKind::Administrator { .. }
    ) && matches!(
        new_kind,
        ChatMemberKind::Left | ChatMemberKind::Banned { .. }
    ) {
        tracing::info!("Bot removed from chat {}", chat_id);
        METRICS.increment_churn();

        if let Err(e) = deactivate_chat(&pool, &chat_id.to_string()) {
            tracing::error!("Failed to deactivate chat {}: {:?}", chat_id, e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    &bot,
                    Severity::Warning,
                    "Database",
                    &format!("Failed to deactivate chat {}: {}", chat_id, e),
                )
                .await;
        }

        send_message_to_admin(
            &bot,
            format!("{bot_name} bot removed from chat: {chat_id}"),
            10,
        )
        .await?;
    }

    Ok(())
}

pub async fn send_telegram_message(bot: &Bot, message: TelegramMessage) -> ResponseResult<()> {
    let TelegramMessage {
        chat_id,
        thread_id,
        message,
    } = message;

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

async fn handle_new_chat_and_start_command(
    pool: &DbPool,
    bot: &Bot,
    start_command: StartCommand,
) -> ResponseResult<()> {
    let StartCommand {
        chat_id,
        thread_id,
        inviter_username,
    } = start_command;
    let bot_name = "Uptime";

    let thread_id_str = thread_id.map(|tid| tid.to_string());
    let thread_id_ref = thread_id_str.as_deref();

    let existing_chat = match find_chat_by_telegram_chat_id(pool, &chat_id.to_string()) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Database error checking existing chat: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to check existing chat in start command: {}", e),
                )
                .await;
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: "Database error occurred. Please try again later.".to_string(),
                },
            )
            .await?;
            return Ok(());
        }
    };

    if existing_chat.is_none() {
        if let Err(e) = create_chat(
            pool,
            CreateChatInput {
                name: "uptime",
                telegram_chat_id: &chat_id.to_string(),
                telegram_thread_id: thread_id_ref,
                webhook_url: None,
                language: "en",
            },
        ) {
            tracing::error!("Failed to create chat: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to create chat for uptime bot: {}", e),
                )
                .await;
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: "Failed to initialize the bot. Please try again later.".to_string(),
                },
            )
            .await?;
            return Ok(());
        }
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

    send_telegram_message(
        bot,
        TelegramMessage {
            chat_id,
            thread_id,
            message,
        },
    )
    .await?;

    let inviter_username = inviter_username.unwrap_or_else(|| "unknown".to_string());

    if existing_chat.is_none() {
        METRICS.increment_new_chat();
        send_message_to_admin(
            bot,
            format!("New {bot_name} /start command: {chat_id} by @{inviter_username}"),
            10,
        )
        .await?;
    }

    Ok(())
}

pub async fn run_bot(pool: DbPool, token: String) {
    tracing::info!("Starting bot...");

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
        .branch(Update::filter_my_chat_member().endpoint({
            let pool = pool.clone();
            move |bot: Bot, update: ChatMemberUpdated| {
                let pool = pool.clone();
                async move { chat_member_handler(bot, update, pool).await }
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
