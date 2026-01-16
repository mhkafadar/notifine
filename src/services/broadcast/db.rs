use chrono::Utc;
use diesel::prelude::*;
use notifine::db::{DbError, DbPool};
use notifine::schema::{
    agreement_users, broadcast_jobs, chat_bot_subscriptions, chats, pending_deactivations,
};
use std::collections::HashMap;

use super::types::{
    BotType, BroadcastJob, BroadcastStatus, BroadcastTarget, ChatBotSubscription,
    DeactivationStatus, PendingDeactivation,
};

#[derive(Insertable)]
#[diesel(table_name = broadcast_jobs)]
struct NewBroadcastJob<'a> {
    message: &'a str,
    status: &'a str,
    created_by_chat_id: i64,
    rate_limit_per_sec: i32,
    discovery_mode: bool,
}

#[derive(Insertable)]
#[diesel(table_name = chat_bot_subscriptions)]
struct NewChatBotSubscription<'a> {
    telegram_chat_id: i64,
    bot_type: &'a str,
    is_reachable: bool,
}

#[derive(Insertable)]
#[diesel(table_name = pending_deactivations)]
struct NewPendingDeactivation<'a> {
    telegram_chat_id: i64,
    source_broadcast_job_id: Option<i32>,
    failed_bots: &'a [Option<String>],
    last_error: Option<&'a str>,
    status: &'a str,
}

pub fn create_broadcast_job(
    pool: &DbPool,
    message: &str,
    created_by_chat_id: i64,
    rate_limit: i32,
    discovery_mode: bool,
) -> Result<BroadcastJob, DbError> {
    let conn = &mut pool.get()?;

    let new_job = NewBroadcastJob {
        message,
        status: BroadcastStatus::Pending.as_str(),
        created_by_chat_id,
        rate_limit_per_sec: rate_limit,
        discovery_mode,
    };

    Ok(diesel::insert_into(broadcast_jobs::table)
        .values(&new_job)
        .get_result(conn)?)
}

pub fn get_pending_or_running_jobs(pool: &DbPool) -> Result<Vec<BroadcastJob>, DbError> {
    let conn = &mut pool.get()?;

    Ok(broadcast_jobs::table
        .filter(
            broadcast_jobs::status
                .eq(BroadcastStatus::Pending.as_str())
                .or(broadcast_jobs::status.eq(BroadcastStatus::Running.as_str()))
                .or(broadcast_jobs::status.eq(BroadcastStatus::Paused.as_str())),
        )
        .order(broadcast_jobs::created_at.asc())
        .load::<BroadcastJob>(conn)?)
}

pub fn get_recent_jobs(pool: &DbPool, limit: i64) -> Result<Vec<BroadcastJob>, DbError> {
    let conn = &mut pool.get()?;

    Ok(broadcast_jobs::table
        .order(broadcast_jobs::created_at.desc())
        .limit(limit)
        .load::<BroadcastJob>(conn)?)
}

pub fn update_job_status(
    pool: &DbPool,
    job_id: i32,
    status: BroadcastStatus,
) -> Result<(), DbError> {
    let conn = &mut pool.get()?;

    let now = Utc::now();
    match status {
        BroadcastStatus::Running => {
            diesel::update(broadcast_jobs::table.filter(broadcast_jobs::id.eq(job_id)))
                .set((
                    broadcast_jobs::status.eq(status.as_str()),
                    broadcast_jobs::started_at.eq(Some(now)),
                    broadcast_jobs::updated_at.eq(now),
                ))
                .execute(conn)?;
        }
        BroadcastStatus::Completed | BroadcastStatus::Cancelled | BroadcastStatus::Failed => {
            diesel::update(broadcast_jobs::table.filter(broadcast_jobs::id.eq(job_id)))
                .set((
                    broadcast_jobs::status.eq(status.as_str()),
                    broadcast_jobs::completed_at.eq(Some(now)),
                    broadcast_jobs::updated_at.eq(now),
                ))
                .execute(conn)?;
        }
        _ => {
            diesel::update(broadcast_jobs::table.filter(broadcast_jobs::id.eq(job_id)))
                .set((
                    broadcast_jobs::status.eq(status.as_str()),
                    broadcast_jobs::updated_at.eq(now),
                ))
                .execute(conn)?;
        }
    }

    Ok(())
}

