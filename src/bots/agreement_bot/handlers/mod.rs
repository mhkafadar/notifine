mod callbacks;
mod commands;
mod inputs;
mod menu;
mod reminders;
mod settings;

use crate::observability::alerts::Severity;
use crate::observability::{ALERTS, METRICS};
use crate::services::broadcast::db::{handle_bot_removed, upsert_chat_bot_subscription};
use crate::services::broadcast::types::BotType;
use crate::services::stats::{record_churn_event, record_new_chat_event};
use crate::utils::telegram_admin::send_message_to_admin;
use html_escape::encode_text;
use notifine::db::DbPool;
use teloxide::prelude::*;
use teloxide::types::{ChatMemberKind, ChatMemberUpdated};

pub use callbacks::callback_handler;
pub use commands::{command_handler, Command};
pub use inputs::message_handler;
pub use menu::{handle_flow_cancel, handle_menu_select};
pub use reminders::handle_reminder_callback;
pub use settings::{
    handle_disclaimer_accept, handle_disclaimer_decline, handle_language_select,
    handle_settings_language_menu, handle_settings_timezone_menu, handle_timezone_select,
};

pub async fn chat_member_handler(
    bot: Bot,
    update: ChatMemberUpdated,
    pool: DbPool,
) -> ResponseResult<()> {
    let chat_id = update.chat.id.0;
    let bot_name = "Agreement";
    let chat_title = update.chat.title().map(|t| t.to_string());

    tracing::info!(
        "Agreement bot received chat member update from {}: {:#?} {:#?}",
        chat_id,
        update.old_chat_member,
        update.new_chat_member
    );

    let old_kind = &update.old_chat_member.kind;
    let new_kind = &update.new_chat_member.kind;

    if *old_kind == ChatMemberKind::Left && *new_kind == ChatMemberKind::Member {
        let inviter_username = update.from.username.clone();

        METRICS.increment_new_chat();

        if let Err(e) = upsert_chat_bot_subscription(&pool, chat_id, BotType::Agreement, true) {
            tracing::warn!("Failed to track subscription for Agreement bot: {:?}", e);
        }

        if let Err(e) = record_new_chat_event(
            &pool,
            chat_id,
            bot_name,
            inviter_username.as_deref(),
            chat_title.as_deref(),
        ) {
            tracing::warn!("Failed to record new chat event: {:?}", e);
        }

        let inviter_str = inviter_username.unwrap_or_else(|| "unknown".to_string());
        send_message_to_admin(
            &bot,
            format!(
                "New {bot_name} chat: {chat_id} by @{}",
                encode_text(&inviter_str)
            ),
            10,
        )
        .await?;
    } else if matches!(
        old_kind,
        ChatMemberKind::Member | ChatMemberKind::Administrator { .. }
    ) && matches!(
        new_kind,
        ChatMemberKind::Left | ChatMemberKind::Banned { .. }
    ) {
        tracing::info!("Agreement bot removed from chat {}", chat_id);
        METRICS.increment_churn();

        if let Err(e) = record_churn_event(&pool, chat_id, bot_name, chat_title.as_deref()) {
            tracing::warn!("Failed to record churn event: {:?}", e);
        }

        match handle_bot_removed(&pool, chat_id, BotType::Agreement) {
            Ok(was_deactivated) => {
                let message = if was_deactivated {
                    format!(
                        "{bot_name} bot removed from chat {chat_id} - chat deactivated (no reachable bots)"
                    )
                } else {
                    format!(
                        "{bot_name} bot removed from chat {chat_id} but other bots still reachable"
                    )
                };
                send_message_to_admin(&bot, message, 10).await?;
            }
            Err(e) => {
                tracing::error!(
                    "Failed to handle bot removal for Agreement chat {}: {:?}",
                    chat_id,
                    e
                );
                METRICS.increment_errors();
                ALERTS
                    .send_alert(
                        &bot,
                        Severity::Warning,
                        "Database",
                        &format!("Failed to handle bot removal for chat {}: {}", chat_id, e),
                    )
                    .await;
            }
        }
    }

    Ok(())
}
