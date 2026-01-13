use notifine::db::DbPool;
use notifine::get_conversation_state;
use teloxide::prelude::*;

use crate::bots::agreement_bot::flows::{
    handle_custom_description_input, handle_custom_reminder_amount_input,
    handle_custom_reminder_title_input, handle_custom_title_input, handle_edit_amount_input,
    handle_edit_description_input, handle_edit_title_input, handle_rent_amount_input,
    handle_rent_title_input,
};
use crate::bots::agreement_bot::types::states;
use crate::bots::agreement_bot::utils::get_user_language;

pub async fn message_handler(bot: Bot, msg: Message, pool: DbPool) -> ResponseResult<()> {
    let user = match msg.from() {
        Some(u) => u,
        None => return Ok(()),
    };

    let user_id = user.id.0 as i64;
    let chat_id = msg.chat.id.0;
    let thread_id = msg.thread_id;

    let text = match msg.text() {
        Some(t) => t.to_string(),
        None => return Ok(()),
    };

    let state = match get_conversation_state(&pool, user_id) {
        Ok(Some(s)) => s,
        _ => return Ok(()),
    };

    let language = get_user_language(&pool, user_id);

    match state.state.as_str() {
        states::RENT_TITLE => {
            handle_rent_title_input(&pool, &bot, chat_id, thread_id, user_id, &language, &text)
                .await?;
        }
        states::RENT_AMOUNT => {
            handle_rent_amount_input(&pool, &bot, chat_id, thread_id, user_id, &language, &text)
                .await?;
        }
        states::CUSTOM_TITLE => {
            handle_custom_title_input(&pool, &bot, chat_id, thread_id, user_id, &language, &text)
                .await?;
        }
        states::CUSTOM_DESCRIPTION => {
            handle_custom_description_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text,
            )
            .await?;
        }
        states::CUSTOM_REMINDER_TITLE => {
            handle_custom_reminder_title_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text,
            )
            .await?;
        }
        states::CUSTOM_REMINDER_AMOUNT => {
            handle_custom_reminder_amount_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text,
            )
            .await?;
        }
        states::EDIT_TITLE => {
            handle_edit_title_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text, &state,
            )
            .await?;
        }
        states::EDIT_AMOUNT => {
            handle_edit_amount_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text, &state,
            )
            .await?;
        }
        states::EDIT_DESCRIPTION => {
            handle_edit_description_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text, &state,
            )
            .await?;
        }
        _ => {}
    }

    Ok(())
}
