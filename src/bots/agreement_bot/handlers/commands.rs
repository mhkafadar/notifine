use crate::bots::bot_service::TelegramMessage;
use crate::observability::alerts::Severity;
use crate::observability::{ALERTS, METRICS};
use crate::services::broadcast::commands::{
    handle_approve_all, handle_broadcast, handle_broadcast_cancel, handle_broadcast_status,
    handle_broadcast_test, handle_pending_list, handle_reject_all,
};
use crate::services::broadcast::db::upsert_chat_bot_subscription;
use crate::services::broadcast::types::BotType;
use notifine::db::DbPool;
use notifine::i18n::{t, t_with_args};
use notifine::models::NewAgreementUser;
use notifine::{
    clear_conversation_state, create_agreement_user, find_agreement_user_by_telegram_id,
    find_agreements_by_user_id,
};
use teloxide::macros::BotCommands;
use teloxide::prelude::*;

use super::super::keyboards::{
    build_agreements_list_keyboard, build_disclaimer_keyboard, build_language_keyboard,
    build_menu_keyboard, build_settings_keyboard, build_timezone_keyboard,
};
use super::super::types::{DEFAULT_LANGUAGE, DEFAULT_TIMEZONE};
use super::super::utils::{
    detect_language_from_message, get_user_language, send_message_with_keyboard,
    send_telegram_message,
};

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
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
    #[command(
        description = "Send a broadcast message to all users (admin only). Usage: /broadcast [--discover] <message>"
    )]
    Broadcast,
    #[command(
        description = "Test broadcast (dry run, shows target count). Usage: /broadcasttest [--discover] <message>"
    )]
    Broadcasttest,
    #[command(description = "Show recent broadcast job status (admin only)")]
    Broadcaststatus,
    #[command(
        description = "Cancel a broadcast job (admin only). Usage: /broadcastcancel <job_id>"
    )]
    Broadcastcancel,
    #[command(description = "List pending chat deactivations (admin only)")]
    Pendinglist,
    #[command(description = "Approve all pending deactivations (admin only)")]
    Approveall,
    #[command(description = "Reject all pending deactivations (admin only)")]
    Rejectall,
}

pub async fn command_handler(
    bot: Bot,
    msg: Message,
    command: Command,
    pool: DbPool,
    admin_chat_id: Option<i64>,
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
        Command::Broadcast => handle_broadcast(&bot, &msg, &pool, admin_chat_id).await?,
        Command::Broadcasttest => handle_broadcast_test(&bot, &msg, &pool, admin_chat_id).await?,
        Command::Broadcaststatus => {
            handle_broadcast_status(&bot, &msg, &pool, admin_chat_id).await?
        }
        Command::Broadcastcancel => {
            handle_broadcast_cancel(&bot, &msg, &pool, admin_chat_id).await?
        }
        Command::Pendinglist => handle_pending_list(&bot, &msg, &pool, admin_chat_id).await?,
        Command::Approveall => handle_approve_all(&bot, &msg, &pool, admin_chat_id).await?,
        Command::Rejectall => handle_reject_all(&bot, &msg, &pool, admin_chat_id).await?,
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(DEFAULT_LANGUAGE, "agreement.errors.database_error"),
                },
            )
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
                send_telegram_message(
                    bot,
                    TelegramMessage {
                        chat_id,
                        thread_id,
                        message: t(&language, "agreement.errors.database_error"),
                    },
                )
                .await?;
                return Ok(());
            }

            let welcome_message = format!(
                "{}\n\n{}",
                t(&language, "agreement.welcome.title"),
                t(&language, "agreement.welcome.description")
            );
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: welcome_message,
                },
            )
            .await?;

            send_disclaimer(bot, chat_id, thread_id, &language).await?;
        }
    }

    if let Err(e) = upsert_chat_bot_subscription(pool, chat_id, BotType::Agreement, true) {
        tracing::warn!("Failed to track subscription for Agreement bot: {:?}", e);
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
    bot: &Bot,
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

    send_telegram_message(
        bot,
        TelegramMessage {
            chat_id,
            thread_id,
            message: help_message,
        },
    )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message,
                },
            )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(&language, "agreement.errors.database_error"),
                },
            )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(&language, "agreement.errors.must_accept_disclaimer"),
                },
            )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(&language, "agreement.errors.database_error"),
                },
            )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(&language, "agreement.errors.database_error"),
                },
            )
            .await?;
            return Ok(());
        }
    };

    if agreements.is_empty() {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(&language, "agreement.list.empty"),
            },
        )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(DEFAULT_LANGUAGE, "agreement.errors.user_not_found"),
                },
            )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(DEFAULT_LANGUAGE, "agreement.errors.database_error"),
                },
            )
            .await?;
            return Ok(());
        }
    };

    if !user.disclaimer_accepted {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(&user.language, "agreement.errors.must_accept_disclaimer"),
            },
        )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(DEFAULT_LANGUAGE, "agreement.errors.user_not_found"),
                },
            )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(DEFAULT_LANGUAGE, "agreement.errors.database_error"),
                },
            )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(DEFAULT_LANGUAGE, "agreement.errors.user_not_found"),
                },
            )
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(DEFAULT_LANGUAGE, "agreement.errors.database_error"),
                },
            )
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
