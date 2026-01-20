use crate::observability::alerts::Severity;
use crate::observability::{ALERTS, METRICS};
use chrono::{NaiveDate, Timelike, Utc};
use chrono_tz::Tz;
use html_escape::encode_text;
use notifine::db::DbPool;
use notifine::i18n::{t, t_with_args};
use notifine::{
    find_due_reminders_with_user_info, update_reminder_status, DueReminderWithUserInfo,
};
use std::env;
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};

const CHECK_INTERVAL_SECS: u64 = 60;

pub async fn run_reminder_scheduler(pool: DbPool) {
    tracing::info!("Starting reminder scheduler...");

    let token = match env::var("AGREEMENT_BOT_TOKEN") {
        Ok(t) => t,
        Err(_) => {
            tracing::error!("AGREEMENT_BOT_TOKEN not set, reminder scheduler disabled");
            return;
        }
    };

    let bot = Bot::new(token);

    loop {
        if let Err(e) = check_and_send_reminders(&pool, &bot).await {
            tracing::error!("Error in reminder scheduler: {:?}", e);
            METRICS.increment_errors();
        }

        tokio::time::sleep(Duration::from_secs(CHECK_INTERVAL_SECS)).await;
    }
}

async fn check_and_send_reminders(
    pool: &DbPool,
    bot: &Bot,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let today = Utc::now().date_naive();

    let due_reminders = match find_due_reminders_with_user_info(pool, today) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to fetch due reminders: {:?}", e);
            ALERTS
                .send_alert(
                    bot,
                    Severity::Error,
                    "ReminderScheduler",
                    &format!("Failed to fetch due reminders: {}", e),
                )
                .await;
            return Ok(());
        }
    };

    if due_reminders.is_empty() {
        return Ok(());
    }

    tracing::info!("Found {} due reminders to send", due_reminders.len());

    for due_reminder in due_reminders {
        if should_send_reminder(&due_reminder) {
            if let Err(e) = send_reminder_notification(pool, bot, &due_reminder).await {
                tracing::error!(
                    "Failed to send reminder {} for user {}: {:?}",
                    due_reminder.reminder.id,
                    due_reminder.user.telegram_user_id,
                    e
                );
            }
        }
    }

    Ok(())
}

fn should_send_reminder(due_reminder: &DueReminderWithUserInfo) -> bool {
    let user_tz: Tz = due_reminder
        .user
        .timezone
        .parse()
        .unwrap_or(chrono_tz::Europe::Istanbul);

    let now_in_user_tz = Utc::now().with_timezone(&user_tz);
    let user_hour = now_in_user_tz.hour();

    (8..22).contains(&user_hour)
}

async fn send_reminder_notification(
    pool: &DbPool,
    bot: &Bot,
    due_reminder: &DueReminderWithUserInfo,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let reminder_id = due_reminder.reminder.id;
    let lang = &due_reminder.user.language;
    let chat_id = ChatId(due_reminder.user.telegram_chat_id);

    if let Err(e) = update_reminder_status(pool, reminder_id, "sending") {
        tracing::error!(
            "Failed to mark reminder {} as sending: {:?}",
            reminder_id,
            e
        );
        return Err(Box::new(std::io::Error::other(format!(
            "Failed to mark reminder as sending: {}",
            e
        ))));
    }

    let message = format_reminder_message(due_reminder, lang);
    let keyboard = build_reminder_keyboard(reminder_id, lang);

    match bot
        .send_message(chat_id, message)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await
    {
        Ok(_) => {
            if let Err(e) = update_reminder_status(pool, reminder_id, "sent") {
                tracing::error!(
                    "Failed to update reminder {} status to sent: {:?}",
                    reminder_id,
                    e
                );
                ALERTS
                    .send_alert(
                        bot,
                        Severity::Warning,
                        "ReminderScheduler",
                        &format!(
                            "Sent reminder {} but failed to update status: {}",
                            reminder_id, e
                        ),
                    )
                    .await;
            }
            tracing::info!(
                "Sent reminder {} to user {}",
                reminder_id,
                due_reminder.user.telegram_user_id
            );
        }
        Err(e) => {
            let error_str = e.to_string();
            let is_blocked_or_unavailable = error_str.contains("bot was blocked")
                || error_str.contains("chat not found")
                || error_str.contains("user is deactivated")
                || error_str.contains("FORBIDDEN")
                || error_str.contains("chat_id is empty");

            if is_blocked_or_unavailable {
                tracing::warn!(
                    "User {} unavailable for reminder {}: {}",
                    due_reminder.user.telegram_user_id,
                    reminder_id,
                    error_str
                );
                if let Err(e) = update_reminder_status(pool, reminder_id, "failed") {
                    tracing::error!("Failed to mark reminder {} as failed: {:?}", reminder_id, e);
                }
            } else {
                if let Err(revert_err) = update_reminder_status(pool, reminder_id, "pending") {
                    tracing::error!(
                        "Failed to revert reminder {} to pending: {:?}",
                        reminder_id,
                        revert_err
                    );
                }
                return Err(Box::new(e));
            }
        }
    }

    Ok(())
}

