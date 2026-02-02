use crate::bots::bot_service::TelegramMessage;
use crate::observability::METRICS;
use bigdecimal::BigDecimal;
use chrono::Utc;
use notifine::db::DbPool;
use notifine::i18n::{t, t_with_args};
use notifine::models::{Agreement, UpdateAgreement};
use notifine::{
    clear_conversation_state, find_agreement_by_id, find_agreement_by_user_and_title,
    find_agreement_user_by_telegram_id, set_conversation_state, update_agreement,
};
use std::str::FromStr;
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;

use crate::bots::agreement_bot::keyboards::{
    build_cancel_keyboard, build_edit_due_day_keyboard, build_edit_menu_keyboard,
    build_edit_timing_keyboard,
};
use crate::bots::agreement_bot::types::{sanitize_input, states, EditDraft, STATE_EXPIRY_MINUTES};
use crate::bots::agreement_bot::utils::{send_message_with_keyboard, send_telegram_message};

pub async fn handle_edit_callback(
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
                            if let Err(e) = clear_conversation_state(pool, telegram_user_id) {
                                tracing::warn!(
                                    "Failed to clear conversation state for user {}: {:?}",
                                    telegram_user_id,
                                    e
                                );
                            }
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

#[allow(clippy::too_many_arguments)]
pub async fn handle_edit_title_input(
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

    if let Ok(Some(existing)) = find_agreement_by_user_and_title(pool, user.id, &title) {
        if existing.id != edit_draft.agreement_id {
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
            if let Err(e) = clear_conversation_state(pool, user_id) {
                tracing::warn!(
                    "Failed to clear conversation state for user {}: {:?}",
                    user_id,
                    e
                );
            }
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.errors.database_error"),
                },
            )
            .await?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_edit_amount_input(
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

    let amount = match BigDecimal::from_str(&cleaned) {
        Ok(a) => a,
        Err(_) => {
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

    if amount <= BigDecimal::from(0) {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.amount_zero"),
            },
        )
        .await?;
        return Ok(());
    }

    if amount > BigDecimal::from(10_000_000) {
        send_telegram_message(
            bot,
            TelegramMessage {
                chat_id,
                thread_id,
                message: t(language, "agreement.validation.amount_too_high"),
            },
        )
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
            if let Err(e) = clear_conversation_state(pool, user_id) {
                tracing::warn!(
                    "Failed to clear conversation state for user {}: {:?}",
                    user_id,
                    e
                );
            }
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.errors.database_error"),
                },
            )
            .await?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_edit_description_input(
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
            if let Err(e) = clear_conversation_state(pool, user_id) {
                tracing::warn!(
                    "Failed to clear conversation state for user {}: {:?}",
                    user_id,
                    e
                );
            }
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
            send_telegram_message(
                bot,
                TelegramMessage {
                    chat_id,
                    thread_id,
                    message: t(language, "agreement.errors.database_error"),
                },
            )
            .await?;
        }
    }

    Ok(())
}
