use crate::observability::alerts::Severity;
use crate::observability::{ALERTS, METRICS};
use chrono::Utc;
use notifine::db::DbPool;
use notifine::i18n::{t, t_with_args};
use notifine::models::Agreement;
use notifine::{
    delete_agreement, delete_reminders_by_agreement_id, find_agreement_by_id,
    find_agreement_user_by_telegram_id, find_agreements_by_user_id, find_reminders_by_agreement_id,
};
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;

use super::super::keyboards::{
    build_agreement_detail_keyboard, build_agreements_list_keyboard, build_delete_confirm_keyboard,
};
use super::super::utils::get_user_language;
use super::handle_edit_callback;

pub async fn handle_agreement_callback(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
    data: &str,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);

    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        Ok(None) => {
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.errors.must_accept_disclaimer"))
                .await?;
            return Ok(());
        }
        Err(e) => {
            tracing::error!("Failed to find user: {:?}", e);
            METRICS.increment_errors();
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.errors.database_error"))
                .await?;
            return Ok(());
        }
    };

    if let Some(rest) = data.strip_prefix("agr:view:") {
        if let Ok(agreement_id) = rest.parse::<i32>() {
            handle_view_agreement(pool, bot, q, user_id, user.id, agreement_id, &language).await?;
        }
    } else if let Some(rest) = data.strip_prefix("agr:delete:confirm:") {
        if let Ok(agreement_id) = rest.parse::<i32>() {
            handle_delete_confirm(pool, bot, q, user.id, agreement_id, &language).await?;
        }
    } else if let Some(rest) = data.strip_prefix("agr:delete:") {
        if let Ok(agreement_id) = rest.parse::<i32>() {
            handle_delete_prompt(pool, bot, q, user.id, agreement_id, &language).await?;
        }
    } else if data == "agr:back:list" {
        handle_back_to_list(pool, bot, q, user_id, user.id, &language).await?;
    } else if let Some(rest) = data.strip_prefix("agr:edit:") {
        handle_edit_callback(pool, bot, q, user_id, user.id, rest, &language).await?;
    } else {
        bot.answer_callback_query(&q.id)
            .text(t(&language, "agreement.errors.unknown_callback"))
            .await?;
    }

    Ok(())
}

async fn handle_view_agreement(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    _telegram_user_id: i64,
    internal_user_id: i32,
    agreement_id: i32,
    language: &str,
) -> ResponseResult<()> {
    let agreement = match find_agreement_by_id(pool, agreement_id) {
        Ok(Some(a)) => a,
        Ok(None) => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.delete.not_found"))
                .await?;
            return Ok(());
        }
        Err(e) => {
            tracing::error!("Failed to find agreement: {:?}", e);
            METRICS.increment_errors();
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.errors.database_error"))
                .await?;
            return Ok(());
        }
    };

    if agreement.user_id != internal_user_id {
        bot.answer_callback_query(&q.id)
            .text(t(language, "agreement.delete.unauthorized"))
            .await?;
        return Ok(());
    }

    let reminders = match find_reminders_by_agreement_id(pool, agreement_id) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to find reminders: {:?}", e);
            METRICS.increment_errors();
            Vec::new()
        }
    };

    let message = build_agreement_detail_view(&agreement, &reminders, language);
    let keyboard =
        build_agreement_detail_keyboard(language, agreement_id, &agreement.agreement_type);

    if let Some(msg) = &q.message {
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    }

    bot.answer_callback_query(&q.id).await?;
    Ok(())
}

