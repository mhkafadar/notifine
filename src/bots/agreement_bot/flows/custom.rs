use crate::bots::bot_service::TelegramMessage;
use crate::observability::METRICS;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, Utc};
use notifine::db::DbPool;
use notifine::i18n::{t, t_with_args};
use notifine::models::{NewAgreement, NewReminder};
use notifine::{
    clear_conversation_state, create_agreement, create_reminders_batch,
    find_agreement_by_user_and_title, find_agreement_user_by_telegram_id, get_conversation_state,
    set_conversation_state,
};
use std::str::FromStr;
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup};

use crate::bots::agreement_bot::keyboards::{
    build_cancel_keyboard, build_confirm_keyboard, build_custom_calendar,
    build_custom_currency_keyboard, build_custom_timing_keyboard, build_menu_keyboard,
    build_reminder_list_keyboard,
};
use crate::bots::agreement_bot::types::{
    sanitize_input, states, CustomDraft, CustomReminderDraft, MAX_REMINDERS_PER_AGREEMENT,
    STATE_EXPIRY_MINUTES,
};
use crate::bots::agreement_bot::utils::{
    get_user_language, send_message_with_keyboard, send_telegram_message,
};

fn update_state(pool: &DbPool, user_id: i64, new_state: &str, draft: &CustomDraft) {
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

pub async fn handle_custom_callback(
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
    let mut draft: CustomDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    if data == "custom:skip_description" {
        start_reminder_flow(pool, bot, chat_id, thread_id, user_id, &language, &draft).await?;
    } else if data == "custom:skip_amount" {
        let message = t(&language, "agreement.custom.add_reminder.timing_prompt");
        let keyboard = build_custom_timing_keyboard(&language);
        update_state(pool, user_id, states::CUSTOM_REMINDER_TIMING, &draft);
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    } else if let Some(currency) = data.strip_prefix("custom:currency:") {
        if !["TRY", "EUR", "USD", "GBP"].contains(&currency) {
            return Ok(());
        }
        draft.currency = Some(currency.to_string());

        let message = t(&language, "agreement.custom.add_reminder.timing_prompt");
        let keyboard = build_custom_timing_keyboard(&language);
        update_state(pool, user_id, states::CUSTOM_REMINDER_TIMING, &draft);
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    } else if let Some(timing) = data.strip_prefix("custom:timing:") {
        if let Some(reminder) = draft.reminders.last_mut() {
            reminder.timing = Some(timing.to_string());
        }
        show_reminder_list(pool, bot, chat_id, thread_id, user_id, &language, &draft).await?;
    } else if data == "custom:add_another" {
        let message = t(
            &language,
            "agreement.custom.add_reminder.reminder_title_prompt",
        );
        let keyboard = build_cancel_keyboard(&language);
        update_state(pool, user_id, states::CUSTOM_REMINDER_TITLE, &draft);
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    } else if data == "custom:finish" {
        if draft.reminders.is_empty() {
            bot.edit_message_text(
                msg.chat.id,
                msg.id,
                t(&language, "agreement.validation.at_least_one_reminder"),
            )
            .await?;
            return Ok(());
        }
        show_custom_summary(
            pool, bot, chat_id, thread_id, msg.id, user_id, &language, &draft,
        )
        .await?;
    } else if data == "custom:confirm" {
        save_custom_agreement(
            pool, bot, chat_id, thread_id, msg.id, user_id, &language, &draft,
        )
        .await?;
    } else if data.starts_with("custom:cal:") {
        handle_calendar_callback(
            pool,
            bot,
            msg.chat.id,
            msg.id,
            user_id,
            &language,
            data,
            &mut draft,
        )
        .await?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_calendar_callback(
    pool: &DbPool,
    bot: &Bot,
    chat_id: ChatId,
    message_id: teloxide::types::MessageId,
    user_id: i64,
    language: &str,
    data: &str,
    draft: &mut CustomDraft,
) -> ResponseResult<()> {
    if data == "custom:cal:noop" {
        return Ok(());
    }

    if let Some(rest) = data.strip_prefix("custom:cal:prev:") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(year), Ok(month)) = (parts[0].parse::<i32>(), parts[1].parse::<u32>()) {
                let (new_year, new_month) = if month == 1 {
                    (year - 1, 12)
                } else {
                    (year, month - 1)
                };
                if let Some(new_date) = NaiveDate::from_ymd_opt(new_year, new_month, 1) {
                    let keyboard = build_custom_calendar(language, new_date);
                    let message = t(language, "agreement.custom.add_reminder.date_prompt");
                    bot.edit_message_text(chat_id, message_id, &message)
                        .reply_markup(keyboard)
                        .await?;
                }
            }
        }
    } else if let Some(rest) = data.strip_prefix("custom:cal:next:") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(year), Ok(month)) = (parts[0].parse::<i32>(), parts[1].parse::<u32>()) {
                let (new_year, new_month) = if month == 12 {
                    (year + 1, 1)
                } else {
                    (year, month + 1)
                };
                if let Some(new_date) = NaiveDate::from_ymd_opt(new_year, new_month, 1) {
                    let keyboard = build_custom_calendar(language, new_date);
                    let message = t(language, "agreement.custom.add_reminder.date_prompt");
                    bot.edit_message_text(chat_id, message_id, &message)
                        .reply_markup(keyboard)
                        .await?;
                }
            }
        }
    } else if let Some(rest) = data.strip_prefix("custom:cal:day:") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() == 3 {
            if let (Ok(year), Ok(month), Ok(day)) = (
                parts[0].parse::<i32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                if let Some(selected_naive) = NaiveDate::from_ymd_opt(year, month, day) {
                    let today = Utc::now().date_naive();
                    if selected_naive < today {
                        let message = format!(
                            "{}\n\n{}",
                            t(language, "agreement.validation.past_date"),
                            t(language, "agreement.custom.add_reminder.date_prompt")
                        );
                        let keyboard = build_custom_calendar(language, selected_naive);
                        bot.edit_message_text(chat_id, message_id, &message)
                            .reply_markup(keyboard)
                            .await?;
                        return Ok(());
                    }

                    let selected_date = format!("{:02}.{:02}.{}", day, month, year);
                    if let Some(reminder) = draft.reminders.last_mut() {
                        reminder.date = Some(selected_date);
                    }

                    let message = t(language, "agreement.custom.add_reminder.amount_prompt");
                    let keyboard = InlineKeyboardMarkup::new(vec![
                        vec![InlineKeyboardButton::callback(
                            t(language, "common.skip_button"),
                            "custom:skip_amount",
                        )],
                        vec![InlineKeyboardButton::callback(
                            t(language, "common.cancel_button"),
                            "flow:cancel",
                        )],
                    ]);
                    update_state(pool, user_id, states::CUSTOM_REMINDER_AMOUNT, draft);
                    bot.edit_message_text(chat_id, message_id, &message)
                        .reply_markup(keyboard)
                        .await?;
                }
            }
        }
    }

    Ok(())
}

