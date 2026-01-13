use chrono::{DateTime, Utc};
use diesel::prelude::*;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

pub mod db;
pub mod i18n;
pub mod models;
pub mod schema;

use self::models::*;
use db::{DbError, PgPool};

#[derive(Debug)]
pub struct WebhookInfo {
    pub webhook_url: String,
    pub is_new: bool,
}

pub fn create_webhook(
    pool: &PgPool,
    webhook_url: &str,
    name: &str,
    chat_id: i32,
) -> Result<Webhook, DbError> {
    use self::schema::webhooks;

    let conn = &mut pool.get()?;

    let new_webhook = NewWebhook {
        webhook_url,
        name,
        chat_id: Some(chat_id),
    };

    Ok(diesel::insert_into(webhooks::table)
        .values(&new_webhook)
        .get_result(conn)?)
}

pub struct WebhookGetOrCreateInput<'a> {
    pub telegram_chat_id: &'a str,
    pub telegram_thread_id: Option<&'a str>,
}

pub fn get_webhook_url_or_create(
    pool: &PgPool,
    input: WebhookGetOrCreateInput,
) -> Result<WebhookInfo, DbError> {
    let WebhookGetOrCreateInput {
        telegram_chat_id,
        telegram_thread_id,
    } = input;

    use self::schema::chats;

    let conn = &mut pool.get()?;

    let result: Option<Chat> = chats::dsl::chats
        .filter(chats::dsl::telegram_id.eq(telegram_chat_id.to_string()))
        .first::<Chat>(conn)
        .optional()?;

    if let Some(chat) = result {
        if let Some(thread_id) = telegram_thread_id {
            if let Some(ref c) = find_chat_by_id(pool, chat.id)? {
                if c.thread_id.is_none() {
                    update_chat_thread_id(pool, c, thread_id)?;
                }
            }
        }

        match find_webhook_by_chat_id(pool, chat.id)? {
            Some(webhook) => Ok(WebhookInfo {
                webhook_url: webhook.webhook_url,
                is_new: false,
            }),
            None => {
                let random_string = create_random_string();
                let new_webhook = create_webhook(pool, &random_string, "new_chat", chat.id)?;
                Ok(WebhookInfo {
                    webhook_url: new_webhook.webhook_url,
                    is_new: true,
                })
            }
        }
    } else {
        let random_string = create_random_string();
        let name = "new_chat";
        let new_chat = create_chat(
            pool,
            CreateChatInput {
                telegram_chat_id,
                name,
                webhook_url: Some(&random_string),
                telegram_thread_id,
                language: "en",
            },
        )?;
        let new_webhook = create_webhook(pool, &random_string, name, new_chat.id)?;

        Ok(WebhookInfo {
            webhook_url: new_webhook.webhook_url,
            is_new: true,
        })
    }
}

pub fn show_webhooks(pool: &PgPool) -> Result<Vec<Webhook>, DbError> {
    use schema::webhooks::dsl::*;

    let conn = &mut pool.get()?;

    Ok(webhooks.limit(5).load::<Webhook>(conn)?)
}

pub struct CreateChatInput<'a> {
    pub telegram_chat_id: &'a str,
    pub name: &'a str,
    pub webhook_url: Option<&'a str>,
    pub telegram_thread_id: Option<&'a str>,
    pub language: &'a str,
}

pub fn create_chat(pool: &PgPool, create_chat_input: CreateChatInput) -> Result<Chat, DbError> {
    let CreateChatInput {
        telegram_chat_id,
        name,
        webhook_url,
        telegram_thread_id,
        language,
    } = create_chat_input;

    use self::schema::chats::table;

    let conn = &mut pool.get()?;

    let new_chat = NewChat {
        telegram_id: telegram_chat_id,
        name,
        webhook_url,
        thread_id: telegram_thread_id,
        language,
    };

    Ok(diesel::insert_into(table)
        .values(&new_chat)
        .get_result(conn)?)
}

pub fn update_chat_thread_id(
    pool: &PgPool,
    chat: &Chat,
    telegram_thread_id: &str,
) -> Result<Chat, DbError> {
    use self::schema::chats::dsl::*;

    let conn = &mut pool.get()?;

    Ok(diesel::update(chat)
        .set(thread_id.eq(telegram_thread_id))
        .get_result::<Chat>(conn)?)
}