pub fn update_job_total_chats(pool: &DbPool, job_id: i32, total: i32) -> Result<(), DbError> {
    let conn = &mut pool.get()?;

    diesel::update(broadcast_jobs::table.filter(broadcast_jobs::id.eq(job_id)))
        .set((
            broadcast_jobs::total_chats.eq(total),
            broadcast_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(conn)?;

    Ok(())
}

pub fn update_job_progress(
    pool: &DbPool,
    job_id: i32,
    last_chat_id: i64,
    success: bool,
    unreachable: bool,
) -> Result<BroadcastJob, DbError> {
    let conn = &mut pool.get()?;

    let mut success_increment = 0;
    let mut failed_increment = 0;
    let mut unreachable_increment = 0;

    if success {
        success_increment = 1;
    } else if unreachable {
        unreachable_increment = 1;
    } else {
        failed_increment = 1;
    }

    diesel::update(broadcast_jobs::table.filter(broadcast_jobs::id.eq(job_id)))
        .set((
            broadcast_jobs::processed_count.eq(broadcast_jobs::processed_count + 1),
            broadcast_jobs::success_count.eq(broadcast_jobs::success_count + success_increment),
            broadcast_jobs::failed_count.eq(broadcast_jobs::failed_count + failed_increment),
            broadcast_jobs::unreachable_count
                .eq(broadcast_jobs::unreachable_count + unreachable_increment),
            broadcast_jobs::last_processed_chat_id.eq(Some(last_chat_id)),
            broadcast_jobs::updated_at.eq(Utc::now()),
        ))
        .get_result(conn)
        .map_err(DbError::from)
}

pub fn update_job_error(pool: &DbPool, job_id: i32, error: &str) -> Result<(), DbError> {
    let conn = &mut pool.get()?;

    diesel::update(broadcast_jobs::table.filter(broadcast_jobs::id.eq(job_id)))
        .set((
            broadcast_jobs::status.eq(BroadcastStatus::Failed.as_str()),
            broadcast_jobs::error_message.eq(Some(error)),
            broadcast_jobs::completed_at.eq(Some(Utc::now())),
            broadcast_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(conn)?;

    Ok(())
}

pub fn get_all_broadcast_targets(pool: &DbPool) -> Result<Vec<BroadcastTarget>, DbError> {
    let conn = &mut pool.get()?;

    let chat_results: Vec<(String, Option<String>)> = chats::table
        .filter(chats::is_active.eq(true))
        .select((chats::telegram_id, chats::thread_id))
        .load(conn)?;

    let agreement_results: Vec<i64> = agreement_users::table
        .select(agreement_users::telegram_chat_id)
        .load(conn)?;

    let subscriptions: Vec<ChatBotSubscription> = chat_bot_subscriptions::table
        .filter(chat_bot_subscriptions::is_reachable.eq(true))
        .load(conn)?;

    let mut sub_map: HashMap<i64, Vec<BotType>> = HashMap::new();
    for sub in subscriptions {
        if let Some(bot_type) = sub.bot_type_enum() {
            sub_map
                .entry(sub.telegram_chat_id)
                .or_default()
                .push(bot_type);
        }
    }

    let mut targets_map: HashMap<i64, BroadcastTarget> = HashMap::new();

    for (telegram_id, thread_id) in chat_results {
        if let Ok(chat_id) = telegram_id.parse::<i64>() {
            let known_bots = sub_map.get(&chat_id).cloned().unwrap_or_default();
            targets_map.insert(
                chat_id,
                BroadcastTarget {
                    telegram_chat_id: chat_id,
                    thread_id: thread_id.and_then(|t| t.parse().ok()),
                    known_working_bots: known_bots,
                },
            );
        }
    }

    for chat_id in agreement_results {
        targets_map
            .entry(chat_id)
            .and_modify(|t| {
                if !t.known_working_bots.contains(&BotType::Agreement) {
                    t.known_working_bots.push(BotType::Agreement);
                }
            })
            .or_insert(BroadcastTarget {
                telegram_chat_id: chat_id,
                thread_id: None,
                known_working_bots: vec![BotType::Agreement],
            });
    }

    let mut targets: Vec<BroadcastTarget> = targets_map.into_values().collect();
    targets.sort_by_key(|t| t.telegram_chat_id);

    Ok(targets)
}

pub fn upsert_chat_bot_subscription(
    pool: &DbPool,
    telegram_chat_id: i64,
    bot_type: BotType,
    success: bool,
) -> Result<(), DbError> {
    let conn = &mut pool.get()?;
    let now = Utc::now();
    let bot_type_str = bot_type.as_str();

    let existing: Option<ChatBotSubscription> = chat_bot_subscriptions::table
        .filter(chat_bot_subscriptions::telegram_chat_id.eq(telegram_chat_id))
        .filter(chat_bot_subscriptions::bot_type.eq(bot_type_str))
        .first(conn)
        .optional()?;

    if let Some(_sub) = existing {
        if success {
            diesel::update(
                chat_bot_subscriptions::table
                    .filter(chat_bot_subscriptions::telegram_chat_id.eq(telegram_chat_id))
                    .filter(chat_bot_subscriptions::bot_type.eq(bot_type_str)),
            )
            .set((
                chat_bot_subscriptions::is_reachable.eq(true),
                chat_bot_subscriptions::last_success_at.eq(Some(now)),
                chat_bot_subscriptions::failure_count.eq(0),
                chat_bot_subscriptions::updated_at.eq(now),
            ))
            .execute(conn)?;
        } else {
            diesel::update(
                chat_bot_subscriptions::table
                    .filter(chat_bot_subscriptions::telegram_chat_id.eq(telegram_chat_id))
                    .filter(chat_bot_subscriptions::bot_type.eq(bot_type_str)),
            )
            .set((
                chat_bot_subscriptions::last_failure_at.eq(Some(now)),
                chat_bot_subscriptions::failure_count.eq(chat_bot_subscriptions::failure_count + 1),
                chat_bot_subscriptions::updated_at.eq(now),
            ))
            .execute(conn)?;
        }
    } else {
        let new_sub = NewChatBotSubscription {
            telegram_chat_id,
            bot_type: bot_type_str,
            is_reachable: success,
        };
        diesel::insert_into(chat_bot_subscriptions::table)
            .values(&new_sub)
            .execute(conn)?;

        if success {
            diesel::update(
                chat_bot_subscriptions::table
                    .filter(chat_bot_subscriptions::telegram_chat_id.eq(telegram_chat_id))
                    .filter(chat_bot_subscriptions::bot_type.eq(bot_type_str)),
            )
            .set(chat_bot_subscriptions::last_success_at.eq(Some(now)))
            .execute(conn)?;
        } else {
            diesel::update(
                chat_bot_subscriptions::table
                    .filter(chat_bot_subscriptions::telegram_chat_id.eq(telegram_chat_id))
                    .filter(chat_bot_subscriptions::bot_type.eq(bot_type_str)),
            )
            .set((
                chat_bot_subscriptions::last_failure_at.eq(Some(now)),
                chat_bot_subscriptions::failure_count.eq(1),
            ))
            .execute(conn)?;
        }
    }

    Ok(())
}

pub fn create_pending_deactivation(
    pool: &DbPool,
    telegram_chat_id: i64,
    broadcast_job_id: Option<i32>,
    failed_bots: &[BotType],
    last_error: Option<&str>,
) -> Result<(), DbError> {
    let conn = &mut pool.get()?;

    let failed_bots_strings: Vec<Option<String>> = failed_bots
        .iter()
        .map(|b| Some(b.as_str().to_string()))
        .collect();

    let existing: Option<PendingDeactivation> = pending_deactivations::table
        .filter(pending_deactivations::telegram_chat_id.eq(telegram_chat_id))
        .first(conn)
        .optional()?;

    if existing.is_some() {
        diesel::update(
            pending_deactivations::table
                .filter(pending_deactivations::telegram_chat_id.eq(telegram_chat_id)),
        )
        .set((
            pending_deactivations::source_broadcast_job_id.eq(broadcast_job_id),
            pending_deactivations::failed_bots.eq(&failed_bots_strings),
            pending_deactivations::last_error.eq(last_error),
            pending_deactivations::status.eq(DeactivationStatus::Pending.as_str()),
        ))
        .execute(conn)?;
    } else {
        let new_deactivation = NewPendingDeactivation {
            telegram_chat_id,
            source_broadcast_job_id: broadcast_job_id,
            failed_bots: &failed_bots_strings,
            last_error,
            status: DeactivationStatus::Pending.as_str(),
        };
        diesel::insert_into(pending_deactivations::table)
            .values(&new_deactivation)
            .execute(conn)?;
    }

    Ok(())
}

pub fn get_pending_deactivations(pool: &DbPool) -> Result<Vec<PendingDeactivation>, DbError> {
    let conn = &mut pool.get()?;

    Ok(pending_deactivations::table
        .filter(pending_deactivations::status.eq(DeactivationStatus::Pending.as_str()))
        .order(pending_deactivations::created_at.asc())
        .load(conn)?)
}

pub fn approve_all_deactivations(pool: &DbPool, approved_by: i64) -> Result<usize, DbError> {
    let conn = &mut pool.get()?;

    conn.transaction(|conn| {
        let pending: Vec<PendingDeactivation> = pending_deactivations::table
            .filter(pending_deactivations::status.eq(DeactivationStatus::Pending.as_str()))
            .load(conn)?;

        let now = Utc::now();

        for deactivation in &pending {
            diesel::update(
                chats::table
                    .filter(chats::telegram_id.eq(deactivation.telegram_chat_id.to_string())),
            )
            .set((chats::is_active.eq(false), chats::deactivated_at.eq(now)))
            .execute(conn)?;
        }

        let count = diesel::update(
            pending_deactivations::table
                .filter(pending_deactivations::status.eq(DeactivationStatus::Pending.as_str())),
        )
        .set((
            pending_deactivations::status.eq(DeactivationStatus::Approved.as_str()),
            pending_deactivations::reviewed_at.eq(Some(now)),
            pending_deactivations::reviewed_by_chat_id.eq(Some(approved_by)),
        ))
        .execute(conn)?;

        Ok(count)
    })
}

pub fn reject_all_deactivations(pool: &DbPool, rejected_by: i64) -> Result<usize, DbError> {
    let conn = &mut pool.get()?;

    let count = diesel::update(
        pending_deactivations::table
            .filter(pending_deactivations::status.eq(DeactivationStatus::Pending.as_str())),
    )
    .set((
        pending_deactivations::status.eq(DeactivationStatus::Rejected.as_str()),
        pending_deactivations::reviewed_at.eq(Some(Utc::now())),
        pending_deactivations::reviewed_by_chat_id.eq(Some(rejected_by)),
    ))
    .execute(conn)?;

    Ok(count)
}

pub fn get_job_by_id(pool: &DbPool, job_id: i32) -> Result<Option<BroadcastJob>, DbError> {
    let conn = &mut pool.get()?;

    Ok(broadcast_jobs::table
        .filter(broadcast_jobs::id.eq(job_id))
        .first(conn)
        .optional()?)
}
