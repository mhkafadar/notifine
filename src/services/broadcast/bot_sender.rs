use std::collections::HashMap;
use std::env;
use teloxide::prelude::*;
use teloxide::types::ParseMode;

use super::types::{BotType, BroadcastTarget, DeliveryResult};

pub struct BotSender {
    bots: HashMap<BotType, Bot>,
}

impl BotSender {
    pub fn new() -> Self {
        let mut bots = HashMap::new();

        for bot_type in BotType::all_ordered() {
            if let Ok(token) = env::var(bot_type.env_token_name()) {
                if !token.is_empty() {
                    bots.insert(bot_type, Bot::new(token));
                }
            }
        }

        Self { bots }
    }

    pub fn available_bots(&self) -> Vec<BotType> {
        let mut available: Vec<BotType> = self.bots.keys().copied().collect();
        available.sort_by_key(|b| match b {
            BotType::Gitlab => 1,
            BotType::Github => 2,
            BotType::Beep => 3,
            BotType::Uptime => 4,
            BotType::Agreement => 5,
        });
        available
    }

    pub fn has_bots(&self) -> bool {
        !self.bots.is_empty()
    }

    pub fn get_bot(&self, bot_type: BotType) -> Option<&Bot> {
        self.bots.get(&bot_type)
    }

    pub async fn send_to_target(
        &self,
        target: &BroadcastTarget,
        message: &str,
        discovery_mode: bool,
    ) -> DeliveryResult {
        let available_bots = self.available_bots();
        let mut attempted_bots = Vec::new();
        let mut successful_bots = Vec::new();
        let mut first_successful_bot: Option<BotType> = None;
        let mut last_error: Option<String> = None;

        let ordered_bots = self.prioritize_bots(&available_bots, &target.known_working_bots);

        for bot_type in ordered_bots {
            if let Some(bot) = self.bots.get(&bot_type) {
                attempted_bots.push(bot_type);

                match self
                    .try_send_message(bot, target.telegram_chat_id, target.thread_id, message)
                    .await
                {
                    Ok(_) => {
                        successful_bots.push(bot_type);
                        if first_successful_bot.is_none() {
                            first_successful_bot = Some(bot_type);
                        }

                        if !discovery_mode {
                            return DeliveryResult {
                                telegram_chat_id: target.telegram_chat_id,
                                success: true,
                                successful_bot: first_successful_bot,
                                successful_bots,
                                attempted_bots,
                                error_message: None,
                            };
                        }
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        tracing::debug!(
                            "Bot {:?} failed for chat {}: {}",
                            bot_type,
                            target.telegram_chat_id,
                            error_str
                        );
                        last_error = Some(format!("{:?}: {}", bot_type, error_str));
                    }
                }
            }
        }

        let success = !successful_bots.is_empty();
        DeliveryResult {
            telegram_chat_id: target.telegram_chat_id,
            success,
            successful_bot: first_successful_bot,
            successful_bots,
            attempted_bots,
            error_message: if !success { last_error } else { None },
        }
    }

    fn prioritize_bots(&self, available: &[BotType], known_working: &[BotType]) -> Vec<BotType> {
        let mut result = Vec::new();

        for bot_type in known_working {
            if available.contains(bot_type) && !result.contains(bot_type) {
                result.push(*bot_type);
            }
        }

        for bot_type in available {
            if !result.contains(bot_type) {
                result.push(*bot_type);
            }
        }

        result
    }

    async fn try_send_message(
        &self,
        bot: &Bot,
        chat_id: i64,
        thread_id: Option<i32>,
        message: &str,
    ) -> Result<(), teloxide::RequestError> {
        let mut request = bot
            .send_message(ChatId(chat_id), message)
            .parse_mode(ParseMode::Html);

        if let Some(tid) = thread_id {
            request = request.message_thread_id(tid);
        }

        request.await?;
        Ok(())
    }
}

impl Default for BotSender {
    fn default() -> Self {
        Self::new()
    }
}
