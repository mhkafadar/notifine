use crate::bots::bot_service::TelegramMessage;
use crate::observability::METRICS;
use bigdecimal::BigDecimal;
use chrono::{Datelike, Months, NaiveDate, Utc};
use notifine::db::{DbError, DbPool};
use notifine::i18n::{t, t_with_args};
use notifine::models::{Agreement, NewAgreement, NewReminder};
use notifine::{
    clear_conversation_state, create_agreement, create_reminders_batch,
    find_agreement_by_user_and_title, find_agreement_user_by_telegram_id, get_conversation_state,
    set_conversation_state,
};
use std::str::FromStr;
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup};

use crate::bots::agreement_bot::keyboards::{
    build_confirm_keyboard, build_contract_duration_keyboard, build_currency_keyboard,
    build_pre_reminder_keyboard, build_yes_no_keyboard, days_in_month,
};
use crate::bots::agreement_bot::types::{states, RentDraft, STATE_EXPIRY_MINUTES};
use crate::bots::agreement_bot::utils::{
    confirm_selection_and_send_next, get_user_language, send_message_with_keyboard,
    send_telegram_message,
};

pub async fn handle_rent_callback(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
    data: &str,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);
    bot.answer_callback_query(&q.id).await?;

    let msg = match &q.message {
        Some(m) => m,
        None => return Ok(()),
    };
    let chat_id = msg.chat.id.0;
    let thread_id = msg.thread_id;

    let state = get_conversation_state(pool, user_id).ok().flatten();
    let mut draft: RentDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    if let Some(role) = data.strip_prefix("rent:role:") {
        draft.user_role = Some(role.to_string());
        let role_display = if role == "tenant" {
            t(&language, "agreement.rent.step2_role.tenant_button")
        } else {
            t(&language, "agreement.rent.step2_role.landlord_button")
        };
        let selected_text = t_with_args(
            &language,
            "common.selected_with_context.role",
            &[&role_display],
        );
        let current_year = Utc::now().year().to_string();
        let next_message = format!(
            "{}\n\n{}",
            t_with_args(&language, "common.step_progress", &["3", "12"]),
            t_with_args(
                &language,
                "agreement.date_picker.enter_year",
                &[&current_year]
            )
        );
        let keyboard = InlineKeyboardMarkup::default();
        update_state(pool, user_id, states::RENT_START_YEAR, &draft);
        confirm_selection_and_send_next(
            bot,
            chat_id,
            thread_id,
            msg.id,
            &selected_text,
            &next_message,
            keyboard,
        )
        .await?;
    } else if let Some(duration_str) = data.strip_prefix("rent:duration:") {
        if duration_str == "other" {
            let selected_text = t_with_args(
                &language,
                "common.selected_with_context.duration",
                &[&t(&language, "agreement.rent.contract_duration.other")],
            );
            let next_message = format!(
                "{}\n\n{}",
                t_with_args(&language, "common.step_progress", &["6", "12"]),
                t(&language, "agreement.rent.contract_duration.custom_prompt")
            );
            let keyboard = InlineKeyboardMarkup::default();
            update_state(pool, user_id, states::RENT_CONTRACT_DURATION_CUSTOM, &draft);
            confirm_selection_and_send_next(
                bot,
                chat_id,
                thread_id,
                msg.id,
                &selected_text,
                &next_message,
                keyboard,
            )
            .await?;
        } else if let Ok(duration) = duration_str.parse::<i32>() {
            draft.contract_duration = Some(duration);
            let duration_key = match duration {
                1 => "agreement.rent.contract_duration.1_year",
                2 => "agreement.rent.contract_duration.2_years",
                3 => "agreement.rent.contract_duration.3_years",
                _ => "agreement.rent.contract_duration.1_year",
            };
            let duration_display = t(&language, duration_key);
            let selected_text = t_with_args(
                &language,
                "common.selected_with_context.duration",
                &[&duration_display],
            );
            let next_message = format!(
                "{}\n\n{}",
                t_with_args(&language, "common.step_progress", &["7", "12"]),
                t(&language, "agreement.rent.step5_currency.prompt")
            );
            let keyboard = build_currency_keyboard();
            update_state(pool, user_id, states::RENT_CURRENCY, &draft);
            confirm_selection_and_send_next(
                bot,
                chat_id,
                thread_id,
                msg.id,
                &selected_text,
                &next_message,
                keyboard,
            )
            .await?;
        }
    } else if let Some(currency) = data.strip_prefix("rent:currency:") {
        if !["TRY", "EUR", "USD", "GBP"].contains(&currency) {
            return Ok(());
        }
        draft.currency = Some(currency.to_string());
        let currency_display = match currency {
            "TRY" => "🇹🇷 TRY",
            "EUR" => "🇪🇺 EUR",
            "USD" => "🇺🇸 USD",
            "GBP" => "🇬🇧 GBP",
            _ => currency,
        };
        let selected_text = t_with_args(
            &language,
            "common.selected_with_context.currency",
            &[currency_display],
        );
        let next_message = format!(
            "{}\n\n{}",
            t_with_args(&language, "common.step_progress", &["8", "12"]),
            t(&language, "agreement.rent.step6_amount.prompt")
        );
        let keyboard = InlineKeyboardMarkup::default();
        update_state(pool, user_id, states::RENT_AMOUNT, &draft);
        confirm_selection_and_send_next(
            bot,
            chat_id,
            thread_id,
            msg.id,
            &selected_text,
            &next_message,
            keyboard,
        )
        .await?;
    } else if let Some(answer) = data.strip_prefix("rent:monthly:") {
        draft.has_monthly_reminder = Some(answer == "yes");
        let answer_display = if answer == "yes" {
            t(&language, "common.selected_with_context.enabled")
        } else {
            t(&language, "common.selected_with_context.disabled")
        };
        let selected_text = t_with_args(
            &language,
            "common.selected_with_context.monthly_reminder",
            &[&answer_display],
        );
        if answer == "yes" {
            let next_message = format!(
                "{}\n\n{}",
                t_with_args(&language, "common.step_progress", &["11", "12"]),
                t(&language, "agreement.rent.step9_pre_reminder.prompt")
            );
            let keyboard = build_pre_reminder_keyboard(&language);
            update_state(pool, user_id, states::RENT_REMINDER_TIMING, &draft);
            confirm_selection_and_send_next(
                bot,
                chat_id,
                thread_id,
                msg.id,
                &selected_text,
                &next_message,
                keyboard,
            )
            .await?;
        } else {
            let next_message = format!(
                "{}\n\n{}",
                t_with_args(&language, "common.step_progress", &["12", "12"]),
                t(&language, "agreement.rent.step10_yearly_increase.prompt")
            );
            let keyboard = build_yes_no_keyboard(&language, "rent:yearly");
            update_state(pool, user_id, states::RENT_YEARLY_INCREASE, &draft);
            confirm_selection_and_send_next(
                bot,
                chat_id,
                thread_id,
                msg.id,
                &selected_text,
                &next_message,
                keyboard,
            )
            .await?;
        }
    } else if let Some(timing) = data.strip_prefix("rent:timing:") {
        draft.reminder_timing = Some(timing.to_string());
        let timing_key = match timing {
            "1_day_before" => "agreement.rent.step9_pre_reminder.1_day_before",
            "3_days_before" => "agreement.rent.step9_pre_reminder.3_days_before",
            "1_week_before" => "agreement.rent.step9_pre_reminder.1_week_before",
            _ => "agreement.rent.step9_pre_reminder.no_extra",
        };
        let timing_display = t(&language, timing_key);
        let selected_text = t_with_args(
            &language,
            "common.selected_with_context.pre_reminder",
            &[&timing_display],
        );
        let next_message = format!(
            "{}\n\n{}",
            t_with_args(&language, "common.step_progress", &["12", "12"]),
            t(&language, "agreement.rent.step10_yearly_increase.prompt")
        );
        let keyboard = build_yes_no_keyboard(&language, "rent:yearly");
        update_state(pool, user_id, states::RENT_YEARLY_INCREASE, &draft);
        confirm_selection_and_send_next(
            bot,
            chat_id,
            thread_id,
            msg.id,
            &selected_text,
            &next_message,
            keyboard,
        )
        .await?;
    } else if let Some(answer) = data.strip_prefix("rent:yearly:") {
        draft.has_yearly_increase_reminder = Some(answer == "yes");
        let answer_display = if answer == "yes" {
            t(&language, "common.selected_with_context.enabled")
        } else {
            t(&language, "common.selected_with_context.disabled")
        };
        let selected_text = t_with_args(
            &language,
            "common.selected_with_context.yearly_increase",
            &[&answer_display],
        );
        bot.edit_message_text(msg.chat.id, msg.id, &selected_text)
            .await?;
        show_rent_summary(pool, bot, chat_id, thread_id, user_id, &language, &draft).await?;
    } else if data == "rent:confirm" {
        save_rent_agreement(
            pool, bot, chat_id, thread_id, msg.id, user_id, &language, &draft,
        )
        .await?;
    }

    Ok(())
}

