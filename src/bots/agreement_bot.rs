use crate::bots::bot_service::TelegramMessage;
use crate::observability::alerts::Severity;
use crate::observability::{ALERTS, METRICS};
use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate, Utc};
use notifine::db::DbPool;
use notifine::i18n::{t, t_with_args};
use notifine::models::{Agreement, NewAgreement, NewAgreementUser, NewReminder, UpdateAgreement};
use notifine::{
    accept_disclaimer, clear_conversation_state, create_agreement, create_agreement_user,
    create_reminders_batch, delete_agreement, delete_reminders_by_agreement_id,
    find_agreement_by_id, find_agreement_by_user_and_title, find_agreement_user_by_telegram_id,
    find_agreements_by_user_id, find_reminder_by_id, find_reminders_by_agreement_id,
    get_conversation_state, set_conversation_state, update_agreement,
    update_agreement_user_language, update_agreement_user_timezone, update_reminder_snooze,
    update_reminder_status,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::str::FromStr;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree;
use teloxide::macros::BotCommands;
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};

const DEFAULT_LANGUAGE: &str = "tr";
const DEFAULT_TIMEZONE: &str = "Europe/Istanbul";
const STATE_EXPIRY_MINUTES: i64 = 30;
const MAX_REMINDERS_PER_AGREEMENT: usize = 20;

fn sanitize_input(text: &str) -> String {
    text.trim()
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect::<String>()
        .trim()
        .to_string()
}

mod states {
    pub const RENT_SCOPE_CHECK: &str = "rent_scope_check";
    pub const RENT_TITLE: &str = "rent_title";
    pub const RENT_ROLE: &str = "rent_role";
    pub const RENT_START_DATE: &str = "rent_start_date";
    pub const RENT_AMOUNT: &str = "rent_amount";
    pub const RENT_CURRENCY: &str = "rent_currency";
    pub const RENT_DUE_DAY: &str = "rent_due_day";
    pub const RENT_MONTHLY_REMINDER: &str = "rent_monthly_reminder";
    pub const RENT_REMINDER_TIMING: &str = "rent_reminder_timing";
    pub const RENT_YEARLY_INCREASE: &str = "rent_yearly_increase";
    pub const RENT_SUMMARY: &str = "rent_summary";
    pub const CUSTOM_TITLE: &str = "custom_title";
    pub const CUSTOM_DESCRIPTION: &str = "custom_description";
    pub const CUSTOM_REMINDER_TITLE: &str = "custom_reminder_title";
    pub const CUSTOM_REMINDER_DATE: &str = "custom_reminder_date";
    pub const CUSTOM_REMINDER_AMOUNT: &str = "custom_reminder_amount";
    pub const CUSTOM_REMINDER_TIMING: &str = "custom_reminder_timing";
    pub const CUSTOM_REMINDER_LIST: &str = "custom_reminder_list";
    pub const CUSTOM_SUMMARY: &str = "custom_summary";
    pub const EDIT_TITLE: &str = "edit_title";
    pub const EDIT_AMOUNT: &str = "edit_amount";
    pub const EDIT_DUE_DAY: &str = "edit_due_day";
    pub const EDIT_DESCRIPTION: &str = "edit_description";
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RentDraft {
    title: Option<String>,
    user_role: Option<String>,
    start_date: Option<String>,
    currency: Option<String>,
    rent_amount: Option<String>,
    due_day: Option<i32>,
    has_monthly_reminder: Option<bool>,
    reminder_timing: Option<String>,
    has_yearly_increase_reminder: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CustomDraft {
    title: Option<String>,
    description: Option<String>,
    currency: Option<String>,
    reminders: Vec<CustomReminderDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CustomReminderDraft {
    title: Option<String>,
    date: Option<String>,
    amount: Option<String>,
    timing: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EditDraft {
    agreement_id: i32,
    field: Option<String>,
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Start the bot")]
    Start,
    #[command(description = "Show help information")]
    Help,
    #[command(description = "Cancel current operation")]
    Cancel,
    #[command(description = "View your settings")]
    Settings,
    #[command(description = "Change language")]
    Language,
    #[command(description = "Change timezone")]
    Timezone,
    #[command(description = "List all agreements")]
    List,
}

async fn command_handler(
    bot: Bot,
    msg: Message,
    command: Command,
    pool: DbPool,
) -> ResponseResult<()> {
    let user = match msg.from() {
        Some(u) => u,
        None => return Ok(()),
    };

    let user_id = user.id.0 as i64;
    let chat_id = msg.chat.id.0;
    let thread_id = msg.thread_id;

    match command {
        Command::Start => handle_start(&pool, &bot, &msg, user_id, chat_id, thread_id).await?,
        Command::Help => handle_help(&pool, &bot, user_id, chat_id, thread_id).await?,
        Command::Cancel => handle_cancel(&pool, &bot, user_id, chat_id, thread_id).await?,
        Command::Settings => handle_settings(&pool, &bot, user_id, chat_id, thread_id).await?,
        Command::Language => handle_language(&pool, &bot, user_id, chat_id, thread_id).await?,
        Command::Timezone => handle_timezone(&pool, &bot, user_id, chat_id, thread_id).await?,
        Command::List => handle_list_agreements(&pool, &bot, user_id, chat_id, thread_id).await?,
    };

    Ok(())
}

async fn handle_start(
    pool: &DbPool,
    bot: &Bot,
    msg: &Message,
    user_id: i64,
    chat_id: i64,
    thread_id: Option<i32>,
) -> ResponseResult<()> {
    let existing_user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("Database error finding user: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find agreement user: {}", e),
                )
                .await;
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(DEFAULT_LANGUAGE, "agreement.errors.database_error"),
            })
            .await?;
            return Ok(());
        }
    };

    match existing_user {
        Some(user) => {
            if user.disclaimer_accepted {
                let message = format!(
                    "{}\n\n{}",
                    t(&user.language, "agreement.welcome.title"),
                    t(&user.language, "agreement.menu.title")
                );
                let keyboard = build_menu_keyboard(&user.language);
                send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;
            } else {
                send_disclaimer(bot, chat_id, thread_id, &user.language).await?;
            }
        }
        None => {
            let language = detect_language_from_message(msg);
            let user_info = match msg.from() {
                Some(u) => u,
                None => return Ok(()),
            };

            let new_user = NewAgreementUser {
                telegram_user_id: user_id,
                telegram_chat_id: chat_id,
                username: user_info.username.as_deref(),
                first_name: Some(&user_info.first_name),
                last_name: user_info.last_name.as_deref(),
                language: &language,
                timezone: DEFAULT_TIMEZONE,
            };

            if let Err(e) = create_agreement_user(pool, new_user) {
                tracing::error!("Failed to create agreement user: {:?}", e);
                METRICS.increment_errors();
                ALERTS
                    .send_alert(
                        bot,
                        Severity::Error,
                        "Database",
                        &format!("Failed to create agreement user: {}", e),
                    )
                    .await;
                send_telegram_message(TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(&language, "agreement.errors.database_error"),
                })
                .await?;
                return Ok(());
            }

            let welcome_message = format!(
                "{}\n\n{}",
                t(&language, "agreement.welcome.title"),
                t(&language, "agreement.welcome.description")
            );
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: welcome_message,
            })
            .await?;

            send_disclaimer(bot, chat_id, thread_id, &language).await?;
        }
    }

    Ok(())
}

async fn send_disclaimer(
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    language: &str,
) -> ResponseResult<()> {
    let disclaimer_message = format!(
        "{}\n\n{}\n\n{}",
        t(language, "agreement.disclaimer.title"),
        t(language, "agreement.disclaimer.content"),
        t(language, "agreement.disclaimer.accept_prompt")
    );

    let keyboard = build_disclaimer_keyboard(language);
    send_message_with_keyboard(bot, chat_id, thread_id, &disclaimer_message, keyboard).await?;

    Ok(())
}

async fn handle_help(
    pool: &DbPool,
    _bot: &Bot,
    user_id: i64,
    chat_id: i64,
    thread_id: Option<i32>,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);

    let help_message = format!(
        "{}\n\n{}",
        t(&language, "agreement.help.title"),
        t(&language, "agreement.help.content")
    );

    send_telegram_message(TelegramMessage {
        chat_id,
        thread_id,
        message: help_message,
    })
    .await?;

    Ok(())
}

async fn handle_cancel(
    pool: &DbPool,
    bot: &Bot,
    user_id: i64,
    chat_id: i64,
    thread_id: Option<i32>,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);

    match clear_conversation_state(pool, user_id) {
        Ok(cleared) => {
            let message = if cleared {
                t(&language, "agreement.cancel.success")
            } else {
                t(&language, "agreement.cancel.nothing")
            };
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message,
            })
            .await?;
        }
        Err(e) => {
            tracing::error!("Failed to clear conversation state: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to clear conversation state: {}", e),
                )
                .await;
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(&language, "agreement.errors.database_error"),
            })
            .await?;
        }
    }

    Ok(())
}