pub fn find_webhook_by_webhook_url(pool: &PgPool, url: &str) -> Result<Option<Webhook>, DbError> {
    use schema::webhooks::dsl::*;

    let conn = &mut pool.get()?;

    Ok(webhooks
        .filter(webhook_url.eq(url))
        .first::<Webhook>(conn)
        .optional()?)
}

pub fn find_chat_by_id(pool: &PgPool, chat_id: i32) -> Result<Option<Chat>, DbError> {
    use schema::chats::dsl::*;

    let conn = &mut pool.get()?;

    Ok(chats
        .filter(id.eq(chat_id))
        .first::<Chat>(conn)
        .optional()?)
}

pub fn find_chat_by_telegram_chat_id(
    pool: &PgPool,
    telegram_chat_id: &str,
) -> Result<Option<Chat>, DbError> {
    use schema::chats::dsl::*;

    let conn = &mut pool.get()?;

    Ok(chats
        .filter(telegram_id.eq(telegram_chat_id))
        .first::<Chat>(conn)
        .optional()?)
}

pub fn find_webhook_by_chat_id(pool: &PgPool, chat_id: i32) -> Result<Option<Webhook>, DbError> {
    use schema::webhooks;

    let conn = &mut pool.get()?;

    Ok(webhooks::dsl::webhooks
        .filter(webhooks::dsl::chat_id.eq(chat_id))
        .first::<Webhook>(conn)
        .optional()?)
}

pub fn create_health_url(
    pool: &PgPool,
    new_url: &str,
    chat_id: i32,
    status_code: i32,
) -> Result<HealthUrl, DbError> {
    use self::schema::health_urls;

    let conn = &mut pool.get()?;

    let new_health_endpoint = NewHealthUrl {
        url: new_url,
        status_code,
        chat_id,
    };

    Ok(diesel::insert_into(health_urls::table)
        .values(&new_health_endpoint)
        .get_result(conn)?)
}

fn create_random_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

pub fn find_chat_by_chat_id(pool: &PgPool, chat_id: i32) -> Result<Option<Chat>, DbError> {
    use schema::chats::dsl::*;

    let conn = &mut pool.get()?;

    Ok(chats
        .filter(id.eq(chat_id))
        .first::<Chat>(conn)
        .optional()?)
}

pub fn get_all_health_urls(pool: &PgPool) -> Result<Vec<HealthUrl>, DbError> {
    use self::schema::health_urls::dsl::*;

    let conn = &mut pool.get()?;

    Ok(health_urls.load::<HealthUrl>(conn)?)
}

pub fn update_health_url_status(
    pool: &PgPool,
    id_to_update: i32,
    new_status_code: i32,
) -> Result<HealthUrl, DbError> {
    use self::schema::health_urls::dsl::*;

    let conn = &mut pool.get()?;

    Ok(diesel::update(health_urls.filter(id.eq(id_to_update)))
        .set(status_code.eq(new_status_code))
        .get_result(conn)?)
}

pub fn get_health_url_by_chat_id_and_url(
    pool: &PgPool,
    chat_id_value: i64,
    url_value: &str,
) -> Result<Option<HealthUrl>, DbError> {
    use self::schema::health_urls::dsl::*;

    let conn = &mut pool.get()?;

    Ok(health_urls
        .filter(chat_id.eq(&(chat_id_value as i32)))
        .filter(url.eq(url_value))
        .first::<HealthUrl>(conn)
        .optional()?)
}

pub fn get_all_chats(pool: &PgPool) -> Result<Vec<Chat>, DbError> {
    use self::schema::chats::dsl::*;

    let conn = &mut pool.get()?;

    Ok(chats.load::<Chat>(conn)?)
}

pub fn get_health_urls_by_chat_id(
    pool: &PgPool,
    chat_id_value: i64,
) -> Result<Vec<HealthUrl>, DbError> {
    use self::schema::health_urls::dsl::*;

    let conn = &mut pool.get()?;

    Ok(health_urls
        .filter(chat_id.eq(chat_id_value as i32))
        .load::<HealthUrl>(conn)?)
}

pub fn delete_health_url_by_id(
    pool: &PgPool,
    health_url_id: i32,
    chat_id_value: i32,
) -> Result<bool, DbError> {
    use self::schema::health_urls::dsl::*;

    let conn = &mut pool.get()?;

    let deleted = diesel::delete(
        health_urls
            .filter(id.eq(health_url_id))
            .filter(chat_id.eq(chat_id_value)),
    )
    .execute(conn)?;

    Ok(deleted > 0)
}

