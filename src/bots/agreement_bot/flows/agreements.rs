use crate::observability::alerts::Severity;
use crate::observability::{ALERTS, METRICS};
use chrono::{Months, Utc};
use notifine::db::DbPool;
use notifine::i18n::{t, t_with_args};
use notifine::models::{Agreement, Reminder};
use notifine::{
    delete_agreement, delete_reminders_by_agreement_id, find_agreement_by_id,
    find_agreement_user_by_telegram_id, find_agreements_by_user_id, find_reminders_by_agreement_id,
};
use std::collections::BTreeMap;
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

    if let Some(duration) = agreement.contract_duration_years {
        parts.push(format!(
            "{}: {}",
            t(language, "agreement.view.field_contract_duration"),
            t_with_args(
                language,
                "agreement.view.contract_duration_years",
                &[&duration.to_string()]
            )
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
        let monthly_due_days: Vec<_> = pending
            .iter()
            .filter(|r| r.reminder_type == "due_day")
            .take(5)
            .copied()
            .collect();

        let pre_notifies: Vec<_> = pending
            .iter()
            .filter(|r| r.reminder_type == "pre_notify")
            .copied()
            .collect();

        let yearly: Vec<_> = pending
            .iter()
            .filter(|r| r.reminder_type == "yearly_increase")
            .copied()
            .collect();

        let five_year: Vec<_> = pending
            .iter()
            .filter(|r| r.reminder_type == "five_year_notice")
            .copied()
            .collect();

        let ten_year: Vec<_> = pending
            .iter()
            .filter(|r| r.reminder_type == "ten_year_notice")
            .copied()
            .collect();

        if !monthly_due_days.is_empty() {
            parts.push(String::new());
            parts.push(t(language, "agreement.view.reminders_monthly_title"));
            for reminder in monthly_due_days {
                parts.push(format_reminder_line_with_pre(
                    reminder,
                    agreement,
                    &pre_notifies,
                    language,
                ));
            }
        }

        if !yearly.is_empty() {
            parts.push(String::new());
            parts.push(t(language, "agreement.view.reminders_yearly_title"));
            for line in group_yearly_reminders(&yearly, language) {
                parts.push(line);
            }
        }

        if !five_year.is_empty() {
            parts.push(String::new());
            parts.push(t(language, "agreement.view.reminders_five_year_title"));
            for line in group_milestone_reminders(
                &five_year,
                language,
                "five_year",
                agreement.start_date,
                agreement.user_role.as_deref(),
            ) {
                parts.push(line);
            }
        }

        if !ten_year.is_empty() {
            parts.push(String::new());
            parts.push(t(language, "agreement.view.reminders_ten_year_title"));
            for line in group_milestone_reminders(
                &ten_year,
                language,
                "ten_year",
                agreement.start_date,
                agreement.user_role.as_deref(),
            ) {
                parts.push(line);
            }
        }
    }

    parts.join("\n")
}

#[allow(dead_code)]
fn group_reminders_by_due_date(
    reminders: &[&Reminder],
    language: &str,
    max_groups: usize,
) -> Vec<String> {
    let mut grouped: BTreeMap<chrono::NaiveDate, Vec<&Reminder>> = BTreeMap::new();

    for reminder in reminders {
        grouped.entry(reminder.due_date).or_default().push(reminder);
    }

    let mut lines = Vec::new();
    for (due_date, group) in grouped.into_iter().take(max_groups) {
        let Some(earliest) = group.iter().min_by_key(|r| r.reminder_date) else {
            continue;
        };

        let timing_parts: Vec<String> = group
            .iter()
            .map(|r| format_time_before(r.due_date, r.reminder_date, language))
            .collect();

        let timing_str = if timing_parts.is_empty() {
            String::new()
        } else {
            format!(" ({})", timing_parts.join(", "))
        };

        lines.push(format!(
            "  {} - {}{}",
            due_date.format("%d.%m.%Y"),
            earliest.title,
            timing_str
        ));
    }

    lines
}

fn format_timing_with_count(timing_parts: &[String], language: &str) -> String {
    if timing_parts.is_empty() {
        return String::new();
    }
    let count = timing_parts.len();
    let timing_list = timing_parts.join(", ");
    let count_text = t_with_args(
        language,
        "agreement.view.notification_count",
        &[&count.to_string()],
    );
    format!("({} {})", timing_list, count_text)
}

fn group_milestone_reminders(
    reminders: &[&Reminder],
    language: &str,
    milestone_type: &str,
    start_date: Option<chrono::NaiveDate>,
    user_role: Option<&str>,
) -> Vec<String> {
    if reminders.is_empty() {
        return Vec::new();
    }

    let mut grouped_by_due: BTreeMap<chrono::NaiveDate, Vec<&Reminder>> = BTreeMap::new();
    for reminder in reminders {
        grouped_by_due
            .entry(reminder.due_date)
            .or_default()
            .push(reminder);
    }

    let mut lines = Vec::new();

    if milestone_type == "five_year" {
        let message_key = match user_role {
            Some("landlord") => "agreement.view.five_year_view_landlord",
            _ => "agreement.view.five_year_view_tenant",
        };
        lines.push(format!("  \"{}\"", t(language, message_key)));

        let milestone_dates: Vec<String> = grouped_by_due
            .keys()
            .filter_map(|due_date| {
                let years = start_date.and_then(|sd| due_date.years_since(sd));
                years.filter(|&y| y > 0).map(|y| {
                    t_with_args(
                        language,
                        "agreement.view.five_year_milestone",
                        &[&y.to_string(), &due_date.format("%d.%m.%Y").to_string()],
                    )
                })
            })
            .collect();

        if !milestone_dates.is_empty() {
            lines.push(format!("  {}", milestone_dates.join(", ")));
        }

        if let Some(first_due) = grouped_by_due.keys().next() {
            if let Some(group) = grouped_by_due.get(first_due) {
                let timing_parts: Vec<String> = group
                    .iter()
                    .map(|r| format_time_before(r.due_date, r.reminder_date, language))
                    .collect();
                let timing_str = format_timing_with_count(&timing_parts, language);
                if !timing_str.is_empty() {
                    lines.push(format!("  {}", timing_str));
                }
            }
        }
    } else if milestone_type == "ten_year" {
        let message_key = match user_role {
            Some("landlord") => "agreement.view.ten_year_view_landlord",
            _ => "agreement.view.ten_year_view_tenant",
        };
        lines.push(format!("  \"{}\"", t(language, message_key)));

        if let Some(due_date) = grouped_by_due.keys().next() {
            let date_line = t_with_args(
                language,
                "agreement.view.ten_year_date",
                &[&due_date.format("%d.%m.%Y").to_string()],
            );
            lines.push(format!("  {}", date_line));

            if let Some(group) = grouped_by_due.get(due_date) {
                let timing_parts: Vec<String> = group
                    .iter()
                    .map(|r| format_time_before(r.due_date, r.reminder_date, language))
                    .collect();
                let timing_str = format_timing_with_count(&timing_parts, language);
                if !timing_str.is_empty() {
                    lines.push(format!("  {}", timing_str));
                }
            }
        }
    }

    lines
}

#[allow(dead_code)]
fn extract_base_title(title: &str) -> &str {
    title.lines().next().unwrap_or(title).trim()
}

fn group_yearly_reminders(reminders: &[&Reminder], language: &str) -> Vec<String> {
    if reminders.is_empty() {
        return Vec::new();
    }

    let mut grouped_by_title: BTreeMap<&str, Vec<&Reminder>> = BTreeMap::new();
    for reminder in reminders {
        grouped_by_title
            .entry(&reminder.title)
            .or_default()
            .push(reminder);
    }

    let mut lines = Vec::new();

    for (title, group) in grouped_by_title {
        let mut due_dates: Vec<_> = group.iter().map(|r| r.due_date).collect();
        due_dates.sort();
        due_dates.dedup();

        let Some(&first_date) = due_dates.first() else {
            continue;
        };
        let Some(&last_date) = due_dates.last() else {
            continue;
        };

        let day_month = first_date.format("%d.%m").to_string();

        let timing_parts: Vec<String> = group
            .iter()
            .filter(|r| r.due_date == first_date)
            .map(|r| format_time_before(r.due_date, r.reminder_date, language))
            .collect();

        let timing_str = format_timing_with_count(&timing_parts, language);

        lines.push(format!(
            "  {} - \"{}\"",
            t_with_args(language, "agreement.view.yearly_summary", &[&day_month]),
            title
        ));
        lines.push(format!(
            "  {}",
            t_with_args(
                language,
                "agreement.view.first_last_dates",
                &[
                    &first_date.format("%d.%m.%Y").to_string(),
                    &last_date.format("%d.%m.%Y").to_string()
                ]
            ),
        ));
        if !timing_str.is_empty() {
            lines.push(format!("  {}", timing_str));
        }
    }

    lines
}

fn format_time_before(
    due_date: chrono::NaiveDate,
    reminder_date: chrono::NaiveDate,
    language: &str,
) -> String {
    let days_diff = (due_date - reminder_date).num_days();

    if days_diff <= 0 {
        return t(language, "agreement.view.reminder_same_day");
    }

    let mut test_date = reminder_date;
    let mut months = 0u32;
    while let Some(next) = test_date.checked_add_months(Months::new(1)) {
        if next <= due_date {
            test_date = next;
            months += 1;
        } else {
            break;
        }
    }

    if months > 0 {
        t_with_args(
            language,
            "agreement.view.months_before",
            &[&months.to_string()],
        )
    } else {
        t_with_args(
            language,
            "agreement.view.days_before",
            &[&days_diff.to_string()],
        )
    }
}

#[allow(dead_code)]
fn format_reminder_line(reminder: &Reminder, agreement: &Agreement) -> String {
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

fn format_reminder_line_with_pre(
    reminder: &notifine::models::Reminder,
    agreement: &Agreement,
    pre_notifies: &[&notifine::models::Reminder],
    language: &str,
) -> String {
    let amount_str = reminder
        .amount
        .as_ref()
        .map(|a| format!("({} {})", a, agreement.currency))
        .unwrap_or_default();

    let pre_reminder_text = pre_notifies
        .iter()
        .find(|p| p.due_date == reminder.due_date)
        .map(|p| {
            let days_before = (reminder.reminder_date - p.reminder_date).num_days().abs();
            if days_before > 0 {
                t_with_args(
                    language,
                    "agreement.view.reminder_with_pre",
                    &[&days_before.to_string()],
                )
            } else {
                t(language, "agreement.view.reminder_due_day_only")
            }
        })
        .unwrap_or_else(|| t(language, "agreement.view.reminder_due_day_only"));

    format!(
        "  {} - {}{}{}",
        reminder.reminder_date.format("%d.%m.%Y"),
        reminder.title,
        if amount_str.is_empty() {
            String::new()
        } else {
            format!(" {}", amount_str)
        },
        if pre_reminder_text.is_empty() {
            String::new()
        } else {
            format!(" {}", pre_reminder_text)
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