async fn handle_list_agreements(
    pool: &DbPool,
    bot: &Bot,
    user_id: i64,
    chat_id: i64,
    thread_id: Option<i32>,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);

    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        Ok(None) => {
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(&language, "agreement.errors.must_accept_disclaimer"),
            })
            .await?;
            return Ok(());
        }
        Err(e) => {
            tracing::error!("Failed to find user: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find user: {}", e),
                )
                .await;
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(&language, "agreement.errors.database_error"),
            })
            .await?;
            return Ok(());
        }
    };

    let agreements = match find_agreements_by_user_id(pool, user.id) {
        Ok(agrs) => agrs,
        Err(e) => {
            tracing::error!("Failed to find agreements: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find agreements: {}", e),
                )
                .await;
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(&language, "agreement.errors.database_error"),
            })
            .await?;
            return Ok(());
        }
    };

    if agreements.is_empty() {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(&language, "agreement.list.empty"),
        })
        .await?;
        return Ok(());
    }

    let keyboard = build_agreements_list_keyboard(&language, &agreements);
    send_message_with_keyboard(
        bot,
        chat_id,
        thread_id,
        &t(&language, "agreement.list.title"),
        keyboard,
    )
    .await?;

    Ok(())
}

async fn handle_settings(
    pool: &DbPool,
    bot: &Bot,
    user_id: i64,
    chat_id: i64,
    thread_id: Option<i32>,
) -> ResponseResult<()> {
    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        Ok(None) => {
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(DEFAULT_LANGUAGE, "agreement.errors.user_not_found"),
            })
            .await?;
            return Ok(());
        }
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find user in settings: {}", e),
                )
                .await;
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(DEFAULT_LANGUAGE, "agreement.errors.database_error"),
            })
            .await?;
            return Ok(());
        }
    };

    if !user.disclaimer_accepted {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(&user.language, "agreement.errors.must_accept_disclaimer"),
        })
        .await?;
        return Ok(());
    }

    let language_display = if user.language == "tr" {
        "Türkçe"
    } else {
        "English"
    };

    let settings_message = format!(
        "{}\n\n{}: {}\n{}: {}",
        t(&user.language, "agreement.settings.title"),
        t(&user.language, "agreement.settings.language_label"),
        language_display,
        t(&user.language, "agreement.settings.timezone_label"),
        user.timezone
    );

    let keyboard = build_settings_keyboard(&user.language);
    send_message_with_keyboard(bot, chat_id, thread_id, &settings_message, keyboard).await?;

    Ok(())
}

async fn handle_language(
    pool: &DbPool,
    bot: &Bot,
    user_id: i64,
    chat_id: i64,
    thread_id: Option<i32>,
) -> ResponseResult<()> {
    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        Ok(None) => {
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(DEFAULT_LANGUAGE, "agreement.errors.user_not_found"),
            })
            .await?;
            return Ok(());
        }
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find user in language: {}", e),
                )
                .await;
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(DEFAULT_LANGUAGE, "agreement.errors.database_error"),
            })
            .await?;
            return Ok(());
        }
    };

    let language_display = if user.language == "tr" {
        "Türkçe"
    } else {
        "English"
    };

    let message = format!(
        "{}\n\n{}",
        t(&user.language, "agreement.language.title"),
        t_with_args(
            &user.language,
            "agreement.language.current",
            &[language_display]
        )
    );

    let keyboard = build_language_keyboard(&user.language);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

async fn handle_timezone(
    pool: &DbPool,
    bot: &Bot,
    user_id: i64,
    chat_id: i64,
    thread_id: Option<i32>,
) -> ResponseResult<()> {
    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        Ok(None) => {
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(DEFAULT_LANGUAGE, "agreement.errors.user_not_found"),
            })
            .await?;
            return Ok(());
        }
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            METRICS.increment_errors();
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "Database",
                    &format!("Failed to find user in timezone: {}", e),
                )
                .await;
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(DEFAULT_LANGUAGE, "agreement.errors.database_error"),
            })
            .await?;
            return Ok(());
        }
    };

    let message = format!(
        "{}\n\n{}\n\n{}",
        t(&user.language, "agreement.timezone.title"),
        t_with_args(
            &user.language,
            "agreement.timezone.current",
            &[&user.timezone]
        ),
        t(&user.language, "agreement.timezone.default_note")
    );

    let keyboard = build_timezone_keyboard();
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

async fn callback_handler(bot: Bot, q: CallbackQuery, pool: DbPool) -> ResponseResult<()> {
    let data = match &q.data {
        Some(d) => d.clone(),
        None => return Ok(()),
    };

    let user_id = q.from.id.0 as i64;

    if data == "disclaimer:accept" {
        handle_disclaimer_accept(&pool, &bot, &q, user_id).await?;
    } else if data == "disclaimer:decline" {
        handle_disclaimer_decline(&pool, &bot, &q, user_id).await?;
    } else if let Some(lang) = data.strip_prefix("lang:") {
        handle_language_select(&pool, &bot, &q, user_id, lang).await?;
    } else if let Some(tz) = data.strip_prefix("tz:") {
        handle_timezone_select(&pool, &bot, &q, user_id, tz).await?;
    } else if data == "menu:rent" || data == "menu:custom" {
        handle_menu_select(&pool, &bot, &q, user_id, &data).await?;
    } else if data == "flow:cancel" {
        handle_flow_cancel(&pool, &bot, &q, user_id).await?;
    } else if data.starts_with("rent:") {
        handle_rent_callback(&pool, &bot, &q, user_id, &data).await?;
    } else if data.starts_with("custom:") {
        handle_custom_callback(&pool, &bot, &q, user_id, &data).await?;
    } else if data.starts_with("agr:") {
        handle_agreement_callback(&pool, &bot, &q, user_id, &data).await?;
    } else if data.starts_with("rem:") {
        handle_reminder_callback(&pool, &bot, &q, user_id, &data).await?;
    } else if data == "settings:language" {
        let language = get_user_language(&pool, user_id);
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
    } else if data == "settings:timezone" {
        let language = get_user_language(&pool, user_id);
        let user = find_agreement_user_by_telegram_id(&pool, user_id)
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
    } else {
        let language = get_user_language(&pool, user_id);
        bot.answer_callback_query(&q.id)
            .text(t(&language, "agreement.errors.unknown_callback"))
            .await?;
    }

    Ok(())
}