pub fn deactivate_chat(pool: &PgPool, telegram_chat_id: &str) -> Result<Option<Chat>, DbError> {
    use self::schema::chats::dsl::*;

    let conn = &mut pool.get()?;

    let result = diesel::update(chats.filter(telegram_id.eq(telegram_chat_id)))
        .set((is_active.eq(false), deactivated_at.eq(Some(Utc::now()))))
        .get_result::<Chat>(conn)
        .optional()?;

    Ok(result)
}

pub fn reactivate_chat(pool: &PgPool, telegram_chat_id: &str) -> Result<Option<Chat>, DbError> {
    use self::schema::chats::dsl::*;

    let conn = &mut pool.get()?;

    let result = diesel::update(chats.filter(telegram_id.eq(telegram_chat_id)))
        .set((is_active.eq(true), deactivated_at.eq(None::<DateTime<Utc>>)))
        .get_result::<Chat>(conn)
        .optional()?;

    Ok(result)
}

pub fn get_active_chats_count(pool: &PgPool) -> Result<i64, DbError> {
    use self::schema::chats::dsl::*;

    let conn = &mut pool.get()?;

    Ok(chats.filter(is_active.eq(true)).count().get_result(conn)?)
}

pub fn get_chats_created_since(pool: &PgPool, since: DateTime<Utc>) -> Result<i64, DbError> {
    use self::schema::chats::dsl::*;

    let conn = &mut pool.get()?;

    Ok(chats
        .filter(created_at.ge(since))
        .count()
        .get_result(conn)?)
}

pub fn find_agreement_user_by_telegram_id(
    pool: &PgPool,
    user_telegram_id: i64,
) -> Result<Option<AgreementUser>, DbError> {
    use self::schema::agreement_users::dsl::*;

    let conn = &mut pool.get()?;

    Ok(agreement_users
        .filter(telegram_user_id.eq(user_telegram_id))
        .first::<AgreementUser>(conn)
        .optional()?)
}

pub fn create_agreement_user(
    pool: &PgPool,
    new_user: NewAgreementUser,
) -> Result<AgreementUser, DbError> {
    use self::schema::agreement_users;

    let conn = &mut pool.get()?;

    Ok(diesel::insert_into(agreement_users::table)
        .values(&new_user)
        .get_result(conn)?)
}

pub fn update_agreement_user_language(
    pool: &PgPool,
    user_telegram_id: i64,
    new_language: &str,
) -> Result<AgreementUser, DbError> {
    use self::schema::agreement_users::dsl::*;

    let conn = &mut pool.get()?;

    Ok(
        diesel::update(agreement_users.filter(telegram_user_id.eq(user_telegram_id)))
            .set((language.eq(new_language), updated_at.eq(Utc::now())))
            .get_result(conn)?,
    )
}

pub fn update_agreement_user_timezone(
    pool: &PgPool,
    user_telegram_id: i64,
    new_timezone: &str,
) -> Result<AgreementUser, DbError> {
    use self::schema::agreement_users::dsl::*;

    let conn = &mut pool.get()?;

    Ok(
        diesel::update(agreement_users.filter(telegram_user_id.eq(user_telegram_id)))
            .set((timezone.eq(new_timezone), updated_at.eq(Utc::now())))
            .get_result(conn)?,
    )
}

pub fn accept_disclaimer(pool: &PgPool, user_telegram_id: i64) -> Result<AgreementUser, DbError> {
    use self::schema::agreement_users::dsl::*;

    let conn = &mut pool.get()?;

    Ok(
        diesel::update(agreement_users.filter(telegram_user_id.eq(user_telegram_id)))
            .set((
                disclaimer_accepted.eq(true),
                disclaimer_accepted_at.eq(Some(Utc::now())),
                updated_at.eq(Utc::now()),
            ))
            .get_result(conn)?,
    )
}

pub fn get_conversation_state(
    pool: &PgPool,
    user_telegram_id: i64,
) -> Result<Option<AgreementConversationState>, DbError> {
    use self::schema::agreement_conversation_states::dsl::*;

    let conn = &mut pool.get()?;

    Ok(agreement_conversation_states
        .filter(telegram_user_id.eq(user_telegram_id))
        .filter(expires_at.gt(Utc::now()))
        .first::<AgreementConversationState>(conn)
        .optional()?)
}