fn update_state(pool: &DbPool, user_id: i64, new_state: &str, draft: &RentDraft) {
    let expires_at = Utc::now() + chrono::Duration::minutes(STATE_EXPIRY_MINUTES);
    if let Err(e) = set_conversation_state(
        pool,
        user_id,
        new_state,
        Some(serde_json::to_value(draft).unwrap_or_default()),
        expires_at,
    ) {
        tracing::error!("Failed to update state: {:?}", e);
    }
}

async fn show_rent_summary(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    draft: &RentDraft,
) -> ResponseResult<()> {
    let role_display = match draft.user_role.as_deref() {
        Some("tenant") => t(language, "agreement.rent.step10_summary.role_tenant"),
        Some("landlord") => t(language, "agreement.rent.step10_summary.role_landlord"),
        _ => "-".to_string(),
    };

    let monthly_status = if draft.has_monthly_reminder.unwrap_or(false) {
        t(language, "agreement.rent.step10_summary.enabled")
    } else {
        t(language, "agreement.rent.step10_summary.disabled")
    };

    let yearly_status = if draft.has_yearly_increase_reminder.unwrap_or(false) {
        t(language, "agreement.rent.step10_summary.enabled")
    } else {
        t(language, "agreement.rent.step10_summary.disabled")
    };

    let contract_duration_str = draft.contract_duration.unwrap_or(1).to_string();

    let summary = format!(
        "{}\n\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n\n{}",
        t(language, "agreement.rent.step10_summary.title"),
        t_with_args(
            language,
            "agreement.rent.step10_summary.agreement_name",
            &[draft.title.as_deref().unwrap_or("-")]
        ),
        t_with_args(
            language,
            "agreement.rent.step10_summary.role",
            &[&role_display]
        ),
        t_with_args(
            language,
            "agreement.rent.step10_summary.start_date",
            &[draft.start_date.as_deref().unwrap_or("-")]
        ),
        t_with_args(
            language,
            "agreement.rent.step10_summary.contract_duration",
            &[&contract_duration_str]
        ),
        t_with_args(
            language,
            "agreement.rent.step10_summary.amount",
            &[
                draft.rent_amount.as_deref().unwrap_or("-"),
                draft.currency.as_deref().unwrap_or("TRY")
            ]
        ),
        t_with_args(
            language,
            "agreement.rent.step10_summary.due_day",
            &[&draft
                .due_day
                .map(|d| d.to_string())
                .unwrap_or("-".to_string())]
        ),
        t_with_args(
            language,
            "agreement.rent.step10_summary.monthly_reminder",
            &[&monthly_status]
        ),
        t_with_args(
            language,
            "agreement.rent.step10_summary.yearly_increase",
            &[&yearly_status]
        ),
        t(language, "agreement.rent.step10_summary.confirm_prompt")
    );

    let keyboard = build_confirm_keyboard(language, "rent");
    update_state(pool, user_id, states::RENT_SUMMARY, draft);

    send_message_with_keyboard(bot, chat_id, thread_id, &summary, keyboard).await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn save_rent_agreement(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    _message_id: teloxide::types::MessageId,
    user_id: i64,
    language: &str,
    draft: &RentDraft,
) -> ResponseResult<()> {
    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        _ => {
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.errors.user_not_found"),
                },
            )
            .await?;
            return Ok(());
        }
    };

    let start_date = draft
        .start_date
        .as_ref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%d.%m.%Y").ok());

    let rent_amount = draft
        .rent_amount
        .as_ref()
        .and_then(|s| BigDecimal::from_str(s).ok());

    let contract_duration = draft.contract_duration.unwrap_or(1);

    let new_agreement = NewAgreement {
        user_id: user.id,
        agreement_type: "rent",
        title: draft.title.as_deref().unwrap_or("Rent Agreement"),
        user_role: draft.user_role.as_deref(),
        start_date,
        currency: draft.currency.as_deref().unwrap_or("TRY"),
        rent_amount,
        due_day: draft.due_day,
        has_monthly_reminder: draft.has_monthly_reminder.unwrap_or(false),
        reminder_timing: draft.reminder_timing.as_deref(),
        reminder_days_before: None,
        has_yearly_increase_reminder: draft.has_yearly_increase_reminder.unwrap_or(false),
        description: None,
        has_ten_year_reminder: true,
        has_five_year_reminder: true,
        contract_duration_years: Some(contract_duration),
    };

    let agreement = match create_agreement(pool, new_agreement) {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("Failed to create agreement: {:?}", e);
            METRICS.increment_errors();
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.errors.database_error"),
                },
            )
            .await?;
            return Ok(());
        }
    };

    if draft.has_monthly_reminder.unwrap_or(false) {
        if let Err(e) = generate_monthly_reminders(pool, &agreement, draft, start_date, language) {
            tracing::error!(
                "Failed to generate monthly reminders for agreement {}: {:?}",
                agreement.id,
                e
            );
            METRICS.increment_errors();
        }
    }

    if draft.has_yearly_increase_reminder.unwrap_or(false) {
        if let Err(e) =
            generate_yearly_increase_reminders(pool, &agreement, draft, start_date, language)
        {
            tracing::error!(
                "Failed to generate yearly increase reminders for agreement {}: {:?}",
                agreement.id,
                e
            );
            METRICS.increment_errors();
        }
    }

    if let Err(e) =
        generate_ten_year_reminders(pool, &agreement, start_date, contract_duration, language)
    {
        tracing::error!(
            "Failed to generate 10-year reminders for agreement {}: {:?}",
            agreement.id,
            e
        );
        METRICS.increment_errors();
    }

    if let Err(e) = generate_five_year_reminders(
        pool,
        &agreement,
        draft,
        start_date,
        contract_duration,
        language,
    ) {
        tracing::error!(
            "Failed to generate 5-year reminders for agreement {}: {:?}",
            agreement.id,
            e
        );
        METRICS.increment_errors();
    }

    if let Err(e) = clear_conversation_state(pool, user_id) {
        tracing::warn!(
            "Failed to clear conversation state for user {}: {:?}",
            user_id,
            e
        );
    }

    let role_display = match draft.user_role.as_deref() {
        Some("tenant") => t(language, "agreement.rent.step10_summary.role_tenant"),
        Some("landlord") => t(language, "agreement.rent.step10_summary.role_landlord"),
        _ => "-".to_string(),
    };

    let start_year = start_date.map(|d| d.year()).unwrap_or(2025);
    let end_year = start_year + contract_duration + 10;

    let success_message = format!(
        "{}\n\n{}\n\n{}",
        t(language, "agreement.rent.success.title"),
        t_with_args(
            language,
            "agreement.rent.success.details",
            &[
                draft.title.as_deref().unwrap_or("-"),
                &role_display,
                &start_year.to_string(),
                &end_year.to_string(),
                draft.rent_amount.as_deref().unwrap_or("-"),
                draft.currency.as_deref().unwrap_or("TRY"),
            ]
        ),
        t(language, "agreement.rent.success.list_hint")
    );

    send_telegram_message(
        bot,
        TelegramMessage {
            chat_id,
            thread_id,
            message: success_message,
        },
    )
    .await?;

    Ok(())
}