async fn handle_flow_cancel(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    user_id: i64,
) -> ResponseResult<()> {
    let language = get_user_language(pool, user_id);

    let _ = clear_conversation_state(pool, user_id);
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

async fn handle_rent_callback(
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

    if let Some(scope_answer) = data.strip_prefix("rent:scope:") {
        if scope_answer == "no" {
            let _ = clear_conversation_state(pool, user_id);
            bot.edit_message_text(
                msg.chat.id,
                msg.id,
                t(&language, "agreement.rent.scope_check.commercial_notice"),
            )
            .await?;
            return Ok(());
        }
        let message = format!(
            "{}\n\n{}",
            t_with_args(&language, "common.step_progress", &["1", "9"]),
            t(&language, "agreement.rent.step1_title.prompt")
        );
        let keyboard = build_cancel_keyboard(&language);
        update_state(pool, user_id, states::RENT_TITLE, &draft);
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    } else if let Some(role) = data.strip_prefix("rent:role:") {
        draft.user_role = Some(role.to_string());
        let message = format!(
            "{}\n\n{}",
            t_with_args(&language, "common.step_progress", &["3", "9"]),
            t(&language, "agreement.rent.step3_start_date.prompt")
        );
        let keyboard = build_mini_calendar(&language, Utc::now().date_naive());
        update_state(pool, user_id, states::RENT_START_DATE, &draft);
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    } else if let Some(currency) = data.strip_prefix("rent:currency:") {
        if !["TRY", "EUR", "USD", "GBP"].contains(&currency) {
            return Ok(());
        }
        draft.currency = Some(currency.to_string());
        let message = format!(
            "{}\n\n{}",
            t_with_args(&language, "common.step_progress", &["5", "9"]),
            t(&language, "agreement.rent.step5_due_day.prompt")
        );
        let keyboard = build_due_day_keyboard();
        update_state(pool, user_id, states::RENT_DUE_DAY, &draft);
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    } else if let Some(day_str) = data.strip_prefix("rent:due_day:") {
        if let Ok(day) = day_str.parse::<i32>() {
            draft.due_day = Some(day);
            let mut message = format!(
                "{}\n\n{}",
                t_with_args(&language, "common.step_progress", &["6", "9"]),
                t(&language, "agreement.rent.step6_monthly_reminder.prompt")
            );
            if day >= 29 {
                message.push_str("\n\n");
                message.push_str(&t(&language, "agreement.rent.step5_due_day.late_day_note"));
            }
            let keyboard = build_yes_no_keyboard(&language, "rent:monthly");
            update_state(pool, user_id, states::RENT_MONTHLY_REMINDER, &draft);
            bot.edit_message_text(msg.chat.id, msg.id, &message)
                .reply_markup(keyboard)
                .await?;
        }
    } else if let Some(answer) = data.strip_prefix("rent:monthly:") {
        draft.has_monthly_reminder = Some(answer == "yes");
        if answer == "yes" {
            let message = format!(
                "{}\n\n{}",
                t_with_args(&language, "common.step_progress", &["7", "9"]),
                t(&language, "agreement.rent.step7_reminder_timing.prompt")
            );
            let keyboard = build_reminder_timing_keyboard(&language);
            update_state(pool, user_id, states::RENT_REMINDER_TIMING, &draft);
            bot.edit_message_text(msg.chat.id, msg.id, &message)
                .reply_markup(keyboard)
                .await?;
        } else {
            let message = format!(
                "{}\n\n{}",
                t_with_args(&language, "common.step_progress", &["8", "9"]),
                t(&language, "agreement.rent.step8_yearly_increase.prompt")
            );
            let keyboard = build_yes_no_keyboard(&language, "rent:yearly");
            update_state(pool, user_id, states::RENT_YEARLY_INCREASE, &draft);
            bot.edit_message_text(msg.chat.id, msg.id, &message)
                .reply_markup(keyboard)
                .await?;
        }
    } else if let Some(timing) = data.strip_prefix("rent:timing:") {
        draft.reminder_timing = Some(timing.to_string());
        let message = format!(
            "{}\n\n{}",
            t_with_args(&language, "common.step_progress", &["8", "9"]),
            t(&language, "agreement.rent.step8_yearly_increase.prompt")
        );
        let keyboard = build_yes_no_keyboard(&language, "rent:yearly");
        update_state(pool, user_id, states::RENT_YEARLY_INCREASE, &draft);
        bot.edit_message_text(msg.chat.id, msg.id, &message)
            .reply_markup(keyboard)
            .await?;
    } else if let Some(answer) = data.strip_prefix("rent:yearly:") {
        draft.has_yearly_increase_reminder = Some(answer == "yes");
        show_rent_summary(
            pool, bot, chat_id, thread_id, msg.id, user_id, &language, &draft,
        )
        .await?;
    } else if data == "rent:confirm" {
        save_rent_agreement(
            pool, bot, chat_id, thread_id, msg.id, user_id, &language, &draft,
        )
        .await?;
    } else if data.starts_with("rent:cal:") {
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

fn update_custom_state(pool: &DbPool, user_id: i64, new_state: &str, draft: &CustomDraft) {
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

#[allow(clippy::too_many_arguments)]
async fn show_rent_summary(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    _thread_id: Option<i32>,
    message_id: teloxide::types::MessageId,
    user_id: i64,
    language: &str,
    draft: &RentDraft,
) -> ResponseResult<()> {
    let role_display = match draft.user_role.as_deref() {
        Some("tenant") => t(language, "agreement.rent.step9_summary.role_tenant"),
        Some("landlord") => t(language, "agreement.rent.step9_summary.role_landlord"),
        _ => "-".to_string(),
    };

    let monthly_status = if draft.has_monthly_reminder.unwrap_or(false) {
        t(language, "agreement.rent.step9_summary.enabled")
    } else {
        t(language, "agreement.rent.step9_summary.disabled")
    };

    let yearly_status = if draft.has_yearly_increase_reminder.unwrap_or(false) {
        t(language, "agreement.rent.step9_summary.enabled")
    } else {
        t(language, "agreement.rent.step9_summary.disabled")
    };

    let summary = format!(
        "{}\n\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n\n{}",
        t(language, "agreement.rent.step9_summary.title"),
        t_with_args(
            language,
            "agreement.rent.step9_summary.agreement_name",
            &[draft.title.as_deref().unwrap_or("-")]
        ),
        t_with_args(
            language,
            "agreement.rent.step9_summary.role",
            &[&role_display]
        ),
        t_with_args(
            language,
            "agreement.rent.step9_summary.start_date",
            &[draft.start_date.as_deref().unwrap_or("-")]
        ),
        t_with_args(
            language,
            "agreement.rent.step9_summary.amount",
            &[
                draft.rent_amount.as_deref().unwrap_or("-"),
                draft.currency.as_deref().unwrap_or("TRY")
            ]
        ),
        t_with_args(
            language,
            "agreement.rent.step9_summary.due_day",
            &[&draft
                .due_day
                .map(|d| d.to_string())
                .unwrap_or("-".to_string())]
        ),
        t_with_args(
            language,
            "agreement.rent.step9_summary.monthly_reminder",
            &[&monthly_status]
        ),
        t_with_args(
            language,
            "agreement.rent.step9_summary.yearly_increase",
            &[&yearly_status]
        ),
        t(language, "agreement.rent.step9_summary.confirm_prompt")
    );

    let keyboard = build_confirm_keyboard(language, "rent");
    update_state(pool, user_id, states::RENT_SUMMARY, draft);

    bot.edit_message_text(ChatId(chat_id), message_id, &summary)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn save_rent_agreement(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    _thread_id: Option<i32>,
    message_id: teloxide::types::MessageId,
    user_id: i64,
    language: &str,
    draft: &RentDraft,
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

    let start_date = draft
        .start_date
        .as_ref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%d.%m.%Y").ok());

    let rent_amount = draft
        .rent_amount
        .as_ref()
        .and_then(|s| BigDecimal::from_str(s).ok());

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

    if draft.has_monthly_reminder.unwrap_or(false) {
        let _ = generate_monthly_reminders(pool, &agreement, draft, start_date, language);
    }

    let _ = clear_conversation_state(pool, user_id);

    let role_display = match draft.user_role.as_deref() {
        Some("tenant") => t(language, "agreement.rent.step9_summary.role_tenant"),
        Some("landlord") => t(language, "agreement.rent.step9_summary.role_landlord"),
        _ => "-".to_string(),
    };

    let start_year = start_date.map(|d| d.year()).unwrap_or(2025);
    let end_year = start_year + 11;

    let success_message = format!(
        "{}\n\n{}",
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
        )
    );

    let keyboard = build_menu_keyboard(language);
    bot.edit_message_text(ChatId(chat_id), message_id, &success_message)
        .reply_markup(keyboard)
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

            let reminder_date = due_date - chrono::Duration::days(days_before);

            let title = if draft.user_role.as_deref() == Some("tenant") {
                t(language, "agreement.rent.success.payment_title")
            } else {
                t(language, "agreement.rent.success.collection_title")
            };

            reminders.push(NewReminder {
                agreement_id: agreement.id,
                reminder_type: if days_before > 0 {
                    "pre_notify".to_string()
                } else {
                    "due_day".to_string()
                },
                title,
                amount: agreement.rent_amount.clone(),
                due_date,
                reminder_date,
            });
        }
    }

    if !reminders.is_empty() {
        create_reminders_batch(pool, reminders)?;
    }

    Ok(())
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn build_mini_calendar(language: &str, current: NaiveDate) -> InlineKeyboardMarkup {
    let year = current.year();
    let month = current.month();

    let month_names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let header = format!("{} {}", month_names[(month - 1) as usize], year);

    let mut rows = vec![vec![
        InlineKeyboardButton::callback(
            t(language, "agreement.calendar.prev_month"),
            format!("rent:cal:prev:{}:{}", year, month),
        ),
        InlineKeyboardButton::callback(header, "rent:cal:noop"),
        InlineKeyboardButton::callback(
            t(language, "agreement.calendar.next_month"),
            format!("rent:cal:next:{}:{}", year, month),
        ),
    ]];

    let day_headers = vec!["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
    rows.push(
        day_headers
            .into_iter()
            .map(|d| InlineKeyboardButton::callback(d.to_string(), "rent:cal:noop"))
            .collect(),
    );

    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let days_in_month = days_in_month(year, month);
    let first_weekday = first_day.weekday().num_days_from_monday() as usize;

    let mut day = 1u32;
    for _week in 0..6 {
        if day > days_in_month {
            break;
        }
        let mut row = Vec::new();
        for weekday in 0..7 {
            if (_week == 0 && weekday < first_weekday) || day > days_in_month {
                row.push(InlineKeyboardButton::callback(
                    " ".to_string(),
                    "rent:cal:noop",
                ));
            } else {
                row.push(InlineKeyboardButton::callback(
                    day.to_string(),
                    format!("rent:cal:day:{}:{}:{}", year, month, day),
                ));
                day += 1;
            }
        }
        rows.push(row);
    }

    rows.push(vec![InlineKeyboardButton::callback(
        t(language, "common.cancel_button"),
        "flow:cancel",
    )]);

    InlineKeyboardMarkup::new(rows)
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
    draft: &mut RentDraft,
) -> ResponseResult<()> {
    if data == "rent:cal:noop" {
        return Ok(());
    }

    if let Some(rest) = data.strip_prefix("rent:cal:prev:") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(year), Ok(month)) = (parts[0].parse::<i32>(), parts[1].parse::<u32>()) {
                let (new_year, new_month) = if month == 1 {
                    (year - 1, 12)
                } else {
                    (year, month - 1)
                };
                let new_date = NaiveDate::from_ymd_opt(new_year, new_month, 1).unwrap();
                let keyboard = build_mini_calendar(language, new_date);
                let message = format!(
                    "{}\n\n{}",
                    t_with_args(language, "common.step_progress", &["3", "9"]),
                    t(language, "agreement.rent.step3_start_date.prompt")
                );
                bot.edit_message_text(chat_id, message_id, &message)
                    .reply_markup(keyboard)
                    .await?;
            }
        }
    } else if let Some(rest) = data.strip_prefix("rent:cal:next:") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(year), Ok(month)) = (parts[0].parse::<i32>(), parts[1].parse::<u32>()) {
                let (new_year, new_month) = if month == 12 {
                    (year + 1, 1)
                } else {
                    (year, month + 1)
                };
                let new_date = NaiveDate::from_ymd_opt(new_year, new_month, 1).unwrap();
                let keyboard = build_mini_calendar(language, new_date);
                let message = format!(
                    "{}\n\n{}",
                    t_with_args(language, "common.step_progress", &["3", "9"]),
                    t(language, "agreement.rent.step3_start_date.prompt")
                );
                bot.edit_message_text(chat_id, message_id, &message)
                    .reply_markup(keyboard)
                    .await?;
            }
        }
    } else if let Some(rest) = data.strip_prefix("rent:cal:day:") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() == 3 {
            if let (Ok(year), Ok(month), Ok(day)) = (
                parts[0].parse::<i32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                let selected_date = format!("{:02}.{:02}.{}", day, month, year);
                draft.start_date = Some(selected_date);

                let message = format!(
                    "{}\n\n{}",
                    t_with_args(language, "common.step_progress", &["4", "9"]),
                    t(language, "agreement.rent.step4_amount.prompt")
                );
                let keyboard = build_currency_keyboard();
                update_state(pool, user_id, states::RENT_AMOUNT, draft);
                bot.edit_message_text(chat_id, message_id, &message)
                    .reply_markup(keyboard)
                    .await?;
            }
        }
    }

    Ok(())
}