fn build_agreement_detail_view(
    agreement: &Agreement,
    reminders: &[notifine::models::Reminder],
    language: &str,
) -> String {
    let mut parts = vec![t(language, "agreement.view.title"), String::new()];

    let type_text = if agreement.agreement_type == "rent" {
        t(language, "agreement.view.type_rent")
    } else {
        t(language, "agreement.view.type_custom")
    };

    parts.push(format!(
        "{}: {}",
        t(language, "agreement.view.field_title"),
        agreement.title
    ));
    parts.push(format!(
        "{}: {}",
        t(language, "agreement.view.field_type"),
        type_text
    ));

    if let Some(role) = &agreement.user_role {
        let role_text = if role == "tenant" {
            t(language, "agreement.view.field_role_tenant")
        } else {
            t(language, "agreement.view.field_role_landlord")
        };
        parts.push(format!(
            "{}: {}",
            t(language, "agreement.view.field_role"),
            role_text
        ));
    }

    if let Some(start_date) = &agreement.start_date {
        parts.push(format!(
            "{}: {}",
            t(language, "agreement.view.field_start_date"),
            start_date.format("%d.%m.%Y")
        ));
    }

    if let Some(amount) = &agreement.rent_amount {
        parts.push(format!(
            "{}: {} {}",
            t(language, "agreement.view.field_amount"),
            amount,
            agreement.currency
        ));
    }

    if let Some(due_day) = agreement.due_day {
        parts.push(format!(
            "{}: {}",
            t(language, "agreement.view.field_due_day"),
            t_with_args(
                language,
                "agreement.view.day_of_month",
                &[&due_day.to_string()]
            )
        ));
    }

    let monthly_status = if agreement.has_monthly_reminder {
        t(language, "agreement.view.enabled")
    } else {
        t(language, "agreement.view.disabled")
    };
    parts.push(format!(
        "{}: {}",
        t(language, "agreement.view.field_monthly_reminder"),
        monthly_status
    ));

    if agreement.agreement_type == "rent" {
        let yearly_status = if agreement.has_yearly_increase_reminder {
            t(language, "agreement.view.enabled")
        } else {
            t(language, "agreement.view.disabled")
        };
        parts.push(format!(
            "{}: {}",
            t(language, "agreement.view.field_yearly_increase"),
            yearly_status
        ));
    }

    if let Some(desc) = &agreement.description {
        if !desc.is_empty() {
            parts.push(format!(
                "{}: {}",
                t(language, "agreement.view.field_description"),
                desc
            ));
        }
    }

    parts.push(String::new());
    parts.push(t(language, "agreement.view.reminders_title"));

    let today = Utc::now().date_naive();
    let pending: Vec<_> = reminders
        .iter()
        .filter(|r| r.reminder_date >= today && r.status == "pending")
        .collect();

    if pending.is_empty() {
        parts.push(t(language, "agreement.view.reminders_empty"));
    } else {
        let monthly: Vec<_> = pending
            .iter()
            .filter(|r| r.reminder_type == "due_day" || r.reminder_type == "pre_notify")
            .take(5)
            .collect();

        let yearly: Vec<_> = pending
            .iter()
            .filter(|r| r.reminder_type == "yearly_increase")
            .take(3)
            .collect();

        let five_year: Vec<_> = pending
            .iter()
            .filter(|r| r.reminder_type == "five_year_notice")
            .take(3)
            .collect();

        let ten_year: Vec<_> = pending
            .iter()
            .filter(|r| r.reminder_type == "ten_year_notice")
            .take(3)
            .collect();

        if !monthly.is_empty() {
            parts.push(String::new());
            parts.push(t(language, "agreement.view.reminders_monthly_title"));
            for reminder in monthly {
                parts.push(format_reminder_line(reminder, agreement));
            }
        }

        if !yearly.is_empty() {
            parts.push(String::new());
            parts.push(t(language, "agreement.view.reminders_yearly_title"));
            for reminder in yearly {
                parts.push(format_reminder_line(reminder, agreement));
            }
        }

        if !five_year.is_empty() {
            parts.push(String::new());
            parts.push(t(language, "agreement.view.reminders_five_year_title"));
            for reminder in five_year {
                parts.push(format_reminder_line(reminder, agreement));
            }
        }

        if !ten_year.is_empty() {
            parts.push(String::new());
            parts.push(t(language, "agreement.view.reminders_ten_year_title"));
            for reminder in ten_year {
                parts.push(format_reminder_line(reminder, agreement));
            }
        }
    }

    parts.join("\n")
}

fn format_reminder_line(reminder: &notifine::models::Reminder, agreement: &Agreement) -> String {
    let amount_str = reminder
        .amount
        .as_ref()
        .map(|a| format!("({} {})", a, agreement.currency))
        .unwrap_or_default();
    format!(
        "  {} - {}{}",
        reminder.reminder_date.format("%d.%m.%Y"),
        reminder.title,
        if amount_str.is_empty() {
            String::new()
        } else {
            format!(" {}", amount_str)
        }
    )
}