fn generate_monthly_reminders(
    pool: &DbPool,
    agreement: &notifine::models::Agreement,
    draft: &RentDraft,
    start_date: Option<NaiveDate>,
    language: &str,
) -> Result<(), notifine::db::DbError> {
    let start = match start_date {
        Some(d) => d,
        None => return Ok(()),
    };

    let due_day = draft.due_day.unwrap_or(1);
    let timing = draft.reminder_timing.as_deref().unwrap_or("same_day");
    let days_before = match timing {
        "1_day_before" => 1,
        "3_days_before" => 3,
        "1_week_before" => 7,
        _ => 0,
    };

    let today = Utc::now().date_naive();
    let mut reminders = Vec::new();

    for month_offset in 0..12 {
        let mut target_year = start.year();
        let mut target_month = start.month() as i32 + month_offset;

        while target_month > 12 {
            target_month -= 12;
            target_year += 1;
        }

        let days_in_month = days_in_month(target_year, target_month as u32);
        let actual_day = due_day.min(days_in_month as i32) as u32;

        if let Some(due_date) =
            NaiveDate::from_ymd_opt(target_year, target_month as u32, actual_day)
        {
            if due_date <= today {
                continue;
            }

            let title = if draft.user_role.as_deref() == Some("tenant") {
                t(language, "agreement.rent.success.payment_title")
            } else {
                t(language, "agreement.rent.success.collection_title")
            };

            if days_before > 0 {
                let pre_reminder_date = due_date - chrono::Duration::days(days_before);
                reminders.push(NewReminder {
                    agreement_id: agreement.id,
                    reminder_type: "pre_notify".to_string(),
                    title: title.clone(),
                    amount: agreement.rent_amount.clone(),
                    due_date,
                    reminder_date: pre_reminder_date,
                });
            }

            reminders.push(NewReminder {
                agreement_id: agreement.id,
                reminder_type: "due_day".to_string(),
                title,
                amount: agreement.rent_amount.clone(),
                due_date,
                reminder_date: due_date,
            });
        }
    }

    if !reminders.is_empty() {
        create_reminders_batch(pool, reminders)?;
    }

    Ok(())
}