async fn handle_custom_callback(
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
        update_custom_state(pool, user_id, states::CUSTOM_REMINDER_TIMING, &draft);
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
        update_custom_state(pool, user_id, states::CUSTOM_REMINDER_TIMING, &draft);
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
        update_custom_state(pool, user_id, states::CUSTOM_REMINDER_TITLE, &draft);
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
        handle_custom_calendar_callback(
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
async fn handle_custom_calendar_callback(
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
                    update_custom_state(pool, user_id, states::CUSTOM_REMINDER_AMOUNT, draft);
                    bot.edit_message_text(chat_id, message_id, &message)
                        .reply_markup(keyboard)
                        .await?;
                }
            }
        }
    }

    Ok(())
}

async fn handle_disclaimer_accept(
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

async fn handle_disclaimer_decline(
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

async fn handle_language_select(
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

async fn handle_timezone_select(
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

async fn handle_menu_select(
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
                start_rent_flow(pool, bot, msg.chat.id.0, msg.thread_id, user_id, &language)
                    .await?;
            }
            "menu:custom" => {
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
        t(language, "agreement.rent.scope_check.title"),
        t(language, "agreement.rent.scope_check.question")
    );

    let keyboard = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            t(language, "agreement.rent.scope_check.yes_button"),
            "rent:scope:yes",
        ),
        InlineKeyboardButton::callback(
            t(language, "agreement.rent.scope_check.no_button"),
            "rent:scope:no",
        ),
    ]]);

    let expires_at = Utc::now() + chrono::Duration::minutes(STATE_EXPIRY_MINUTES);
    let draft = RentDraft::default();

    if let Err(e) = set_conversation_state(
        pool,
        user_id,
        states::RENT_SCOPE_CHECK,
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

    let keyboard = build_cancel_keyboard(language);

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

async fn handle_agreement_callback(
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
    let upcoming: Vec<_> = reminders
        .iter()
        .filter(|r| r.reminder_date >= today && r.status == "pending")
        .take(5)
        .collect();

    if upcoming.is_empty() {
        parts.push(t(language, "agreement.view.reminders_empty"));
    } else {
        for reminder in upcoming {
            let amount_str = reminder
                .amount
                .as_ref()
                .map(|a| format!("({} {})", a, agreement.currency))
                .unwrap_or_default();
            parts.push(format!(
                "• {} - {}{}",
                reminder.reminder_date.format("%d.%m.%Y"),
                reminder.title,
                if amount_str.is_empty() {
                    String::new()
                } else {
                    format!(" {}", amount_str)
                }
            ));
        }
    }

    parts.join("\n")
}

fn build_agreement_detail_keyboard(
    language: &str,
    agreement_id: i32,
    agreement_type: &str,
) -> InlineKeyboardMarkup {
    let edit_text = t(language, "agreement.view.edit_button");
    let delete_text = t(language, "agreement.view.delete_button");
    let back_text = t(language, "agreement.view.back_button");

    let mut rows = vec![vec![
        InlineKeyboardButton::callback(edit_text, format!("agr:edit:{}", agreement_id)),
        InlineKeyboardButton::callback(delete_text, format!("agr:delete:{}", agreement_id)),
    ]];

    rows.push(vec![InlineKeyboardButton::callback(
        back_text,
        "agr:back:list",
    )]);

    let _ = agreement_type;

    InlineKeyboardMarkup::new(rows)
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

fn build_delete_confirm_keyboard(language: &str, agreement_id: i32) -> InlineKeyboardMarkup {
    let confirm_text = t(language, "agreement.delete.confirm_button");
    let cancel_text = t(language, "agreement.delete.cancel_button");

    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            confirm_text,
            format!("agr:delete:confirm:{}", agreement_id),
        ),
        InlineKeyboardButton::callback(cancel_text, "agr:back:list"),
    ]])
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

async fn handle_edit_callback(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    telegram_user_id: i64,
    internal_user_id: i32,
    rest: &str,
    language: &str,
) -> ResponseResult<()> {
    let parts: Vec<&str> = rest.split(':').collect();

    if parts.is_empty() {
        bot.answer_callback_query(&q.id)
            .text(t(language, "agreement.errors.unknown_callback"))
            .await?;
        return Ok(());
    }

    let agreement_id = match parts[0].parse::<i32>() {
        Ok(id) => id,
        Err(_) => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.errors.unknown_callback"))
                .await?;
            return Ok(());
        }
    };

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

    if parts.len() == 1 {
        show_edit_menu(bot, q, agreement_id, &agreement.agreement_type, language).await?;
    } else {
        let field = parts[1];
        handle_edit_field(
            pool,
            bot,
            q,
            telegram_user_id,
            agreement_id,
            &agreement,
            field,
            language,
        )
        .await?;
    }

    Ok(())
}

async fn show_edit_menu(
    bot: &Bot,
    q: &CallbackQuery,
    agreement_id: i32,
    agreement_type: &str,
    language: &str,
) -> ResponseResult<()> {
    let keyboard = build_edit_menu_keyboard(language, agreement_id, agreement_type);

    if let Some(msg) = &q.message {
        bot.edit_message_text(
            msg.chat.id,
            msg.id,
            format!(
                "{}\n\n{}",
                t(language, "agreement.edit.title"),
                t(language, "agreement.edit.select_field")
            ),
        )
        .reply_markup(keyboard)
        .await?;
    }

    bot.answer_callback_query(&q.id).await?;
    Ok(())
}

fn build_edit_menu_keyboard(
    language: &str,
    agreement_id: i32,
    agreement_type: &str,
) -> InlineKeyboardMarkup {
    let mut rows = vec![];

    rows.push(vec![InlineKeyboardButton::callback(
        t(language, "agreement.edit.title_button"),
        format!("agr:edit:{}:title", agreement_id),
    )]);

    if agreement_type == "rent" {
        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.amount_button"),
            format!("agr:edit:{}:amount", agreement_id),
        )]);

        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.due_day_button"),
            format!("agr:edit:{}:due_day", agreement_id),
        )]);

        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.monthly_reminder_button"),
            format!("agr:edit:{}:monthly", agreement_id),
        )]);

        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.reminder_timing_button"),
            format!("agr:edit:{}:timing", agreement_id),
        )]);

        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.yearly_increase_button"),
            format!("agr:edit:{}:yearly", agreement_id),
        )]);
    } else {
        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.description_button"),
            format!("agr:edit:{}:description", agreement_id),
        )]);
    }

    rows.push(vec![InlineKeyboardButton::callback(
        t(language, "agreement.edit.back_button"),
        format!("agr:view:{}", agreement_id),
    )]);

    InlineKeyboardMarkup::new(rows)
}