pub fn set_conversation_state(
    pool: &PgPool,
    user_telegram_id: i64,
    new_state: &str,
    new_state_data: Option<serde_json::Value>,
    new_expires_at: DateTime<Utc>,
) -> Result<AgreementConversationState, DbError> {
    use self::schema::agreement_conversation_states::dsl::*;

    let conn = &mut pool.get()?;

    let existing = agreement_conversation_states
        .filter(telegram_user_id.eq(user_telegram_id))
        .first::<AgreementConversationState>(conn)
        .optional()?;

    if existing.is_some() {
        Ok(diesel::update(
            agreement_conversation_states.filter(telegram_user_id.eq(user_telegram_id)),
        )
        .set((
            state.eq(new_state),
            state_data.eq(new_state_data),
            expires_at.eq(new_expires_at),
            updated_at.eq(Utc::now()),
        ))
        .get_result(conn)?)
    } else {
        let new_conversation_state = NewAgreementConversationState {
            telegram_user_id: user_telegram_id,
            state: new_state,
            state_data: new_state_data,
            expires_at: new_expires_at,
        };

        Ok(diesel::insert_into(agreement_conversation_states)
            .values(&new_conversation_state)
            .get_result(conn)?)
    }
}

pub fn clear_conversation_state(pool: &PgPool, user_telegram_id: i64) -> Result<bool, DbError> {
    use self::schema::agreement_conversation_states::dsl::*;

    let conn = &mut pool.get()?;

    let deleted =
        diesel::delete(agreement_conversation_states.filter(telegram_user_id.eq(user_telegram_id)))
            .execute(conn)?;

    Ok(deleted > 0)
}

pub fn create_agreement(pool: &PgPool, new_agreement: NewAgreement) -> Result<Agreement, DbError> {
    use self::schema::agreements;

    let conn = &mut pool.get()?;

    Ok(diesel::insert_into(agreements::table)
        .values(&new_agreement)
        .get_result(conn)?)
}

pub fn find_agreement_by_id(
    pool: &PgPool,
    agreement_id: i32,
) -> Result<Option<Agreement>, DbError> {
    use self::schema::agreements::dsl::*;

    let conn = &mut pool.get()?;

    Ok(agreements
        .filter(id.eq(agreement_id))
        .first::<Agreement>(conn)
        .optional()?)
}

pub fn find_agreements_by_user_id(
    pool: &PgPool,
    user_id_val: i32,
) -> Result<Vec<Agreement>, DbError> {
    use self::schema::agreements::dsl::*;

    let conn = &mut pool.get()?;

    Ok(agreements
        .filter(user_id.eq(user_id_val))
        .load::<Agreement>(conn)?)
}

pub fn find_agreement_by_user_and_title(
    pool: &PgPool,
    user_id_val: i32,
    title_val: &str,
) -> Result<Option<Agreement>, DbError> {
    use self::schema::agreements::dsl::*;

    let conn = &mut pool.get()?;

    Ok(agreements
        .filter(user_id.eq(user_id_val))
        .filter(title.eq(title_val))
        .first::<Agreement>(conn)
        .optional()?)
}

pub fn delete_agreement(
    pool: &PgPool,
    agreement_id: i32,
    owner_user_id: i32,
) -> Result<bool, DbError> {
    use self::schema::agreements::dsl::*;

    let conn = &mut pool.get()?;

    let deleted = diesel::delete(
        agreements
            .filter(id.eq(agreement_id))
            .filter(user_id.eq(owner_user_id)),
    )
    .execute(conn)?;

    Ok(deleted > 0)
}

pub fn update_agreement(
    pool: &PgPool,
    agreement_id: i32,
    owner_user_id: i32,
    updates: UpdateAgreement,
) -> Result<Agreement, DbError> {
    use self::schema::agreements::dsl::*;

    let conn = &mut pool.get()?;

    Ok(diesel::update(
        agreements
            .filter(id.eq(agreement_id))
            .filter(user_id.eq(owner_user_id)),
    )
    .set((&updates, updated_at.eq(Utc::now())))
    .get_result(conn)?)
}

pub fn create_reminder(pool: &PgPool, new_reminder: NewReminder) -> Result<Reminder, DbError> {
    use self::schema::reminders;

    let conn = &mut pool.get()?;

    Ok(diesel::insert_into(reminders::table)
        .values(&new_reminder)
        .get_result(conn)?)
}