fn generate_yearly_increase_reminders(
    pool: &DbPool,
    agreement: &Agreement,
    draft: &RentDraft,
    start_date: Option<NaiveDate>,
    language: &str,
) -> Result<(), DbError> {
    let start = match start_date {
        Some(d) => d,
        None => return Ok(()),
    };

    if !draft.has_yearly_increase_reminder.unwrap_or(false) {
        return Ok(());
    }

    let today = Utc::now().date_naive();
    let mut reminders = Vec::new();

    for year in 1..=11 {
        let anniversary = match start.checked_add_months(Months::new(12 * year)) {
            Some(d) => d,
            None => continue,
        };

        let title = if draft.user_role.as_deref() == Some("landlord") {
            t(language, "agreement.rent.yearly_increase.landlord_title")
        } else {
            t(language, "agreement.rent.yearly_increase.tenant_title")
        };

        if let Some(reminder_4m) = anniversary.checked_sub_months(Months::new(4)) {
            if reminder_4m > today {
                reminders.push(NewReminder {
                    agreement_id: agreement.id,
                    reminder_type: "yearly_increase".to_string(),
                    title: title.clone(),
                    amount: None,
                    due_date: anniversary,
                    reminder_date: reminder_4m,
                });
            }
        }

        if let Some(reminder_3m) = anniversary.checked_sub_months(Months::new(3)) {
            if reminder_3m > today {
                reminders.push(NewReminder {
                    agreement_id: agreement.id,
                    reminder_type: "yearly_increase".to_string(),
                    title: title.clone(),
                    amount: None,
                    due_date: anniversary,
                    reminder_date: reminder_3m,
                });
            }
        }
    }

    if !reminders.is_empty() {
        create_reminders_batch(pool, reminders)?;
    }

    Ok(())
}

