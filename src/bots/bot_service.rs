// bot_service.rs
use crate::utils::telegram_admin::send_message_to_admin;
use notifine::{get_webhook_url_or_create, WebhookGetOrCreateInput};
use std::collections::HashSet;
use std::env;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree::case;
use teloxide::macros::BotCommands;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::LoggingErrorHandler;
use teloxide::prelude::{ChatId, ChatMemberUpdated, Message, Requester, ResponseResult, Update};
use teloxide::types::{ChatMemberKind, ParseMode};
use teloxide::{dptree, filter_command, Bot};

#[derive(Debug, Clone)]
pub struct BotConfig {
    pub bot_name: String,
    pub token: String,
}

#[derive(Debug, Clone)]
pub struct BotService {
    pub bot: Bot,
    config: BotConfig,
}

pub struct StartCommand {
    pub chat_id: i64,
    pub thread_id: Option<i32>,
    pub inviter_username: Option<String>,
}

pub struct TelegramMessage {
    pub chat_id: i64,
    pub thread_id: Option<i32>,
    pub message: String,
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
    #[command(
        description = "Send a broadcast message to all users (admin only). Usage: /broadcast <message>"
    )]
    Broadcast,
}

impl BotService {
    pub fn new(config: BotConfig) -> Self {
        BotService {
            bot: Bot::new(&config.token),
            config,
        }
    }

    async fn handle_start_command(&self, msg: Message) -> ResponseResult<()> {
        log::info!("Start command received");
        let inviter_username = match msg.from() {
            Some(user) => user.username.clone(),
            None => None,
        };

        let thread_id = msg.thread_id;

        self.handle_new_chat_and_start_command(StartCommand {
            chat_id: msg.chat.id.0,
            thread_id,
            inviter_username,
        })
        .await?;

        Ok(())
    }

    async fn handle_new_chat_and_start_command(
        &self,
        start_command: StartCommand,
    ) -> ResponseResult<()> {
        let StartCommand {
            chat_id,
            thread_id,
            inviter_username,
        } = start_command;
        let bot_name = &self.config.bot_name;

        // Convert thread_id to String if present
        let thread_id_str = thread_id.map(|tid| tid.to_string());
        let thread_id_ref = thread_id_str.as_deref();

        let webhook_info = get_webhook_url_or_create(WebhookGetOrCreateInput {
            telegram_chat_id: chat_id.to_string().as_str(),
            telegram_thread_id: thread_id_ref,
        });

        let message = if webhook_info.webhook_url.is_empty() {
            log::error!("Error creating or getting webhook: {:?}", webhook_info);
            "Hi there!\
                      Our bot is curently has some problems \
                      Please create a Github issue here: \
                      https://github.com/mhkafadar/notifine/issues/new"
                .to_string()
        } else {
            format!(
                "Hi there! \
                 To setup notifications for \
                 this chat your {} project(repo), \
                 open Settings -> Webhooks and add this \
                 URL: {}/{}/{}",
                bot_name,
                env::var("WEBHOOK_BASE_URL").expect("WEBHOOK_BASE_URL must be set"),
                bot_name.to_lowercase(),
                webhook_info.webhook_url
            )
        };

        self.send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message,
        })
        .await?;

        if webhook_info.is_new {
            let inviter_username = inviter_username.unwrap_or_else(|| "unknown".to_string());

            send_message_to_admin(
                &self.bot,
                format!("New {bot_name} webhook added: {chat_id} by @{inviter_username}"),
                10,
            )
            .await?;
        }

        Ok(())
    }

    async fn handle_my_chat_member_update(&self, update: ChatMemberUpdated) -> ResponseResult<()> {
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
            self.handle_new_chat_and_start_command(StartCommand {
                chat_id,
                thread_id: None,
                inviter_username: update.from.username,
            })
            .await?
        }

        Ok(())
    }

    pub async fn send_telegram_message(&self, message: TelegramMessage) -> ResponseResult<()> {
        let TelegramMessage {
            chat_id,
            thread_id,
            message,
        } = message;

        log::info!("Sending message to {}: {}", chat_id, message);
        let bot = &self.bot;
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

    async fn handle_broadcast_command(&self, msg: Message) -> ResponseResult<()> {
        // Check if the user is admin
        let admin_chat_id: i64 = env::var("TELEGRAM_ADMIN_CHAT_ID")
            .expect("TELEGRAM_ADMIN_CHAT_ID must be set")
            .parse::<i64>()
            .expect("Error parsing TELEGRAM_ADMIN_CHAT_ID");

        if msg.chat.id.0 != admin_chat_id {
            self.bot
                .send_message(
                    msg.chat.id,
                    "Sorry, this command is only available to administrators.",
                )
                .await?;
            return Ok(());
        }

        let broadcast_message = msg
            .text()
            .and_then(|text| text.split_once(' ').map(|(_, message)| message.to_string()));

        let broadcast_message = match broadcast_message {
            Some(message) if !message.trim().is_empty() => message,
            _ => {
                self.bot
                    .send_message(
                        msg.chat.id,
                        "Please provide a message to broadcast. Usage: /broadcast <message>",
                    )
                    .await?;
                return Ok(());
            }
        };

        // Get all chats from the database and filter unique chat IDs
        let chats = notifine::get_all_chats();
        let mut unique_chats = HashSet::new();
        let mut success_count = 0;
        let mut total_unique_chats = 0;

        for chat in chats {
            // Only process each chat ID once
            if unique_chats.insert(chat.telegram_id.clone()) {
                total_unique_chats += 1;
                let telegram_message = TelegramMessage {
                    chat_id: chat.telegram_id.parse().expect("Failed to parse chat ID"),
                    thread_id: chat.thread_id.and_then(|tid| tid.parse().ok()),
                    message: broadcast_message.clone(),
                };

                // If sending to one chat fails, continue with others
                match self.send_telegram_message(telegram_message).await {
                    Ok(_) => success_count += 1,
                    Err(e) => log::error!(
                        "Failed to send broadcast message to chat {}: {}",
                        chat.telegram_id,
                        e
                    ),
                }
            }
        }

        self.bot
            .send_message(
                msg.chat.id,
                format!(
                    "Broadcast complete!\nMessage sent successfully to {success_count} out of {total_unique_chats} unique chats."
                ),
            )
            .await?;

        Ok(())
    }

    pub async fn run_bot(self) {
        let handler = Update::filter_message()
            .branch(
                filter_command::<Command, _>()
                    .branch(case![Command::Start].endpoint(
                        move |msg: Message, bot: BotService| async move {
                            bot.handle_start_command(msg).await
                        },
                    ))
                    .branch(case![Command::Broadcast].endpoint(
                        move |msg: Message, bot: BotService| async move {
                            bot.handle_broadcast_command(msg).await
                        },
                    )),
            )
            .branch(Update::filter_my_chat_member().endpoint(
                move |upd: ChatMemberUpdated, bot: BotService| async move {
                    bot.handle_my_chat_member_update(upd).await
                },
            ));

        Dispatcher::builder(self.bot.clone(), handler)
            .dependencies(dptree::deps![self])
            .default_handler(|_| async {})
            .error_handler(LoggingErrorHandler::with_custom_text(
                "An error has occurred in the dispatcher",
            ))
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;
    }
}
