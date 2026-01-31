use chrono::{Duration, NaiveDate};
use chrono_tz::Europe::Istanbul;
use diesel::prelude::*;
use notifine::db::{DbError, DbPool};
use notifine::models::{ChatEvent, NewChatEvent};
use notifine::schema::{chat_bot_subscriptions, chat_events};

#[allow(clippy::too_many_arguments)]
pub fn record_chat_event(
    pool: &DbPool,
    telegram_chat_id: i64,
    event_type: &str,
    bot_type: &str,
    inviter_username: Option<&str>,
    is_cross_bot_user: bool,
    other_bots: Option<&str>,
    chat_title: Option<&str>,
) -> Result<ChatEvent, DbError> {
    let conn = &mut pool.get()?;

    let new_event = NewChatEvent {
        telegram_chat_id,
        event_type,
        bot_type,
        inviter_username,
        is_cross_bot_user,
        other_bots,
        chat_title,
    };

    Ok(diesel::insert_into(chat_events::table)
        .values(&new_event)
        .get_result(conn)?)
}

pub fn get_chat_events_for_date(pool: &DbPool, date: NaiveDate) -> Result<Vec<ChatEvent>, DbError> {
    let conn = &mut pool.get()?;

    let start_of_day = date
        .and_hms_opt(0, 0, 0)
        .expect("valid time")
        .and_local_timezone(Istanbul)
        .single()
        .expect("valid Istanbul timezone")
        .to_utc();
    let start_of_next_day = (date + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .expect("valid time")
        .and_local_timezone(Istanbul)
        .single()
        .expect("valid Istanbul timezone")
        .to_utc();

    Ok(chat_events::table
        .filter(chat_events::created_at.ge(start_of_day))
        .filter(chat_events::created_at.lt(start_of_next_day))
        .order(chat_events::created_at.asc())
        .load(conn)?)
}

pub fn get_other_bots_for_chat(
    pool: &DbPool,
    telegram_chat_id: i64,
    exclude_bot: &str,
) -> Result<Vec<String>, DbError> {
    let conn = &mut pool.get()?;

    let bots: Vec<String> = chat_bot_subscriptions::table
        .filter(chat_bot_subscriptions::telegram_chat_id.eq(telegram_chat_id))
        .filter(chat_bot_subscriptions::is_reachable.eq(true))
        .filter(chat_bot_subscriptions::bot_type.ne(exclude_bot))
        .select(chat_bot_subscriptions::bot_type)
        .load(conn)?;

    Ok(bots)
}

pub fn get_remaining_bots_for_chat(
    pool: &DbPool,
    telegram_chat_id: i64,
) -> Result<Vec<String>, DbError> {
    let conn = &mut pool.get()?;

    let bots: Vec<String> = chat_bot_subscriptions::table
        .filter(chat_bot_subscriptions::telegram_chat_id.eq(telegram_chat_id))
        .filter(chat_bot_subscriptions::is_reachable.eq(true))
        .select(chat_bot_subscriptions::bot_type)
        .load(conn)?;

    Ok(bots)
}

pub fn record_new_chat_event(
    pool: &DbPool,
    telegram_chat_id: i64,
    bot_type: &str,
    inviter_username: Option<&str>,
    chat_title: Option<&str>,
) -> Result<ChatEvent, DbError> {
    let other_bots = get_other_bots_for_chat(pool, telegram_chat_id, bot_type)?;
    let is_cross_bot_user = !other_bots.is_empty();
    let other_bots_str = if is_cross_bot_user {
        Some(other_bots.join(","))
    } else {
        None
    };

    record_chat_event(
        pool,
        telegram_chat_id,
        "new",
        bot_type,
        inviter_username,
        is_cross_bot_user,
        other_bots_str.as_deref(),
        chat_title,
    )
}

pub fn record_churn_event(
    pool: &DbPool,
    telegram_chat_id: i64,
    bot_type: &str,
    chat_title: Option<&str>,
) -> Result<ChatEvent, DbError> {
    let remaining_bots = get_remaining_bots_for_chat(pool, telegram_chat_id)?;
    let has_remaining_bots = !remaining_bots.is_empty();
    let remaining_bots_str = if has_remaining_bots {
        Some(remaining_bots.join(","))
    } else {
        None
    };

    record_chat_event(
        pool,
        telegram_chat_id,
        "churn",
        bot_type,
        None,
        has_remaining_bots,
        remaining_bots_str.as_deref(),
        chat_title,
    )
}