fn generate_ten_year_reminders(
    pool: &DbPool,
    agreement: &Agreement,
    start_date: Option<NaiveDate>,
    contract_duration: i32,
    language: &str,
) -> Result<(), DbError> {
    let start = match start_date {
        Some(d) => d,
        None => return Ok(()),
    };

    let total_months = (contract_duration + 10) * 12;
    let ten_year_milestone = match start.checked_add_months(Months::new(total_months as u32)) {
        Some(d) => d,
        None => return Ok(()),
    };

    let today = Utc::now().date_naive();
    let mut reminders = Vec::new();
    let is_landlord = agreement.user_role.as_deref() == Some("landlord");

    if let Some(reminder_6m) = ten_year_milestone.checked_sub_months(Months::new(6)) {
        if reminder_6m > today {
            let title_key = if is_landlord {
                "agreement.rent.ten_year.landlord_6_months_title"
            } else {
                "agreement.rent.ten_year.tenant_6_months_title"
            };
            reminders.push(NewReminder {
                agreement_id: agreement.id,
                reminder_type: "ten_year_notice".to_string(),
                title: t(language, title_key),
                amount: None,
                due_date: ten_year_milestone,
                reminder_date: reminder_6m,
            });
        }
    }

    if let Some(reminder_4m) = ten_year_milestone.checked_sub_months(Months::new(4)) {
        if reminder_4m > today {
            let notice_deadline = ten_year_milestone
                .checked_sub_months(Months::new(3))
                .unwrap_or(ten_year_milestone);
            let title_key = if is_landlord {
                "agreement.rent.ten_year.landlord_4_months_title"
            } else {
                "agreement.rent.ten_year.tenant_4_months_title"
            };
            let title = t_with_args(
                language,
                title_key,
                &[&notice_deadline.format("%d.%m.%Y").to_string()],
            );
            reminders.push(NewReminder {
                agreement_id: agreement.id,
                reminder_type: "ten_year_notice".to_string(),
                title,
                amount: None,
                due_date: ten_year_milestone,
                reminder_date: reminder_4m,
            });
        }
    }

    if let Some(reminder_3m) = ten_year_milestone.checked_sub_months(Months::new(3)) {
        if reminder_3m > today {
            let title_key = if is_landlord {
                "agreement.rent.ten_year.landlord_3_months_title"
            } else {
                "agreement.rent.ten_year.tenant_3_months_title"
            };
            reminders.push(NewReminder {
                agreement_id: agreement.id,
                reminder_type: "ten_year_notice".to_string(),
                title: t(language, title_key),
                amount: None,
                due_date: ten_year_milestone,
                reminder_date: reminder_3m,
            });
        }
    }

    if !reminders.is_empty() {
        create_reminders_batch(pool, reminders)?;
    }

    Ok(())
}