#[allow(clippy::too_many_arguments)]
async fn handle_edit_field(
    pool: &DbPool,
    bot: &Bot,
    q: &CallbackQuery,
    telegram_user_id: i64,
    agreement_id: i32,
    agreement: &Agreement,
    field: &str,
    language: &str,
) -> ResponseResult<()> {
    match field {
        "monthly" => {
            let new_value = !agreement.has_monthly_reminder;
            let updates = UpdateAgreement {
                has_monthly_reminder: Some(new_value),
                ..Default::default()
            };

            match update_agreement(pool, agreement_id, agreement.user_id, updates) {
                Ok(_) => {
                    let field_name = t(language, "agreement.view.field_monthly_reminder");
                    let message = if new_value {
                        t_with_args(language, "agreement.edit.toggle_on", &[&field_name])
                    } else {
                        t_with_args(language, "agreement.edit.toggle_off", &[&field_name])
                    };
                    bot.answer_callback_query(&q.id).text(&message).await?;

                    show_edit_menu(bot, q, agreement_id, &agreement.agreement_type, language)
                        .await?;
                }
                Err(e) => {
                    tracing::error!("Failed to update agreement: {:?}", e);
                    METRICS.increment_errors();
                    bot.answer_callback_query(&q.id)
                        .text(t(language, "agreement.errors.database_error"))
                        .await?;
                }
            }
        }
        "yearly" => {
            let new_value = !agreement.has_yearly_increase_reminder;
            let updates = UpdateAgreement {
                has_yearly_increase_reminder: Some(new_value),
                ..Default::default()
            };

            match update_agreement(pool, agreement_id, agreement.user_id, updates) {
                Ok(_) => {
                    let field_name = t(language, "agreement.view.field_yearly_increase");
                    let message = if new_value {
                        t_with_args(language, "agreement.edit.toggle_on", &[&field_name])
                    } else {
                        t_with_args(language, "agreement.edit.toggle_off", &[&field_name])
                    };
                    bot.answer_callback_query(&q.id).text(&message).await?;

                    show_edit_menu(bot, q, agreement_id, &agreement.agreement_type, language)
                        .await?;
                }
                Err(e) => {
                    tracing::error!("Failed to update agreement: {:?}", e);
                    METRICS.increment_errors();
                    bot.answer_callback_query(&q.id)
                        .text(t(language, "agreement.errors.database_error"))
                        .await?;
                }
            }
        }
        "timing" => {
            let keyboard = build_edit_timing_keyboard(language, agreement_id);
            if let Some(msg) = &q.message {
                bot.edit_message_text(
                    msg.chat.id,
                    msg.id,
                    t(language, "agreement.edit.timing_prompt"),
                )
                .reply_markup(keyboard)
                .await?;
            }
            bot.answer_callback_query(&q.id).await?;
        }
        "timing_before" => {
            let updates = UpdateAgreement {
                reminder_timing: Some(Some("before".to_string())),
                reminder_days_before: Some(Some(3)),
                ..Default::default()
            };

            match update_agreement(pool, agreement_id, agreement.user_id, updates) {
                Ok(_) => {
                    let field_name = t(language, "agreement.view.field_monthly_reminder");
                    bot.answer_callback_query(&q.id)
                        .text(t_with_args(
                            language,
                            "agreement.edit.success",
                            &[&field_name],
                        ))
                        .await?;
                    show_edit_menu(bot, q, agreement_id, &agreement.agreement_type, language)
                        .await?;
                }
                Err(e) => {
                    tracing::error!("Failed to update agreement: {:?}", e);
                    METRICS.increment_errors();
                    bot.answer_callback_query(&q.id)
                        .text(t(language, "agreement.errors.database_error"))
                        .await?;
                }
            }
        }
        "timing_on_day" => {
            let updates = UpdateAgreement {
                reminder_timing: Some(Some("on_day".to_string())),
                reminder_days_before: Some(Some(0)),
                ..Default::default()
            };

            match update_agreement(pool, agreement_id, agreement.user_id, updates) {
                Ok(_) => {
                    let field_name = t(language, "agreement.view.field_monthly_reminder");
                    bot.answer_callback_query(&q.id)
                        .text(t_with_args(
                            language,
                            "agreement.edit.success",
                            &[&field_name],
                        ))
                        .await?;
                    show_edit_menu(bot, q, agreement_id, &agreement.agreement_type, language)
                        .await?;
                }
                Err(e) => {
                    tracing::error!("Failed to update agreement: {:?}", e);
                    METRICS.increment_errors();
                    bot.answer_callback_query(&q.id)
                        .text(t(language, "agreement.errors.database_error"))
                        .await?;
                }
            }
        }
        "title" | "amount" | "due_day" | "description" => {
            let expires_at = Utc::now() + chrono::Duration::minutes(STATE_EXPIRY_MINUTES);
            let edit_draft = EditDraft {
                agreement_id,
                field: Some(field.to_string()),
            };

            let state_name = match field {
                "title" => states::EDIT_TITLE,
                "amount" => states::EDIT_AMOUNT,
                "due_day" => states::EDIT_DUE_DAY,
                "description" => states::EDIT_DESCRIPTION,
                _ => unreachable!(),
            };

            if let Err(e) = set_conversation_state(
                pool,
                telegram_user_id,
                state_name,
                Some(serde_json::to_value(&edit_draft).unwrap_or_default()),
                expires_at,
            ) {
                tracing::error!("Failed to set conversation state: {:?}", e);
                METRICS.increment_errors();
                bot.answer_callback_query(&q.id)
                    .text(t(language, "agreement.errors.database_error"))
                    .await?;
                return Ok(());
            }

            let prompt = match field {
                "title" => t(language, "agreement.edit.title_prompt"),
                "amount" => t(language, "agreement.edit.amount_prompt"),
                "due_day" => t(language, "agreement.edit.due_day_prompt"),
                "description" => t(language, "agreement.edit.description_prompt"),
                _ => unreachable!(),
            };

            if field == "due_day" {
                let keyboard = build_edit_due_day_keyboard(agreement_id);
                if let Some(msg) = &q.message {
                    bot.edit_message_text(msg.chat.id, msg.id, &prompt)
                        .reply_markup(keyboard)
                        .await?;
                }
            } else {
                let keyboard = build_cancel_keyboard(language);
                if let Some(msg) = &q.message {
                    bot.edit_message_text(msg.chat.id, msg.id, &prompt)
                        .reply_markup(keyboard)
                        .await?;
                }
            }

            bot.answer_callback_query(&q.id).await?;
        }
        _ if field.starts_with("due_day_") => {
            if let Some(day_str) = field.strip_prefix("due_day_") {
                if let Ok(day) = day_str.parse::<i32>() {
                    let updates = UpdateAgreement {
                        due_day: Some(Some(day)),
                        ..Default::default()
                    };

                    match update_agreement(pool, agreement_id, agreement.user_id, updates) {
                        Ok(_) => {
                            let _ = clear_conversation_state(pool, telegram_user_id);
                            let field_name = t(language, "agreement.view.field_due_day");
                            bot.answer_callback_query(&q.id)
                                .text(t_with_args(
                                    language,
                                    "agreement.edit.success",
                                    &[&field_name],
                                ))
                                .await?;
                            show_edit_menu(
                                bot,
                                q,
                                agreement_id,
                                &agreement.agreement_type,
                                language,
                            )
                            .await?;
                        }
                        Err(e) => {
                            tracing::error!("Failed to update agreement: {:?}", e);
                            METRICS.increment_errors();
                            bot.answer_callback_query(&q.id)
                                .text(t(language, "agreement.errors.database_error"))
                                .await?;
                        }
                    }
                }
            }
        }
        _ => {
            bot.answer_callback_query(&q.id)
                .text(t(language, "agreement.errors.unknown_callback"))
                .await?;
        }
    }

    Ok(())
}

fn build_edit_timing_keyboard(language: &str, agreement_id: i32) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.timing_before"),
            format!("agr:edit:{}:timing_before", agreement_id),
        )],
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.timing_on_day"),
            format!("agr:edit:{}:timing_on_day", agreement_id),
        )],
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.back_button"),
            format!("agr:edit:{}", agreement_id),
        )],
    ])
}