pub async fn handle_custom_title_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    let title = sanitize_input(text);

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

    if let Ok(Some(_)) = find_agreement_by_user_and_title(pool, user.id, &title) {
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
    let mut draft: CustomDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    draft.title = Some(title.to_string());

    let message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["2", "3"]),
        t(language, "agreement.custom.step2_description.prompt")
    );

    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.custom.step2_description.skip_button"),
            "custom:skip_description",
        )],
        vec![InlineKeyboardButton::callback(
            t(language, "common.cancel_button"),
            "flow:cancel",
        )],
    ]);

    update_state(pool, user_id, states::CUSTOM_DESCRIPTION, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

pub async fn handle_custom_description_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    let description = sanitize_input(text);

    if description.len() > 200 {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.description_too_long"),
            },
        )
        .await?;
        return Ok(());
    }

    let state = get_conversation_state(pool, user_id).ok().flatten();
    let mut draft: CustomDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    draft.description = if description.is_empty() {
        None
    } else {
        Some(description.to_string())
    };

    start_reminder_flow(pool, bot, chat_id, thread_id, user_id, language, &draft).await?;

    Ok(())
}

async fn start_reminder_flow(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    draft: &CustomDraft,
) -> ResponseResult<()> {
    let message = format!(
        "{}\n\n{}",
        t_with_args(language, "common.step_progress", &["3", "3"]),
        t(
            language,
            "agreement.custom.add_reminder.reminder_title_prompt"
        )
    );

    let keyboard = build_cancel_keyboard(language);
    update_state(pool, user_id, states::CUSTOM_REMINDER_TITLE, draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

pub async fn handle_custom_reminder_title_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    let title = sanitize_input(text);

    if title.is_empty() {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.reminder_title_required"),
            },
        )
        .await?;
        return Ok(());
    }

    if title.len() > 100 {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.reminder_title_too_long"),
            },
        )
        .await?;
        return Ok(());
    }

    let state = get_conversation_state(pool, user_id).ok().flatten();
    let mut draft: CustomDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    if draft.reminders.len() >= MAX_REMINDERS_PER_AGREEMENT {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.max_reminders_reached"),
            },
        )
        .await?;
        return Ok(());
    }

    let new_reminder = CustomReminderDraft {
        title: Some(title.to_string()),
        ..Default::default()
    };
    draft.reminders.push(new_reminder);

    let message = t(language, "agreement.custom.add_reminder.date_prompt");
    let keyboard = build_custom_calendar(language, Utc::now().date_naive());
    update_state(pool, user_id, states::CUSTOM_REMINDER_DATE, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

pub async fn handle_custom_reminder_amount_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
) -> ResponseResult<()> {
    let amount_str = text.trim().replace(',', ".").replace([' ', '_'], "");

    if amount_str.chars().filter(|&c| c == '.').count() > 1
        || amount_str.contains(['e', 'E'])
        || !amount_str.chars().all(|c| c.is_numeric() || c == '.')
    {
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

    const MAX_AMOUNT: f64 = 10_000_000.0;
    let amount = match amount_str.parse::<f64>() {
        Ok(a) if a > 0.0 && a <= MAX_AMOUNT => (a * 100.0).round() / 100.0,
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
    let mut draft: CustomDraft = state
        .as_ref()
        .and_then(|s| s.state_data.as_ref())
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    if let Some(reminder) = draft.reminders.last_mut() {
        reminder.amount = Some(format!("{:.2}", amount));
    }

    let message = t(language, "agreement.custom.add_reminder.currency_prompt");
    let keyboard = build_custom_currency_keyboard();
    update_state(pool, user_id, states::CUSTOM_REMINDER_AMOUNT, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

async fn show_reminder_list(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    draft: &CustomDraft,
) -> ResponseResult<()> {
    let mut message = format!(
        "{}\n\n{}",
        t(language, "agreement.custom.add_reminder.added"),
        t(language, "agreement.custom.add_reminder.list_title")
    );

    for (i, reminder) in draft.reminders.iter().enumerate() {
        let title = reminder.title.as_deref().unwrap_or("-");
        let date = reminder.date.as_deref().unwrap_or("-");
        let amount_display = match (&reminder.amount, &draft.currency) {
            (Some(a), Some(c)) => format!(" - {} {}", a, c),
            _ => String::new(),
        };
        message.push_str(&format!(
            "\n{}. {} ({}){}",
            i + 1,
            title,
            date,
            amount_display
        ));
    }

    let keyboard = build_reminder_list_keyboard(language);
    update_state(pool, user_id, states::CUSTOM_REMINDER_LIST, draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn show_custom_summary(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    _thread_id: Option<i32>,
    message_id: teloxide::types::MessageId,
    user_id: i64,
    language: &str,
    draft: &CustomDraft,
) -> ResponseResult<()> {
    let mut summary = format!(
        "{}\n\n{}\n",
        t(language, "agreement.custom.summary.title"),
        t_with_args(
            language,
            "agreement.custom.summary.agreement_name",
            &[draft.title.as_deref().unwrap_or("-")]
        )
    );

    if let Some(desc) = &draft.description {
        summary.push_str(&t_with_args(
            language,
            "agreement.custom.summary.description",
            &[desc],
        ));
        summary.push('\n');
    }

    summary.push_str(&format!(
        "\n{}\n",
        t_with_args(
            language,
            "agreement.custom.success.reminder_count",
            &[&draft.reminders.len().to_string()]
        )
    ));

    for (i, reminder) in draft.reminders.iter().enumerate() {
        let title = reminder.title.as_deref().unwrap_or("-");
        let date = reminder.date.as_deref().unwrap_or("-");
        let amount_display = match (&reminder.amount, &draft.currency) {
            (Some(a), Some(c)) => format!(" - {} {}", a, c),
            _ => String::new(),
        };
        summary.push_str(&format!(
            "{}. {} ({}){}\n",
            i + 1,
            title,
            date,
            amount_display
        ));
    }

    summary.push_str(&format!(
        "\n{}",
        t(language, "agreement.rent.step9_summary.confirm_prompt")
    ));

    let keyboard = build_confirm_keyboard(language, "custom");
    update_state(pool, user_id, states::CUSTOM_SUMMARY, draft);

    bot.edit_message_text(ChatId(chat_id), message_id, &summary)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn save_custom_agreement(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    _thread_id: Option<i32>,
    message_id: teloxide::types::MessageId,
    user_id: i64,
    language: &str,
    draft: &CustomDraft,
) -> ResponseResult<()> {
    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        _ => {
            bot.edit_message_text(
                ChatId(chat_id),
                message_id,
                t(language, "agreement.errors.user_not_found"),
            )
            .await?;
            return Ok(());
        }
    };

    let new_agreement = NewAgreement {
        user_id: user.id,
        agreement_type: "custom",
        title: draft.title.as_deref().unwrap_or("Custom Agreement"),
        user_role: None,
        start_date: None,
        currency: draft.currency.as_deref().unwrap_or("TRY"),
        rent_amount: None,
        due_day: None,
        has_monthly_reminder: false,
        reminder_timing: None,
        reminder_days_before: None,
        has_yearly_increase_reminder: false,
        description: draft.description.as_deref(),
        has_ten_year_reminder: false,
        has_five_year_reminder: false,
    };

    let agreement = match create_agreement(pool, new_agreement) {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("Failed to create agreement: {:?}", e);
            METRICS.increment_errors();
            bot.edit_message_text(
                ChatId(chat_id),
                message_id,
                t(language, "agreement.errors.database_error"),
            )
            .await?;
            return Ok(());
        }
    };

    let reminders: Vec<NewReminder> = draft
        .reminders
        .iter()
        .flat_map(|r| {
            let due_date = match r
                .date
                .as_ref()
                .and_then(|s| NaiveDate::parse_from_str(s, "%d.%m.%Y").ok())
            {
                Some(d) => d,
                None => return vec![],
            };

            let timing = r.timing.as_deref().unwrap_or("same_day");
            let days_before = match timing {
                "1_day_before" => 1,
                "3_days_before" => 3,
                "1_week_before" => 7,
                _ => 0,
            };

            let amount = r.amount.as_ref().and_then(|s| BigDecimal::from_str(s).ok());
            let title = r.title.clone().unwrap_or_else(|| "Reminder".to_string());

            let mut result = Vec::new();

            if days_before > 0 {
                let pre_reminder_date = due_date - chrono::Duration::days(days_before);
                result.push(NewReminder {
                    agreement_id: agreement.id,
                    reminder_type: "pre_notify".to_string(),
                    title: title.clone(),
                    amount: amount.clone(),
                    due_date,
                    reminder_date: pre_reminder_date,
                });
            }

            result.push(NewReminder {
                agreement_id: agreement.id,
                reminder_type: "due_day".to_string(),
                title,
                amount,
                due_date,
                reminder_date: due_date,
            });

            result
        })
        .collect();

    let reminder_count = reminders.len();
    if !reminders.is_empty() {
        if let Err(e) = create_reminders_batch(pool, reminders) {
            tracing::error!("Failed to create reminders: {:?}", e);
            METRICS.increment_errors();

            if let Err(e) = clear_conversation_state(pool, user_id) {
                tracing::warn!(
                    "Failed to clear conversation state for user {}: {:?}",
                    user_id,
                    e
                );
            }

            let error_message = t(language, "agreement.errors.reminders_failed");
            let keyboard = build_menu_keyboard(language);
            bot.edit_message_text(ChatId(chat_id), message_id, &error_message)
                .reply_markup(keyboard)
                .await?;
            return Ok(());
        }
    }

    if let Err(e) = clear_conversation_state(pool, user_id) {
        tracing::warn!(
            "Failed to clear conversation state for user {}: {:?}",
            user_id,
            e
        );
    }

    let success_message = format!(
        "{}\n\n{}",
        t(language, "agreement.custom.success.title"),
        t_with_args(
            language,
            "agreement.custom.success.reminder_count",
            &[&reminder_count.to_string()]
        )
    );

    let keyboard = build_menu_keyboard(language);
    bot.edit_message_text(ChatId(chat_id), message_id, &success_message)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}
