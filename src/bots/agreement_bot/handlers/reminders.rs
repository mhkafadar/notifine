use chrono::Utc;
use notifine::db::DbPool;
use notifine::i18n::{t, t_with_args};
use notifine::{
    find_agreement_by_id, find_agreement_user_by_telegram_id, find_reminder_by_id,
    update_reminder_snooze, update_reminder_status,
};
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;

use crate::bots::agreement_bot::keyboards::build_snooze_options_keyboard;
use crate::bots::agreement_bot::utils::get_user_language;

pub async fn handle_reminder_callback(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
    data: &str,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);
    let msg = match &q.message {
        Some(m) => m,
        None => return Ok(()),
    };

    if let Some(reminder_id_str) = data.strip_prefix("rem:done:") {
        handle_reminder_done(pool, bot, q, msg, user_id, &language, reminder_id_str).await?;
    } else if let Some(reminder_id_str) = data.strip_prefix("rem:snooze:") {
        handle_reminder_snooze_menu(pool, bot, q, msg, user_id, &language, reminder_id_str).await?;
    } else if let Some(rest) = data.strip_prefix("rem:snooze_") {
        handle_reminder_snooze_duration(pool, bot, q, msg, user_id, &language, rest).await?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_reminder_done(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    msg: &teloxide::types::Message,
    user_id: i64,
    language: &str,
    reminder_id_str: &str,
) -> ResponseResult<()> {
    let reminder_id: i32 = match reminder_id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }
    };

    let reminder = match find_reminder_by_id(pool, reminder_id) {
        Ok(Some(r)) => r,
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }
    };

    let agreement = match find_agreement_by_id(pool, reminder.agreement_id) {
        Ok(Some(a)) => a,
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }
    };

    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.errors.user_not_found"))
                .await?;
            return Ok(());
        }
    };

    if agreement.user_id != user.id {
        bot.answer_callback_query(&q.id)
            .text(t(language, "agreement.delete.unauthorized"))
            .await?;
        return Ok(());
    }

    if let Err(e) = update_reminder_status(pool, reminder_id, "done") {
        tracing::error!("Failed to mark reminder as done: {:?}", e);
        bot.answer_callback_query(&q.id)
            .text(t(language, "agreement.errors.database_error"))
            .await?;
        return Ok(());
    }

    bot.answer_callback_query(&q.id)
        .text(t(language, "agreement.reminder.marked_done"))
        .await?;

    bot.edit_message_reply_markup(msg.chat.id, msg.id).await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_reminder_snooze_menu(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    msg: &teloxide::types::Message,
    user_id: i64,
    language: &str,
    reminder_id_str: &str,
) -> ResponseResult<()> {
    let reminder_id: i32 = match reminder_id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }
    };

    let reminder = match find_reminder_by_id(pool, reminder_id) {
        Ok(Some(r)) => r,
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }
    };

    let agreement = match find_agreement_by_id(pool, reminder.agreement_id) {
        Ok(Some(a)) => a,
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }
    };

    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.errors.user_not_found"))
                .await?;
            return Ok(());
        }
    };

    if agreement.user_id != user.id {
        bot.answer_callback_query(&q.id)
            .text(t(language, "agreement.delete.unauthorized"))
            .await?;
        return Ok(());
    }

    let keyboard = build_snooze_options_keyboard(reminder_id, language);
    bot.edit_message_reply_markup(msg.chat.id, msg.id)
        .reply_markup(keyboard)
        .await?;
    bot.answer_callback_query(&q.id).await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_reminder_snooze_duration(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    msg: &teloxide::types::Message,
    user_id: i64,
    language: &str,
    rest: &str,
) -> ResponseResult<()> {
    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() != 2 {
        bot.answer_callback_query(&q.id)
            .text(t(language, "agreement.reminder.not_found"))
            .await?;
        return Ok(());
    }

    let duration = parts[0];
    let reminder_id: i32 = match parts[1].parse() {
        Ok(id) => id,
        Err(_) => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }
    };

    let reminder = match find_reminder_by_id(pool, reminder_id) {
        Ok(Some(r)) => r,
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }
    };

    let agreement = match find_agreement_by_id(pool, reminder.agreement_id) {
        Ok(Some(a)) => a,
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }
    };

    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.errors.user_not_found"))
                .await?;
            return Ok(());
        }
    };

    if agreement.user_id != user.id {
        bot.answer_callback_query(&q.id)
            .text(t(language, "agreement.delete.unauthorized"))
            .await?;
        return Ok(());
    }

    let snooze_until = match duration {
        "1h" => Utc::now() + chrono::Duration::hours(1),
        "3h" => Utc::now() + chrono::Duration::hours(3),
        "1d" => Utc::now() + chrono::Duration::days(1),
        "3d" => Utc::now() + chrono::Duration::days(3),
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }
    };

    if let Err(e) = update_reminder_snooze(pool, reminder_id, snooze_until) {
        tracing::error!("Failed to snooze reminder: {:?}", e);
        bot.answer_callback_query(&q.id)
            .text(t(language, "agreement.errors.database_error"))
            .await?;
        return Ok(());
    }

    let snooze_display = snooze_until.format("%d.%m.%Y %H:%M").to_string();
    let message = t_with_args(language, "agreement.reminder.snoozed", &[&snooze_display]);

    bot.answer_callback_query(&q.id).text(&message).await?;
    bot.edit_message_reply_markup(msg.chat.id, msg.id).await?;

    Ok(())
}