fn build_edit_due_day_keyboard(agreement_id: i32) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    for row_start in (1..=28).step_by(7) {
        let row: Vec<InlineKeyboardButton> = (row_start..row_start + 7)
            .filter(|&d| d <= 28)
            .map(|d| {
                InlineKeyboardButton::callback(
                    d.to_string(),
                    format!("agr:edit:{}:due_day_{}", agreement_id, d),
                )
            })
            .collect();
        rows.push(row);
    }

    InlineKeyboardMarkup::new(rows)
}

fn build_agreements_list_keyboard(
    language: &str,
    agreements: &[Agreement],
) -> InlineKeyboardMarkup {
    let view_text = t(language, "agreement.list.view_button");
    let delete_text = t(language, "agreement.list.delete_button");

    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    for agreement in agreements {
        let icon = if agreement.agreement_type == "rent" {
            "🏠"
        } else {
            "📝"
        };
        let title_row = vec![InlineKeyboardButton::callback(
            format!("{} {}", icon, &agreement.title),
            format!("agr:view:{}", agreement.id),
        )];
        rows.push(title_row);

        let action_row = vec![
            InlineKeyboardButton::callback(view_text.clone(), format!("agr:view:{}", agreement.id)),
            InlineKeyboardButton::callback(
                delete_text.clone(),
                format!("agr:delete:{}", agreement.id),
            ),
        ];
        rows.push(action_row);
    }

    InlineKeyboardMarkup::new(rows)
}

fn build_disclaimer_keyboard(language: &str) -> InlineKeyboardMarkup {
    let accept_text = t(language, "agreement.disclaimer.accept_button");
    let decline_text = t(language, "agreement.disclaimer.decline_button");

    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(accept_text, "disclaimer:accept"),
        InlineKeyboardButton::callback(decline_text, "disclaimer:decline"),
    ]])
}

fn build_menu_keyboard(language: &str) -> InlineKeyboardMarkup {
    let rent_text = t(language, "agreement.menu.rent_button");
    let custom_text = t(language, "agreement.menu.custom_button");

    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(rent_text, "menu:rent"),
        InlineKeyboardButton::callback(custom_text, "menu:custom"),
    ]])
}

fn build_language_keyboard(language: &str) -> InlineKeyboardMarkup {
    let en_text = t(language, "agreement.language.en_button");
    let tr_text = t(language, "agreement.language.tr_button");

    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(en_text, "lang:en"),
        InlineKeyboardButton::callback(tr_text, "lang:tr"),
    ]])
}

fn build_timezone_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "Europe/Istanbul",
            "tz:Europe/Istanbul",
        )],
        vec![InlineKeyboardButton::callback(
            "Europe/London",
            "tz:Europe/London",
        )],
        vec![InlineKeyboardButton::callback(
            "America/New_York",
            "tz:America/New_York",
        )],
    ])
}

fn build_settings_keyboard(language: &str) -> InlineKeyboardMarkup {
    let language_text = t(language, "agreement.settings.change_language");
    let timezone_text = t(language, "agreement.settings.change_timezone");

    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            language_text,
            "settings:language",
        )],
        vec![InlineKeyboardButton::callback(
            timezone_text,
            "settings:timezone",
        )],
    ])
}

fn build_cancel_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        t(language, "common.cancel_button"),
        "flow:cancel",
    )]])
}

fn build_yes_no_keyboard(language: &str, callback_prefix: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            t(language, "common.yes_button"),
            format!("{}:yes", callback_prefix),
        ),
        InlineKeyboardButton::callback(
            t(language, "common.no_button"),
            format!("{}:no", callback_prefix),
        ),
    ]])
}

#[allow(dead_code)]
fn build_role_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            t(language, "agreement.rent.step2_role.tenant_button"),
            "rent:role:tenant",
        ),
        InlineKeyboardButton::callback(
            t(language, "agreement.rent.step2_role.landlord_button"),
            "rent:role:landlord",
        ),
    ]])
}

fn build_currency_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🇹🇷 TRY", "rent:currency:TRY"),
            InlineKeyboardButton::callback("🇪🇺 EUR", "rent:currency:EUR"),
        ],
        vec![
            InlineKeyboardButton::callback("🇺🇸 USD", "rent:currency:USD"),
            InlineKeyboardButton::callback("🇬🇧 GBP", "rent:currency:GBP"),
        ],
    ])
}

fn build_due_day_keyboard() -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for row_start in (1..=31).step_by(7) {
        let row: Vec<InlineKeyboardButton> = (row_start..=(row_start + 6).min(31))
            .map(|day| {
                InlineKeyboardButton::callback(day.to_string(), format!("rent:due_day:{}", day))
            })
            .collect();
        rows.push(row);
    }
    InlineKeyboardMarkup::new(rows)
}

fn build_reminder_timing_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.step7_reminder_timing.same_day"),
                "rent:timing:same_day",
            ),
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.1_day_before",
                ),
                "rent:timing:1_day_before",
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.3_days_before",
                ),
                "rent:timing:3_days_before",
            ),
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.1_week_before",
                ),
                "rent:timing:1_week_before",
            ),
        ],
    ])
}

fn build_confirm_keyboard(language: &str, callback_prefix: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            t(language, "common.confirm_button"),
            format!("{}:confirm", callback_prefix),
        )],
        vec![InlineKeyboardButton::callback(
            t(language, "common.cancel_button"),
            "flow:cancel",
        )],
    ])
}

fn get_user_language(pool: &DbPool, user_id: i64) -> String {
    match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(user)) => user.language,
        _ => DEFAULT_LANGUAGE.to_string(),
    }
}

fn detect_language_from_message(msg: &Message) -> String {
    if let Some(user) = msg.from() {
        if let Some(lang_code) = &user.language_code {
            match lang_code.as_str() {
                "tr" | "tr-TR" => return "tr".to_string(),
                "en" | "en-US" | "en-GB" => return "en".to_string(),
                _ => {}
            }
        }
    }
    DEFAULT_LANGUAGE.to_string()
}

async fn send_telegram_message(message: TelegramMessage) -> ResponseResult<()> {
    let TelegramMessage {
        chat_id,
        thread_id,
        message,
    } = message;

    let token = env::var("AGREEMENT_BOT_TOKEN").expect("AGREEMENT_BOT_TOKEN must be set");

    let bot = Bot::new(token);
    let chat_id = ChatId(chat_id);

    let mut request = bot
        .send_message(chat_id, &message)
        .parse_mode(ParseMode::Html);

    if let Some(tid) = thread_id {
        request = request.message_thread_id(tid);
    }

    request.await?;

    Ok(())
}

async fn send_message_with_keyboard(
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    message: &str,
    keyboard: InlineKeyboardMarkup,
) -> ResponseResult<()> {
    let mut request = bot
        .send_message(ChatId(chat_id), message)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard);

    if let Some(tid) = thread_id {
        request = request.message_thread_id(tid);
    }

    request.await?;

    Ok(())
}

async fn message_handler(bot: Bot, msg: Message, pool: DbPool) -> ResponseResult<()> {
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

async fn handle_rent_title_input(
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
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.title_required"),
        })
        .await?;
        return Ok(());
    }

    if title.len() > 50 {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.title_too_long"),
        })
        .await?;
        return Ok(());
    }

    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        _ => return Ok(()),
    };

    if let Ok(Some(_)) = find_agreement_by_user_and_title(pool, user.id, title) {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.duplicate_title"),
        })
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
        t_with_args(language, "common.step_progress", &["2", "9"]),
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

async fn handle_rent_amount_input(
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
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.invalid_amount"),
            })
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
        t_with_args(language, "common.step_progress", &["4", "9"]),
        t(language, "agreement.rent.step4_amount.currency_prompt")
    );

    let keyboard = build_currency_keyboard();
    update_state(pool, user_id, states::RENT_CURRENCY, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

async fn handle_custom_title_input(
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
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.title_required"),
        })
        .await?;
        return Ok(());
    }

    if title.len() > 50 {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.title_too_long"),
        })
        .await?;
        return Ok(());
    }

    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        _ => return Ok(()),
    };

    if let Ok(Some(_)) = find_agreement_by_user_and_title(pool, user.id, &title) {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.duplicate_title"),
        })
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

    update_custom_state(pool, user_id, states::CUSTOM_DESCRIPTION, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

async fn handle_custom_description_input(
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
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.description_too_long"),
        })
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
    update_custom_state(pool, user_id, states::CUSTOM_REMINDER_TITLE, draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

async fn handle_custom_reminder_title_input(
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
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.reminder_title_required"),
        })
        .await?;
        return Ok(());
    }

    if title.len() > 100 {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.reminder_title_too_long"),
        })
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
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.max_reminders_reached"),
        })
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
    update_custom_state(pool, user_id, states::CUSTOM_REMINDER_DATE, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

async fn handle_custom_reminder_amount_input(
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
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.invalid_amount"),
        })
        .await?;
        return Ok(());
    }

    const MAX_AMOUNT: f64 = 10_000_000.0;
    let amount = match amount_str.parse::<f64>() {
        Ok(a) if a > 0.0 && a <= MAX_AMOUNT => (a * 100.0).round() / 100.0,
        _ => {
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.invalid_amount"),
            })
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
    update_custom_state(pool, user_id, states::CUSTOM_REMINDER_AMOUNT, &draft);
    send_message_with_keyboard(bot, chat_id, thread_id, &message, keyboard).await?;

    Ok(())
}

