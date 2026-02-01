use crate::observability::METRICS;
use chrono::Utc;
use notifine::db::DbPool;
use notifine::i18n::{t, t_with_args};
use notifine::{clear_conversation_state, set_conversation_state};
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;

use crate::bots::agreement_bot::types::{states, CustomDraft, RentDraft, STATE_EXPIRY_MINUTES};
use crate::bots::agreement_bot::utils::{get_user_language, send_message_with_keyboard};
use teloxide::types::InlineKeyboardMarkup;

pub async fn handle_flow_cancel(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);

    if let Err(e) = clear_conversation_state(pool, user_id) {
        tracing::warn!(
            "Failed to clear conversation state for user {}: {:?}",
            user_id,
            e
        );
    }
    bot.answer_callback_query(&q.id).await?;

    if let Some(msg) = &q.message {
        bot.edit_message_text(
            msg.chat.id,
            msg.id,
            t(&language, "agreement.cancel.success"),
        )
        .await?;
    }

    Ok(())
}

pub async fn handle_menu_select(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
    selection: &str,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);

    bot.answer_callback_query(&q.id).await?;

    if let Some(msg) = &q.message {
        match selection {
            "menu:rent" => {
                let selected_text = t(&language, "agreement.menu.selected_rent");
                bot.edit_message_text(msg.chat.id, msg.id, &selected_text)
                    .await?;
                start_rent_flow(pool, bot, msg.chat.id.0, msg.thread_id, user_id, &language)
                    .await?;
            }
            "menu:custom" => {
                let selected_text = t(&language, "agreement.menu.selected_custom");
                bot.edit_message_text(msg.chat.id, msg.id, &selected_text)
                    .await?;
                start_custom_flow(pool, bot, msg.chat.id.0, msg.thread_id, user_id, &language)
                    .await?;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn start_rent_flow(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
) -> ResponseResult<()> {
    let message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["1", "12"]),
        t(language, "agreement.rent.step1_title.prompt")
    );

    let keyboard = InlineKeyboardMarkup::default();

    let expires_at = Utc::now() + chrono::Duration::minutes(STATE_EXPIRY_MINUTES);
    let draft = RentDraft::default();

    if let Err(e) = set_conversation_state(
        pool,
        user_id,
        states::RENT_TITLE,
        Some(serde_json::to_value(&draft).unwrap_or_default()),
        expires_at,
    ) {
        tracing::error!("Failed to set conversation state: {:?}", e);
        METRICS.increment_errors();
    }

    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

async fn start_custom_flow(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
) -> ResponseResult<()> {
    let message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["1", "3"]),
        t(language, "agreement.custom.step1_title.prompt")
    );

    let keyboard = InlineKeyboardMarkup::default();

    let expires_at = Utc::now() + chrono::Duration::minutes(STATE_EXPIRY_MINUTES);
    let draft = CustomDraft::default();

    if let Err(e) = set_conversation_state(
        pool,
        user_id,
        states::CUSTOM_TITLE,
        Some(serde_json::to_value(&draft).unwrap_or_default()),
        expires_at,
    ) {
        tracing::error!("Failed to set conversation state: {:?}", e);
        METRICS.increment_errors();
    }

    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}
