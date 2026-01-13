use crate::observability::alerts::Severity;
use crate::observability::{ALERTS, METRICS};
use notifine::db::DbPool;
use notifine::i18n::{t, t_with_args};
use notifine::{
    accept_disclaimer, find_agreement_user_by_telegram_id, update_agreement_user_language,
    update_agreement_user_timezone,
};
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;

use crate::bots::agreement_bot::keyboards::{
    build_language_keyboard, build_menu_keyboard, build_timezone_keyboard,
};
use crate::bots::agreement_bot::types::DEFAULT_TIMEZONE;
use crate::bots::agreement_bot::utils::get_user_language;

pub async fn handle_disclaimer_accept(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);

    match accept_disclaimer(pool, user_id) {
        Ok(_) => {
            METRICS.increment_new_chat();
            bot.answer_callback_query(&q.id).await?;

            if let Some(msg) = &q.message {
                let accepted_message = t(&language, "agreement.disclaimer.accepted");
                let keyboard = build_menu_keyboard(&language);
                bot.edit_message_text(msg.chat.id, msg.id, &accepted_message)
                    .reply_markup(keyboard)
                    .await?;
            }
        }
        Err(e) => {
            tracing::error!("Failed to accept disclaimer: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to accept disclaimer: {}", e),
                )
                .await;
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.errors.database_error"))
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_disclaimer_decline(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);

    bot.answer_callback_query(&q.id).await?;

    if let Some(msg) = &q.message {
        let declined_message = t(&language, "agreement.disclaimer.declined");
        bot.edit_message_text(msg.chat.id, msg.id, &declined_message)
            .await?;
    }

    Ok(())
}

pub async fn handle_language_select(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
    new_language: &str,
) -> ResponseResult<()> {
    match update_agreement_user_language(pool, user_id, new_language) {
        Ok(_) => {
            bot.answer_callback_query(&q.id).await?;

            if let Some(msg) = &q.message {
                let changed_message = t(new_language, "agreement.language.changed");
                bot.edit_message_text(msg.chat.id, msg.id, &changed_message)
                    .await?;
            }
        }
        Err(e) => {
            tracing::error!("Failed to update language: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to update language: {}", e),
                )
                .await;
            let language = get_user_language(pool, user_id);
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.errors.database_error"))
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_timezone_select(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
    new_timezone: &str,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);

    match update_agreement_user_timezone(pool, user_id, new_timezone) {
        Ok(_) => {
            bot.answer_callback_query(&q.id).await?;

            if let Some(msg) = &q.message {
                let changed_message =
                    t_with_args(&language, "agreement.timezone.changed", &[new_timezone]);
                bot.edit_message_text(msg.chat.id, msg.id, &changed_message)
                    .await?;
            }
        }
        Err(e) => {
            tracing::error!("Failed to update timezone: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to update timezone: {}", e),
                )
                .await;
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.errors.database_error"))
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_settings_language_menu(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);
    let language_display = if language == "tr" {
        "Türkçe"
    } else {
        "English"
    };
    let message = format!(
        "{}\n\n{}",
        t(&language, "agreement.language.title"),
        t_with_args(&language, "agreement.language.current", &[language_display])
    );
    let keyboard = build_language_keyboard(&language);
    if let Some(msg) = &q.message {
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    }
    bot.answer_callback_query(&q.id).await?;

    Ok(())
}

pub async fn handle_settings_timezone_menu(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);
    let user = find_agreement_user_by_telegram_id(pool, user_id)
        .ok()
        .flatten();
    let current_tz = user
        .map(|u| u.timezone)
        .unwrap_or_else(|| DEFAULT_TIMEZONE.to_string());
    let message = format!(
        "{}\n\n{}\n\n{}",
        t(&language, "agreement.timezone.title"),
        t_with_args(&language, "agreement.timezone.current", &[&current_tz]),
        t(&language, "agreement.timezone.default_note")
    );
    let keyboard = build_timezone_keyboard();
    if let Some(msg) = &q.message {
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    }
    bot.answer_callback_query(&q.id).await?;

    Ok(())
}
