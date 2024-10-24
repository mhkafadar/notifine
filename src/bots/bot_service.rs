// bot_service.rs
use crate::utils::telegram_admin::send_message_to_admin;
use notifine::{get_webhook_url_or_create, WebhookGetOrCreateInput};
use std::env;
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree::case;
use teloxide::macros::BotCommands;
use teloxide::payloads::SendMessageSetters;
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
        let thread_id_ref = thread_id_str.as_ref().map(String::as_str);

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

    pub async fn run_bot(self) {
        log::info!("Starting bot...");

        let bot_clone = self.bot.clone();
        let command_handler_self = self.clone();
        let chat_member_handler_self = self.clone();

        let command_handler =
            filter_command::<Command, _>().branch(case![Command::Start].endpoint({
                move |msg: Message| {
                    let bot_service = command_handler_self.clone(); // Use the pre-cloned `self`
                    async move { bot_service.handle_start_command(msg).await }
                }
            }));

        let chat_member_handler = {
            move |update: ChatMemberUpdated| {
                let bot_service = chat_member_handler_self.clone(); // Use the pre-cloned `self`
                async move { bot_service.handle_my_chat_member_update(update).await }
            }
        };

        let message_handler = dptree::entry()
            .branch(Update::filter_message().branch(command_handler))
            .branch(Update::filter_my_chat_member().endpoint(chat_member_handler));

        Dispatcher::builder(bot_clone, message_handler)
            .dependencies(dptree::deps![InMemStorage::<State>::new()])
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;

        log::info!("Closing bot... Goodbye!");
    }
}
