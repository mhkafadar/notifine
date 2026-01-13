use notifine::db::DbPool;
use notifine::i18n::t;
use teloxide::prelude::*;
use teloxide::types::CallbackQuery;

use crate::bots::agreement_bot::flows::{
    handle_agreement_callback, handle_custom_callback, handle_rent_callback,
};
use crate::bots::agreement_bot::utils::get_user_language;

use super::{
    handle_disclaimer_accept, handle_disclaimer_decline, handle_flow_cancel,
    handle_language_select, handle_menu_select, handle_reminder_callback,
    handle_settings_language_menu, handle_settings_timezone_menu, handle_timezone_select,
};

pub async fn callback_handler(bot: Bot, q: CallbackQuery, pool: DbPool) -> ResponseResult<()> {
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
        handle_settings_language_menu(&pool, &bot, &q, user_id).await?;
    } else if data == "settings:timezone" {
        handle_settings_timezone_menu(&pool, &bot, &q, user_id).await?;
    } else {
        let language = get_user_language(&pool, user_id);
        bot.answer_callback_query(&q.id)
            .text(t(&language, "agreement.errors.unknown_callback"))
            .await?;
    }

    Ok(())
}
