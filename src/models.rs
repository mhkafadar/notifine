use crate::schema::{
    agreement_conversation_states, agreement_users, agreements, chats, daily_stats, health_urls,
    reminders, trello_tokens, webhooks,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::pg::data_types::PgTimestamp;
use diesel::prelude::*;

#[derive(Queryable, Identifiable, Selectable)]
#[diesel(table_name = crate::schema::chats)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Chat {
    pub id: i32,
    pub name: String,
    pub telegram_id: String,
    pub webhook_url: Option<String>,
    pub thread_id: Option<String>,
    pub language: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_active: bool,
    pub deactivated_at: Option<DateTime<Utc>>,
}

#[derive(Insertable)]
#[diesel(table_name = chats)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewChat<'a> {
    pub name: &'a str,
    pub telegram_id: &'a str,
    pub webhook_url: Option<&'a str>,
    pub thread_id: Option<&'a str>,
    pub language: &'a str,
}

#[derive(Queryable, Associations, Identifiable)]
#[diesel(belongs_to(Chat))]
#[diesel(table_name = webhooks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Webhook {
    pub id: i32,
    pub name: String,
    pub webhook_url: String,
    pub created_at: PgTimestamp,
    pub updated_at: PgTimestamp,
    pub chat_id: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = webhooks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWebhook<'a> {
    pub name: &'a str,
    pub webhook_url: &'a str,
    pub chat_id: Option<i32>,
}

#[derive(Queryable, Identifiable)]
#[diesel(table_name = trello_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TrelloToken {
    pub id: i32,
    pub access_token: Option<String>,
    pub access_token_secret: Option<String>,
    pub token_key: Option<String>,
    pub token_secret: Option<String>,
    pub telegram_user_id: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = trello_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewTrelloToken<'a> {
    pub access_token: Option<&'a str>,
    pub access_token_secret: Option<&'a str>,
    pub token_key: Option<&'a str>,
    pub token_secret: Option<&'a str>,
    pub telegram_user_id: Option<&'a str>,
}

#[derive(Debug, Queryable, Identifiable)]
#[diesel(table_name = health_urls)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct HealthUrl {
    pub id: i32,
    pub url: String,
    pub chat_id: i32,
    pub status_code: i32,
    pub created_at: PgTimestamp,
    pub updated_at: PgTimestamp,
}

#[derive(Insertable)]
#[diesel(table_name = health_urls)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewHealthUrl<'a> {
    pub url: &'a str,
    pub chat_id: i32,
    pub status_code: i32,
}

#[derive(Debug, Queryable, Identifiable)]
#[diesel(table_name = daily_stats)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DailyStats {
    pub id: i32,
    pub date: NaiveDate,
    pub messages_sent: i32,
    pub webhooks_received: i32,
    pub github_webhooks: i32,
    pub gitlab_webhooks: i32,
    pub beep_webhooks: i32,
    pub new_chats: i32,
    pub churned_chats: i32,
    pub uptime_checks: i32,
    pub uptime_failures: i32,
    pub errors_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = daily_stats)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDailyStats {
    pub date: NaiveDate,
    pub messages_sent: i32,
    pub webhooks_received: i32,
    pub github_webhooks: i32,
    pub gitlab_webhooks: i32,
    pub beep_webhooks: i32,
    pub new_chats: i32,
    pub churned_chats: i32,
    pub uptime_checks: i32,
    pub uptime_failures: i32,
    pub errors_count: i32,
}

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = agreement_users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AgreementUser {
    pub id: i32,
    pub telegram_user_id: i64,
    pub telegram_chat_id: i64,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub language: String,
    pub timezone: String,
    pub disclaimer_accepted: bool,
    pub disclaimer_accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = agreement_users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAgreementUser<'a> {
    pub telegram_user_id: i64,
    pub telegram_chat_id: i64,
    pub username: Option<&'a str>,
    pub first_name: Option<&'a str>,
    pub last_name: Option<&'a str>,
    pub language: &'a str,
    pub timezone: &'a str,
}

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = agreement_conversation_states)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AgreementConversationState {
    pub id: i32,
    pub telegram_user_id: i64,
    pub state: String,
    pub state_data: Option<serde_json::Value>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = agreement_conversation_states)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAgreementConversationState<'a> {
    pub telegram_user_id: i64,
    pub state: &'a str,
    pub state_data: Option<serde_json::Value>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Identifiable, Selectable, Associations)]
#[diesel(table_name = agreements)]
#[diesel(belongs_to(AgreementUser, foreign_key = user_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Agreement {
    pub id: i32,
    pub user_id: i32,
    pub agreement_type: String,
    pub title: String,
    pub user_role: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub currency: String,
    pub rent_amount: Option<BigDecimal>,
    pub due_day: Option<i32>,
    pub has_monthly_reminder: bool,
    pub reminder_timing: Option<String>,
    pub reminder_days_before: Option<i32>,
    pub has_yearly_increase_reminder: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = agreements)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAgreement<'a> {
    pub user_id: i32,
    pub agreement_type: &'a str,
    pub title: &'a str,
    pub user_role: Option<&'a str>,
    pub start_date: Option<NaiveDate>,
    pub currency: &'a str,
    pub rent_amount: Option<BigDecimal>,
    pub due_day: Option<i32>,
    pub has_monthly_reminder: bool,
    pub reminder_timing: Option<&'a str>,
    pub reminder_days_before: Option<i32>,
    pub has_yearly_increase_reminder: bool,
    pub description: Option<&'a str>,
}

#[derive(AsChangeset, Default)]
#[diesel(table_name = agreements)]
pub struct UpdateAgreement {
    pub title: Option<String>,
    pub rent_amount: Option<Option<BigDecimal>>,
    pub due_day: Option<Option<i32>>,
    pub has_monthly_reminder: Option<bool>,
    pub reminder_timing: Option<Option<String>>,
    pub reminder_days_before: Option<Option<i32>>,
    pub has_yearly_increase_reminder: Option<bool>,
    pub description: Option<Option<String>>,
}

#[derive(Debug, Queryable, Identifiable, Selectable, Associations)]
#[diesel(table_name = reminders)]
#[diesel(belongs_to(Agreement))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Reminder {
    pub id: i32,
    pub agreement_id: i32,
    pub reminder_type: String,
    pub title: String,
    pub amount: Option<BigDecimal>,
    pub due_date: NaiveDate,
    pub reminder_date: NaiveDate,
    pub status: String,
    pub snooze_count: i32,
    pub snoozed_until: Option<DateTime<Utc>>,
    pub sent_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = reminders)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewReminder {
    pub agreement_id: i32,
    pub reminder_type: String,
    pub title: String,
    pub amount: Option<BigDecimal>,
    pub due_date: NaiveDate,
    pub reminder_date: NaiveDate,
}