fn generate_five_year_reminders(
    pool: &DbPool,
    agreement: &Agreement,
    draft: &RentDraft,
    start_date: Option<NaiveDate>,
    contract_duration: i32,
    language: &str,
) -> Result<(), DbError> {
    let start = match start_date {
        Some(d) => d,
        None => return Ok(()),
    };

    if !agreement.has_five_year_reminder {
        return Ok(());
    }

    let ten_year_period_months = ((contract_duration + 10) * 12) as u32;
    let ten_year_end = match start.checked_add_months(Months::new(ten_year_period_months)) {
        Some(d) => d,
        None => return Ok(()),
    };

    let today = Utc::now().date_naive();
    let mut reminders = Vec::new();

    for period in 1..=3 {
        let five_year_milestone = match start.checked_add_months(Months::new(12 * 5 * period)) {
            Some(d) => d,
            None => continue,
        };

        if five_year_milestone >= ten_year_end {
            continue;
        }

        let period_years = (5 * period).to_string();
        let title = if draft.user_role.as_deref() == Some("landlord") {
            t_with_args(
                language,
                "agreement.rent.five_year.landlord_title",
                &[&period_years],
            )
        } else {
            t_with_args(
                language,
                "agreement.rent.five_year.tenant_title",
                &[&period_years],
            )
        };

        if let Some(reminder_6m) = five_year_milestone.checked_sub_months(Months::new(6)) {
            if reminder_6m > today {
                reminders.push(NewReminder {
                    agreement_id: agreement.id,
                    reminder_type: "five_year_notice".to_string(),
                    title: title.clone(),
                    amount: None,
                    due_date: five_year_milestone,
                    reminder_date: reminder_6m,
                });
            }
        }

        if let Some(reminder_3m) = five_year_milestone.checked_sub_months(Months::new(3)) {
            if reminder_3m > today {
                reminders.push(NewReminder {
                    agreement_id: agreement.id,
                    reminder_type: "five_year_notice".to_string(),
                    title: title.clone(),
                    amount: None,
                    due_date: five_year_milestone,
                    reminder_date: reminder_3m,
                });
            }
        }

        if let Some(reminder_31d) = five_year_milestone.checked_sub_days(chrono::Days::new(31)) {
            if reminder_31d > today {
                let title_31d = t_with_args(
                    language,
                    "agreement.rent.five_year.notice_deadline_title",
                    &[&period_years, "31"],
                );
                reminders.push(NewReminder {
                    agreement_id: agreement.id,
                    reminder_type: "five_year_notice".to_string(),
                    title: title_31d,
                    amount: None,
                    due_date: five_year_milestone,
                    reminder_date: reminder_31d,
                });
            }
        }
    }

    if !reminders.is_empty() {
        create_reminders_batch(pool, reminders)?;
    }

    Ok(())
}