fn build_custom_calendar(language: &str, current: NaiveDate) -> InlineKeyboardMarkup {
    let year = current.year();
    let month = current.month();

    let month_names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let header = format!("{} {}", month_names[(month - 1) as usize], year);

    let mut rows = vec![vec![
        InlineKeyboardButton::callback(
            t(language, "agreement.calendar.prev_month"),
            format!("custom:cal:prev:{}:{}", year, month),
        ),
        InlineKeyboardButton::callback(header, "custom:cal:noop"),
        InlineKeyboardButton::callback(
            t(language, "agreement.calendar.next_month"),
            format!("custom:cal:next:{}:{}", year, month),
        ),
    ]];

    let day_headers = vec!["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
    rows.push(
        day_headers
            .into_iter()
            .map(|d| InlineKeyboardButton::callback(d.to_string(), "custom:cal:noop"))
            .collect(),
    );

    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let days_in_month = days_in_month(year, month);
    let first_weekday = first_day.weekday().num_days_from_monday() as usize;

    let mut day = 1u32;
    for _week in 0..6 {
        if day > days_in_month {
            break;
        }
        let mut row = Vec::new();
        for weekday in 0..7 {
            if (_week == 0 && weekday < first_weekday) || day > days_in_month {
                row.push(InlineKeyboardButton::callback(
                    " ".to_string(),
                    "custom:cal:noop",
                ));
            } else {
                row.push(InlineKeyboardButton::callback(
                    day.to_string(),
                    format!("custom:cal:day:{}:{}:{}", year, month, day),
                ));
                day += 1;
            }
        }
        rows.push(row);
    }

    rows.push(vec![InlineKeyboardButton::callback(
        t(language, "common.cancel_button"),
        "flow:cancel",
    )]);

    InlineKeyboardMarkup::new(rows)
}

fn build_custom_currency_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🇹🇷 TRY", "custom:currency:TRY"),
            InlineKeyboardButton::callback("🇪🇺 EUR", "custom:currency:EUR"),
        ],
        vec![
            InlineKeyboardButton::callback("🇺🇸 USD", "custom:currency:USD"),
            InlineKeyboardButton::callback("🇬🇧 GBP", "custom:currency:GBP"),
        ],
    ])
}

fn build_custom_timing_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.step7_reminder_timing.same_day"),
                "custom:timing:same_day",
            ),
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.1_day_before",
                ),
                "custom:timing:1_day_before",
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.3_days_before",
                ),
                "custom:timing:3_days_before",
            ),
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.1_week_before",
                ),
                "custom:timing:1_week_before",
            ),
        ],
    ])
}

fn build_reminder_list_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.custom.add_reminder.add_another"),
            "custom:add_another",
        )],
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.custom.add_reminder.finish"),
            "custom:finish",
        )],
        vec![InlineKeyboardButton::callback(
            t(language, "common.cancel_button"),
            "flow:cancel",
        )],
    ])
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
    update_custom_state(pool, user_id, states::CUSTOM_REMINDER_LIST, draft);
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
    update_custom_state(pool, user_id, states::CUSTOM_SUMMARY, draft);

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
        .filter_map(|r| {
            let due_date = r
                .date
                .as_ref()
                .and_then(|s| NaiveDate::parse_from_str(s, "%d.%m.%Y").ok())?;

            let timing = r.timing.as_deref().unwrap_or("same_day");
            let days_before = match timing {
                "1_day_before" => 1,
                "3_days_before" => 3,
                "1_week_before" => 7,
                _ => 0,
            };
            let reminder_date = due_date - chrono::Duration::days(days_before);

            let amount = r.amount.as_ref().and_then(|s| BigDecimal::from_str(s).ok());

            Some(NewReminder {
                agreement_id: agreement.id,
                reminder_type: if days_before > 0 {
                    "pre_notify".to_string()
                } else {
                    "due_day".to_string()
                },
                title: r.title.clone().unwrap_or_else(|| "Reminder".to_string()),
                amount,
                due_date,
                reminder_date,
            })
        })
        .collect();

    let reminder_count = reminders.len();
    if !reminders.is_empty() {
        if let Err(e) = create_reminders_batch(pool, reminders) {
            tracing::error!("Failed to create reminders: {:?}", e);
            METRICS.increment_errors();

            let _ = clear_conversation_state(pool, user_id);

            let error_message = t(language, "agreement.errors.reminders_failed");
            let keyboard = build_menu_keyboard(language);
            bot.edit_message_text(ChatId(chat_id), message_id, &error_message)
                .reply_markup(keyboard)
                .await?;
            return Ok(());
        }
    }

    let _ = clear_conversation_state(pool, user_id);

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

#[allow(clippy::too_many_arguments)]
async fn handle_edit_title_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
    state: &notifine::models::AgreementConversationState,
) -> ResponseResult<()> {
    let edit_draft: EditDraft = state
        .state_data
        .as_ref()
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    let title = sanitize_input(text);

    if title.is_empty() {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.title_required"),
        })
        .await?;
        return Ok(());
    }

    if title.len() > 50 {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.title_too_long"),
        })
        .await?;
        return Ok(());
    }

    let user = match find_agreement_user_by_telegram_id(pool, user_id) {
        Ok(Some(u)) => u,
        _ => return Ok(()),
    };

    if let Ok(Some(existing)) = find_agreement_by_user_and_title(pool, user.id, &title) {
        if existing.id != edit_draft.agreement_id {
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.duplicate_title"),
            })
            .await?;
            return Ok(());
        }
    }

    let agreement = match find_agreement_by_id(pool, edit_draft.agreement_id) {
        Ok(Some(a)) => a,
        _ => return Ok(()),
    };

    let updates = UpdateAgreement {
        title: Some(title),
        ..Default::default()
    };

    match update_agreement(pool, edit_draft.agreement_id, agreement.user_id, updates) {
        Ok(_) => {
            let _ = clear_conversation_state(pool, user_id);
            let field_name = t(language, "agreement.view.field_title");
            let success_msg = t_with_args(language, "agreement.edit.success", &[&field_name]);

            let keyboard = build_edit_menu_keyboard(
                language,
                edit_draft.agreement_id,
                &agreement.agreement_type,
            );
            send_message_with_keyboard(
                bot,
                chat_id,
                thread_id,
                &format!(
                    "{}\n\n{}\n\n{}",
                    success_msg,
                    t(language, "agreement.edit.title"),
                    t(language, "agreement.edit.select_field")
                ),
                keyboard,
            )
            .await?;
        }
        Err(e) => {
            tracing::error!("Failed to update agreement: {:?}", e);
            METRICS.increment_errors();
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.errors.database_error"),
            })
            .await?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_edit_amount_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
    state: &notifine::models::AgreementConversationState,
) -> ResponseResult<()> {
    let edit_draft: EditDraft = state
        .state_data
        .as_ref()
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    let cleaned = text.trim().replace(',', ".").replace(' ', "");

    if cleaned.contains('e') || cleaned.contains('E') {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.invalid_amount"),
        })
        .await?;
        return Ok(());
    }

    let amount = match BigDecimal::from_str(&cleaned) {
        Ok(a) => a,
        Err(_) => {
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.invalid_amount"),
            })
            .await?;
            return Ok(());
        }
    };

    if amount <= BigDecimal::from(0) {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.amount_zero"),
        })
        .await?;
        return Ok(());
    }

    if amount > BigDecimal::from(10_000_000) {
        send_telegram_message(TelegramMessage {
            chat_id,
            thread_id,
            message: t(language, "agreement.validation.amount_too_high"),
        })
        .await?;
        return Ok(());
    }

    let agreement = match find_agreement_by_id(pool, edit_draft.agreement_id) {
        Ok(Some(a)) => a,
        _ => return Ok(()),
    };

    let updates = UpdateAgreement {
        rent_amount: Some(Some(amount)),
        ..Default::default()
    };

    match update_agreement(pool, edit_draft.agreement_id, agreement.user_id, updates) {
        Ok(_) => {
            let _ = clear_conversation_state(pool, user_id);
            let field_name = t(language, "agreement.view.field_amount");
            let success_msg = t_with_args(language, "agreement.edit.success", &[&field_name]);

            let keyboard = build_edit_menu_keyboard(
                language,
                edit_draft.agreement_id,
                &agreement.agreement_type,
            );
            send_message_with_keyboard(
                bot,
                chat_id,
                thread_id,
                &format!(
                    "{}\n\n{}\n\n{}",
                    success_msg,
                    t(language, "agreement.edit.title"),
                    t(language, "agreement.edit.select_field")
                ),
                keyboard,
            )
            .await?;
        }
        Err(e) => {
            tracing::error!("Failed to update agreement: {:?}", e);
            METRICS.increment_errors();
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.errors.database_error"),
            })
            .await?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_edit_description_input(
    pool: &DbPool,
    bot: &Bot,
    chat_id: i64,
    thread_id: Option<i32>,
    user_id: i64,
    language: &str,
    text: &str,
    state: &notifine::models::AgreementConversationState,
) -> ResponseResult<()> {
    let edit_draft: EditDraft = state
        .state_data
        .as_ref()
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .unwrap_or_default();

    let description = if text.trim() == "-" {
        None
    } else {
        let desc = sanitize_input(text);
        if desc.len() > 200 {
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.description_too_long"),
            })
            .await?;
            return Ok(());
        }
        Some(desc)
    };

    let agreement = match find_agreement_by_id(pool, edit_draft.agreement_id) {
        Ok(Some(a)) => a,
        _ => return Ok(()),
    };

    let updates = UpdateAgreement {
        description: Some(description),
        ..Default::default()
    };

    match update_agreement(pool, edit_draft.agreement_id, agreement.user_id, updates) {
        Ok(_) => {
            let _ = clear_conversation_state(pool, user_id);
            let field_name = t(language, "agreement.view.field_description");
            let success_msg = t_with_args(language, "agreement.edit.success", &[&field_name]);

            let keyboard = build_edit_menu_keyboard(
                language,
                edit_draft.agreement_id,
                &agreement.agreement_type,
            );
            send_message_with_keyboard(
                bot,
                chat_id,
                thread_id,
                &format!(
                    "{}\n\n{}\n\n{}",
                    success_msg,
                    t(language, "agreement.edit.title"),
                    t(language, "agreement.edit.select_field")
                ),
                keyboard,
            )
            .await?;
        }
        Err(e) => {
            tracing::error!("Failed to update agreement: {:?}", e);
            METRICS.increment_errors();
            send_telegram_message(TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.errors.database_error"),
            })
            .await?;
        }
    }

    Ok(())
}