async fn handle_delete_prompt(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    internal_user_id: i32,
    agreement_id: i32,
    language: &str,
) -> ResponseResult<()> {
    let agreement = match find_agreement_by_id(pool, agreement_id) {
        Ok(Some(a)) => a,
        Ok(None) => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.delete.not_found"))
                .await?;
            return Ok(());
        }
        Err(e) => {
            tracing::error!("Failed to find agreement: {:?}", e);
            METRICS.increment_errors();
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.errors.database_error"))
                .await?;
            return Ok(());
        }
    };

    if agreement.user_id != internal_user_id {
        bot.answer_callback_query(&q.id)
            .text(t(language, "agreement.delete.unauthorized"))
            .await?;
        return Ok(());
    }

    let reminder_count = match find_reminders_by_agreement_id(pool, agreement_id) {
        Ok(r) => r.len(),
        Err(_) => 0,
    };

    let message = format!(
        "{}\n\n{}",
        t(language, "agreement.delete.confirm_title"),
        t_with_args(
            language,
            "agreement.delete.confirm_message",
            &[&agreement.title, &reminder_count.to_string()]
        )
    );

    let keyboard = build_delete_confirm_keyboard(language, agreement_id);

    if let Some(msg) = &q.message {
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    }

    bot.answer_callback_query(&q.id).await?;
    Ok(())
}

async fn handle_delete_confirm(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    internal_user_id: i32,
    agreement_id: i32,
    language: &str,
) -> ResponseResult<()> {
    let agreement = match find_agreement_by_id(pool, agreement_id) {
        Ok(Some(a)) => a,
        Ok(None) => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.delete.not_found"))
                .await?;
            return Ok(());
        }
        Err(e) => {
            tracing::error!("Failed to find agreement: {:?}", e);
            METRICS.increment_errors();
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.errors.database_error"))
                .await?;
            return Ok(());
        }
    };

    if agreement.user_id != internal_user_id {
        bot.answer_callback_query(&q.id)
            .text(t(language, "agreement.delete.unauthorized"))
            .await?;
        return Ok(());
    }

    let title = agreement.title.clone();

    if let Err(e) = delete_reminders_by_agreement_id(pool, agreement_id) {
        tracing::error!("Failed to delete reminders: {:?}", e);
        METRICS.increment_errors();
        ALERTS
            .send_alert(
                bot,
                Severity::Error,
                "Database",
                &format!(
                    "Failed to delete reminders for agreement {}: {}",
                    agreement_id, e
                ),
            )
            .await;
    }

    match delete_agreement(pool, agreement_id, internal_user_id) {
        Ok(true) => {
            let message = t_with_args(language, "agreement.delete.success", &[&title]);

            if let Some(msg) = &q.message {
                let agreements =
                    find_agreements_by_user_id(pool, internal_user_id).unwrap_or_default();

                if agreements.is_empty() {
                    bot.edit_message_text(msg.chat.id, msg.id, &message).await?;
                } else {
                    let list_message =
                        format!("{}\n\n{}", message, t(language, "agreement.list.title"));
                    let keyboard = build_agreements_list_keyboard(language, &agreements);
                    bot.edit_message_text(msg.chat.id, msg.id, &list_message)
                        .reply_markup(keyboard)
                        .await?;
                }
            }

            bot.answer_callback_query(&q.id).await?;
        }
        Ok(false) => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.delete.not_found"))
                .await?;
        }
        Err(e) => {
            tracing::error!("Failed to delete agreement: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to delete agreement {}: {}", agreement_id, e),
                )
                .await;
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.errors.database_error"))
                .await?;
        }
    }

    Ok(())
}

async fn handle_back_to_list(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    _telegram_user_id: i64,
    internal_user_id: i32,
    language: &str,
) -> ResponseResult<()> {
    let agreements = match find_agreements_by_user_id(pool, internal_user_id) {
        Ok(agrs) => agrs,
        Err(e) => {
            tracing::error!("Failed to find agreements: {:?}", e);
            METRICS.increment_errors();
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.errors.database_error"))
                .await?;
            return Ok(());
        }
    };

    if let Some(msg) = &q.message {
        if agreements.is_empty() {
            bot.edit_message_text(msg.chat.id, msg.id, t(language, "agreement.list.empty"))
                .await?;
        } else {
            let keyboard = build_agreements_list_keyboard(language, &agreements);
            bot.edit_message_text(msg.chat.id, msg.id, t(language, "agreement.list.title"))
                .reply_markup(keyboard)
                .await?;
        }
    }

    bot.answer_callback_query(&q.id).await?;
    Ok(())
}