pub async fn handle_rent_title_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    let title = text.trim();

    if title.is_empty() {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.title_required"),
            },
        )
        .await?;
        return Ok(());
    }

    if title.len() > 50 {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.title_too_long"),
            },
        )
        .await?;
        return Ok(());
    }

    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        _ => return Ok(()),
    };

    if let Ok(Some(_)) = find_agreement_by_user_and_title(pool, user.id, title) {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.duplicate_title"),
            },
        )
        .await?;
        return Ok(());
    }

    let state = get_conversation_state(pool, user_id).ok().flatten();
    let mut draft: RentDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    draft.title = Some(title.to_string());

    let message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["2", "12"]),
        t(language, "agreement.rent.step2_role.prompt")
    );

    let keyboard = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            t(language, "agreement.rent.step2_role.tenant_button"),
            "rent:role:tenant",
        ),
        InlineKeyboardButton::callback(
            t(language, "agreement.rent.step2_role.landlord_button"),
            "rent:role:landlord",
        ),
    ]]);

    update_state(pool, user_id, states::RENT_ROLE, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

pub async fn handle_rent_amount_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    let amount_str = text.trim().replace(',', ".").replace([' ', '_'], "");

    const MAX_RENT_AMOUNT: f64 = 10_000_000.0;
    let amount = match amount_str.parse::<f64>() {
        Ok(a) if a > 0.0 && a <= MAX_RENT_AMOUNT => a,
        _ => {
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.validation.invalid_amount"),
                },
            )
            .await?;
            return Ok(());
        }
    };

    let state = get_conversation_state(pool, user_id).ok().flatten();
    let mut draft: RentDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    draft.rent_amount = Some(format!("{:.2}", amount));

    let message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["9", "12"]),
        t(language, "agreement.rent.step7_due_day.prompt_text")
    );

    let keyboard = InlineKeyboardMarkup::default();
    update_state(pool, user_id, states::RENT_DUE_DAY, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

pub async fn handle_rent_due_day_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    use crate::bots::agreement_bot::types::sanitize_input;
    let day_str = sanitize_input(text);

    let day = match day_str.parse::<i32>() {
        Ok(d) if (1..=31).contains(&d) => d,
        _ => {
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.validation.invalid_day"),
                },
            )
            .await?;
            return Ok(());
        }
    };

    let state = get_conversation_state(pool, user_id).ok().flatten();
    let mut draft: RentDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    draft.due_day = Some(day);

    let mut message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["10", "12"]),
        t(language, "agreement.rent.step8_monthly_reminder.prompt")
    );
    if day >= 29 {
        message.push_str("\n\n");
        message.push_str(&t(language, "agreement.rent.step7_due_day.late_day_note"));
    }

    let keyboard = build_yes_no_keyboard(language, "rent:monthly");
    update_state(pool, user_id, states::RENT_MONTHLY_REMINDER, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

pub async fn handle_rent_start_year_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    use crate::bots::agreement_bot::types::sanitize_input;
    let year_str = sanitize_input(text);

    let year = match year_str.parse::<i32>() {
        Ok(y) if (1900..=2100).contains(&y) => y,
        _ => {
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.date_picker.invalid_year"),
                },
            )
            .await?;
            return Ok(());
        }
    };

    let state = get_conversation_state(pool, user_id).ok().flatten();
    let mut draft: RentDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    draft.start_year = Some(year);

    let message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["4", "12"]),
        t(language, "agreement.date_picker.enter_month")
    );
    let keyboard = InlineKeyboardMarkup::default();
    update_state(pool, user_id, states::RENT_START_MONTH, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

pub async fn handle_rent_start_month_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    use crate::bots::agreement_bot::types::sanitize_input;
    let month_str = sanitize_input(text);

    let month = match month_str.parse::<u32>() {
        Ok(m) if (1..=12).contains(&m) => m,
        _ => {
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.date_picker.invalid_month"),
                },
            )
            .await?;
            return Ok(());
        }
    };

    let state = get_conversation_state(pool, user_id).ok().flatten();
    let mut draft: RentDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    draft.start_month = Some(month);

    let year = draft.start_year.unwrap_or(Utc::now().year());
    let max_day = days_in_month(year, month);

    let message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["5", "12"]),
        t_with_args(
            language,
            "agreement.date_picker.enter_day",
            &[&max_day.to_string()]
        )
    );
    let keyboard = InlineKeyboardMarkup::default();
    update_state(pool, user_id, states::RENT_START_DAY, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

pub async fn handle_rent_start_day_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    use crate::bots::agreement_bot::types::sanitize_input;
    let day_str = sanitize_input(text);

    let state = get_conversation_state(pool, user_id).ok().flatten();
    let mut draft: RentDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    let year = match draft.start_year {
        Some(y) => y,
        None => {
            tracing::error!("Reached day input handler without year in draft");
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.errors.database_error"),
                },
            )
            .await?;
            return Ok(());
        }
    };
    let month = match draft.start_month {
        Some(m) => m,
        None => {
            tracing::error!("Reached day input handler without month in draft");
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.errors.database_error"),
                },
            )
            .await?;
            return Ok(());
        }
    };
    let max_day = days_in_month(year, month);

    let day = match day_str.parse::<u32>() {
        Ok(d) if d >= 1 && d <= max_day => d,
        _ => {
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t_with_args(
                        language,
                        "agreement.date_picker.invalid_day",
                        &[&max_day.to_string()],
                    ),
                },
            )
            .await?;
            return Ok(());
        }
    };

    draft.start_date = Some(format!("{:02}.{:02}.{}", day, month, year));
    draft.start_year = None;
    draft.start_month = None;

    let message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["6", "12"]),
        t(language, "agreement.rent.step4_contract_duration.prompt")
    );
    let keyboard = build_contract_duration_keyboard(language);
    update_state(pool, user_id, states::RENT_CONTRACT_DURATION, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

pub async fn handle_rent_contract_duration_custom_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    use crate::bots::agreement_bot::types::sanitize_input;
    let duration_str = sanitize_input(text);

    let duration = match duration_str.parse::<i32>() {
        Ok(d) if (1..=30).contains(&d) => d,
        _ => {
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(
                        language,
                        "agreement.rent.contract_duration.invalid_duration",
                    ),
                },
            )
            .await?;
            return Ok(());
        }
    };

    let state = get_conversation_state(pool, user_id).ok().flatten();
    let mut draft: RentDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    draft.contract_duration = Some(duration);

    let message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["7", "12"]),
        t(language, "agreement.rent.step5_currency.prompt")
    );
    let keyboard = build_currency_keyboard();
    update_state(pool, user_id, states::RENT_CURRENCY, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}