async fn handle_reminder_callback(
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
        let reminder_id: i32 = match reminder_id_str.parse() {
            Ok(id) => id,
            Err(_) => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.reminder.not_found"))
                    .await?;
                return Ok(());
            }
        };

        let reminder = match find_reminder_by_id(pool, reminder_id) {
            Ok(Some(r)) => r,
            _ => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.reminder.not_found"))
                    .await?;
                return Ok(());
            }
        };

        let agreement = match find_agreement_by_id(pool, reminder.agreement_id) {
            Ok(Some(a)) => a,
            _ => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.reminder.not_found"))
                    .await?;
                return Ok(());
            }
        };

        let user = match find_agreement_user_by_telegram_id(pool, user_id) {
            Ok(Some(u)) => u,
            _ => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.errors.user_not_found"))
                    .await?;
                return Ok(());
            }
        };

        if agreement.user_id != user.id {
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.delete.unauthorized"))
                .await?;
            return Ok(());
        }

        if let Err(e) = update_reminder_status(pool, reminder_id, "done") {
            tracing::error!("Failed to mark reminder as done: {:?}", e);
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.errors.database_error"))
                .await?;
            return Ok(());
        }

        bot.answer_callback_query(&q.id)
            .text(t(&language, "agreement.reminder.marked_done"))
            .await?;

        bot.edit_message_reply_markup(msg.chat.id, msg.id).await?;
    } else if let Some(reminder_id_str) = data.strip_prefix("rem:snooze:") {
        let reminder_id: i32 = match reminder_id_str.parse() {
            Ok(id) => id,
            Err(_) => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.reminder.not_found"))
                    .await?;
                return Ok(());
            }
        };

        let reminder = match find_reminder_by_id(pool, reminder_id) {
            Ok(Some(r)) => r,
            _ => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.reminder.not_found"))
                    .await?;
                return Ok(());
            }
        };

        let agreement = match find_agreement_by_id(pool, reminder.agreement_id) {
            Ok(Some(a)) => a,
            _ => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.reminder.not_found"))
                    .await?;
                return Ok(());
            }
        };

        let user = match find_agreement_user_by_telegram_id(pool, user_id) {
            Ok(Some(u)) => u,
            _ => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.errors.user_not_found"))
                    .await?;
                return Ok(());
            }
        };

        if agreement.user_id != user.id {
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.delete.unauthorized"))
                .await?;
            return Ok(());
        }

        let keyboard = build_snooze_options_keyboard(reminder_id, &language);
        bot.edit_message_reply_markup(msg.chat.id, msg.id)
            .reply_markup(keyboard)
            .await?;
        bot.answer_callback_query(&q.id).await?;
    } else if let Some(rest) = data.strip_prefix("rem:snooze_") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() != 2 {
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.reminder.not_found"))
                .await?;
            return Ok(());
        }

        let duration = parts[0];
        let reminder_id: i32 = match parts[1].parse() {
            Ok(id) => id,
            Err(_) => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.reminder.not_found"))
                    .await?;
                return Ok(());
            }
        };

        let reminder = match find_reminder_by_id(pool, reminder_id) {
            Ok(Some(r)) => r,
            _ => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.reminder.not_found"))
                    .await?;
                return Ok(());
            }
        };

        let agreement = match find_agreement_by_id(pool, reminder.agreement_id) {
            Ok(Some(a)) => a,
            _ => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.reminder.not_found"))
                    .await?;
                return Ok(());
            }
        };

        let user = match find_agreement_user_by_telegram_id(pool, user_id) {
            Ok(Some(u)) => u,
            _ => {
                bot.answer_callback_query(&q.id)
                    .text(t(&language, "agreement.errors.user_not_found"))
                    .await?;
                return Ok(());
            }
        };

        if agreement.user_id != user.id {
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.delete.unauthorized"))
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
                    .text(t(&language, "agreement.reminder.not_found"))
                    .await?;
                return Ok(());
            }
        };

        if let Err(e) = update_reminder_snooze(pool, reminder_id, snooze_until) {
            tracing::error!("Failed to snooze reminder: {:?}", e);
            bot.answer_callback_query(&q.id)
                .text(t(&language, "agreement.errors.database_error"))
                .await?;
            return Ok(());
        }

        let snooze_display = snooze_until.format("%d.%m.%Y %H:%M").to_string();
        let message = t_with_args(&language, "agreement.reminder.snoozed", &[&snooze_display]);

        bot.answer_callback_query(&q.id).text(&message).await?;
        bot.edit_message_reply_markup(msg.chat.id, msg.id).await?;
    }

    Ok(())
}

fn build_snooze_options_keyboard(reminder_id: i32, lang: &str) -> InlineKeyboardMarkup {
    let row1 = vec![
        InlineKeyboardButton::callback(
            t(lang, "agreement.reminder.snooze_1h"),
            format!("rem:snooze_1h:{}", reminder_id),
        ),
        InlineKeyboardButton::callback(
            t(lang, "agreement.reminder.snooze_3h"),
            format!("rem:snooze_3h:{}", reminder_id),
        ),
    ];

    let row2 = vec![
        InlineKeyboardButton::callback(
            t(lang, "agreement.reminder.snooze_1d"),
            format!("rem:snooze_1d:{}", reminder_id),
        ),
        InlineKeyboardButton::callback(
            t(lang, "agreement.reminder.snooze_3d"),
            format!("rem:snooze_3d:{}", reminder_id),
        ),
    ];

    InlineKeyboardMarkup::new(vec![row1, row2])
}

pub async fn run_bot(pool: DbPool) {
    tracing::info!("Starting Agreement bot...");

    let token = env::var("AGREEMENT_BOT_TOKEN").expect("AGREEMENT_BOT_TOKEN must be set");
    let bot = Bot::new(token);

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint({
                    let pool = pool.clone();
                    move |bot: Bot, msg: Message, cmd: Command| {
                        let pool = pool.clone();
                        async move { command_handler(bot, msg, cmd, pool).await }
                    }
                }),
        )
        .branch(Update::filter_message().endpoint({
            let pool = pool.clone();
            move |bot: Bot, msg: Message| {
                let pool = pool.clone();
                async move { message_handler(bot, msg, pool).await }
            }
        }))
        .branch(Update::filter_callback_query().endpoint({
            let pool = pool.clone();
            move |bot: Bot, q: CallbackQuery| {
                let pool = pool.clone();
                async move { callback_handler(bot, q, pool).await }
            }
        }));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