fn format_reminder_message(due_reminder: &DueReminderWithUserInfo, lang: &str) -> String {
    let reminder = &due_reminder.reminder;
    let agreement = &due_reminder.agreement;

    let type_emoji = match reminder.reminder_type.as_str() {
        "rent_payment" | "rent_collection" => "🏠",
        "yearly_increase" => "📈",
        _ => "📝",
    };

    let title = t_with_args(lang, "agreement.reminder.notification_title", &[type_emoji]);

    let agreement_line = t_with_args(
        lang,
        "agreement.reminder.agreement_name",
        &[encode_text(&agreement.title).as_ref()],
    );

    let reminder_line = t_with_args(
        lang,
        "agreement.reminder.reminder_title",
        &[encode_text(&reminder.title).as_ref()],
    );

    let due_date_line = t_with_args(
        lang,
        "agreement.reminder.due_date",
        &[&format_date(reminder.due_date, lang)],
    );

    let amount_line = if let Some(ref amount) = reminder.amount {
        let currency = encode_text(&agreement.currency);
        format!(
            "\n{}",
            t_with_args(
                lang,
                "agreement.reminder.amount",
                &[&amount.to_string(), currency.as_ref()]
            )
        )
    } else {
        String::new()
    };

    let days_info = get_days_info(reminder.reminder_date, reminder.due_date, lang);

    format!(
        "<b>{}</b>\n\n{}\n{}\n{}{}{}",
        title, agreement_line, reminder_line, due_date_line, amount_line, days_info
    )
}

fn get_days_info(_reminder_date: NaiveDate, due_date: NaiveDate, lang: &str) -> String {
    let today = Utc::now().date_naive();
    let days_until_due = (due_date - today).num_days();

    if days_until_due < 0 {
        let days_overdue = -days_until_due;
        format!(
            "\n\n⚠️ {}",
            t_with_args(
                lang,
                "agreement.reminder.overdue",
                &[&days_overdue.to_string()]
            )
        )
    } else if days_until_due == 0 {
        format!("\n\n⏰ {}", t(lang, "agreement.reminder.due_today"))
    } else if days_until_due <= 7 {
        format!(
            "\n\n📅 {}",
            t_with_args(
                lang,
                "agreement.reminder.days_left",
                &[&days_until_due.to_string()]
            )
        )
    } else {
        String::new()
    }
}

fn format_date(date: NaiveDate, _lang: &str) -> String {
    date.format("%d.%m.%Y").to_string()
}

fn build_reminder_keyboard(reminder_id: i32, lang: &str) -> InlineKeyboardMarkup {
    let done_button = InlineKeyboardButton::callback(
        t(lang, "agreement.reminder.mark_done"),
        format!("rem:done:{}", reminder_id),
    );

    let snooze_button = InlineKeyboardButton::callback(
        t(lang, "agreement.reminder.snooze"),
        format!("rem:snooze:{}", reminder_id),
    );

    InlineKeyboardMarkup::new(vec![vec![done_button, snooze_button]])
}
