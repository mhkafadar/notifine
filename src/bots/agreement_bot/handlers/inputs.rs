use notifine::db::DbPool;
use notifine::{find_agreement_user_by_telegram_id, get_conversation_state};
use teloxide::prelude::*;

use crate::bots::agreement_bot::flows::{
    handle_custom_description_input, handle_custom_reminder_amount_input,
    handle_custom_reminder_day_input, handle_custom_reminder_month_input,
    handle_custom_reminder_title_input, handle_custom_reminder_year_input,
    handle_custom_title_input, handle_edit_amount_input, handle_edit_description_input,
    handle_edit_title_input, handle_rent_amount_input, handle_rent_contract_duration_custom_input,
    handle_rent_due_day_input, handle_rent_start_day_input, handle_rent_start_month_input,
    handle_rent_start_year_input, handle_rent_title_input,
};
use crate::bots::agreement_bot::keyboards::{build_disclaimer_keyboard, build_menu_keyboard};
use crate::bots::agreement_bot::types::states;
use crate::bots::agreement_bot::utils::{
    detect_language, get_user_language, send_message_with_keyboard,
};

use super::perform_onboarding;

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

    if msg.chat.is_private() {
        let existing_user = find_agreement_user_by_telegram_id(&pool, user_id)
            .ok()
            .flatten();

        match existing_user {
            None => {
                let language = detect_language(&msg);
                perform_onboarding(&pool, &bot, &msg, user_id, chat_id, thread_id, &language)
                    .await?;
                return Ok(());
            }
            Some(ref db_user) if db_user.disclaimer_accepted => {
                let state = get_conversation_state(&pool, user_id).ok().flatten();
                if state.is_none() {
                    let message = notifine::i18n::t(&db_user.language, "agreement.menu.title");
                    let keyboard = build_menu_keyboard(&db_user.language);
                    send_message_with_keyboard(&bot, chat_id, thread_id, &message, keyboard)
                        .await?;
                    return Ok(());
                }
            }
            Some(ref db_user) => {
                let disclaimer_message = format!(
                    "{}\n\n{}\n\n{}",
                    notifine::i18n::t(&db_user.language, "agreement.disclaimer.title"),
                    notifine::i18n::t(&db_user.language, "agreement.disclaimer.content"),
                    notifine::i18n::t(&db_user.language, "agreement.disclaimer.accept_prompt")
                );
                let keyboard = build_disclaimer_keyboard(&db_user.language);
                send_message_with_keyboard(&bot, chat_id, thread_id, &disclaimer_message, keyboard)
                    .await?;
                return Ok(());
            }
        }
    }

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
        states::RENT_START_YEAR => {
            handle_rent_start_year_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text,
            )
            .await?;
        }
        states::RENT_START_MONTH => {
            handle_rent_start_month_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text,
            )
            .await?;
        }
        states::RENT_START_DAY => {
            handle_rent_start_day_input(&pool, &bot, chat_id, thread_id, user_id, &language, &text)
                .await?;
        }
        states::RENT_CONTRACT_DURATION_CUSTOM => {
            handle_rent_contract_duration_custom_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text,
            )
            .await?;
        }
        states::RENT_DUE_DAY => {
            handle_rent_due_day_input(&pool, &bot, chat_id, thread_id, user_id, &language, &text)
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
        states::CUSTOM_REMINDER_YEAR => {
            handle_custom_reminder_year_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text,
            )
            .await?;
        }
        states::CUSTOM_REMINDER_MONTH => {
            handle_custom_reminder_month_input(
                &pool, &bot, chat_id, thread_id, user_id, &language, &text,
            )
            .await?;
        }
        states::CUSTOM_REMINDER_DAY => {
            handle_custom_reminder_day_input(
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