pub fn create_reminders_batch(
    pool: &PgPool,
    new_reminders: Vec<NewReminder>,
) -> Result<Vec<Reminder>, DbError> {
    use self::schema::reminders;

    let conn = &mut pool.get()?;

    Ok(diesel::insert_into(reminders::table)
        .values(&new_reminders)
        .get_results(conn)?)
}

pub fn find_reminders_by_agreement_id(
    pool: &PgPool,
    agreement_id_val: i32,
) -> Result<Vec<Reminder>, DbError> {
    use self::schema::reminders::dsl::*;

    let conn = &mut pool.get()?;

    Ok(reminders
        .filter(agreement_id.eq(agreement_id_val))
        .order(reminder_date.asc())
        .load::<Reminder>(conn)?)
}

pub fn find_reminder_by_id(pool: &PgPool, reminder_id: i32) -> Result<Option<Reminder>, DbError> {
    use self::schema::reminders::dsl::*;

    let conn = &mut pool.get()?;

    Ok(reminders
        .filter(id.eq(reminder_id))
        .first::<Reminder>(conn)
        .optional()?)
}

pub fn find_pending_reminders_by_date(
    pool: &PgPool,
    target_date: chrono::NaiveDate,
) -> Result<Vec<Reminder>, DbError> {
    use self::schema::reminders::dsl::*;

    let conn = &mut pool.get()?;

    Ok(reminders
        .filter(status.eq("pending"))
        .filter(reminder_date.eq(target_date))
        .load::<Reminder>(conn)?)
}

pub fn update_reminder_status(
    pool: &PgPool,
    reminder_id: i32,
    new_status: &str,
) -> Result<Reminder, DbError> {
    use self::schema::reminders::dsl::*;

    let conn = &mut pool.get()?;

    let now = Utc::now();
    let completed = if new_status == "done" {
        Some(now)
    } else {
        None
    };
    let sent = if new_status == "sent" {
        Some(now)
    } else {
        None
    };

    Ok(diesel::update(reminders.filter(id.eq(reminder_id)))
        .set((
            status.eq(new_status),
            completed_at.eq(completed),
            sent_at.eq(sent),
            updated_at.eq(now),
        ))
        .get_result(conn)?)
}

pub fn update_reminder_snooze(
    pool: &PgPool,
    reminder_id: i32,
    snoozed_until_val: DateTime<Utc>,
) -> Result<Reminder, DbError> {
    use self::schema::reminders::dsl::*;

    let conn = &mut pool.get()?;

    Ok(diesel::update(reminders.filter(id.eq(reminder_id)))
        .set((
            snooze_count.eq(snooze_count + 1),
            snoozed_until.eq(Some(snoozed_until_val)),
            updated_at.eq(Utc::now()),
        ))
        .get_result(conn)?)
}

pub fn delete_reminders_by_agreement_id(
    pool: &PgPool,
    agreement_id_val: i32,
) -> Result<usize, DbError> {
    use self::schema::reminders::dsl::*;

    let conn = &mut pool.get()?;

    Ok(diesel::delete(reminders.filter(agreement_id.eq(agreement_id_val))).execute(conn)?)
}

#[derive(Debug)]
pub struct DueReminderWithUserInfo {
    pub reminder: Reminder,
    pub agreement: Agreement,
    pub user: AgreementUser,
}

pub fn find_due_reminders_with_user_info(
    pool: &PgPool,
    target_date: chrono::NaiveDate,
) -> Result<Vec<DueReminderWithUserInfo>, DbError> {
    use self::schema::{agreement_users, agreements, reminders};

    let conn = &mut pool.get()?;

    let results: Vec<(Reminder, Agreement, AgreementUser)> = reminders::table
        .inner_join(agreements::table.on(reminders::agreement_id.eq(agreements::id)))
        .inner_join(agreement_users::table.on(agreements::user_id.eq(agreement_users::id)))
        .filter(reminders::status.eq("pending"))
        .filter(reminders::reminder_date.le(target_date))
        .filter(
            reminders::snoozed_until
                .is_null()
                .or(reminders::snoozed_until.le(Utc::now())),
        )
        .order(reminders::reminder_date.asc())
        .limit(100)
        .select((
            Reminder::as_select(),
            Agreement::as_select(),
            AgreementUser::as_select(),
        ))
        .load(conn)?;

    Ok(results
        .into_iter()
        .map(|(reminder, agreement, user)| DueReminderWithUserInfo {
            reminder,
            agreement,
            user,
        })
        .collect())
}
