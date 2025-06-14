use crate::schema::{chats, health_urls, tesla_auth, tesla_orders, trello_tokens, webhooks};
use diesel::data_types::PgTimestamp;
use diesel::prelude::*;

#[derive(Queryable, Identifiable, Selectable)]
#[diesel(table_name = crate::schema::chats)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Chat {
    pub id: i32,
    pub name: String,
    pub telegram_id: String,
    pub webhook_url: String,
    pub thread_id: Option<String>,
    pub language: String,
}

#[derive(Insertable)]
#[diesel(table_name = chats)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewChat<'a> {
    pub name: &'a str,
    pub telegram_id: &'a str,
    pub webhook_url: &'a str,
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
#[diesel(table_name = tesla_auth)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TeslaAuth {
    pub id: i32,
    pub chat_id: i64,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub token_type: String,
    pub created_at: PgTimestamp,
    pub updated_at: PgTimestamp,
    pub monitoring_enabled: bool,
}

#[derive(Insertable)]
#[diesel(table_name = tesla_auth)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewTeslaAuth<'a> {
    pub chat_id: i64,
    pub access_token: &'a str,
    pub refresh_token: &'a str,
    pub expires_in: i64,
    pub token_type: &'a str,
}

#[derive(Debug, Queryable, Identifiable, Associations)]
#[diesel(belongs_to(TeslaAuth, foreign_key = chat_id))]
#[diesel(table_name = tesla_orders)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TeslaOrder {
    pub id: i32,
    pub chat_id: i64,
    pub order_data: serde_json::Value,
    pub created_at: PgTimestamp,
    pub updated_at: PgTimestamp,
}

#[derive(Insertable)]
#[diesel(table_name = tesla_orders)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewTeslaOrder {
    pub chat_id: i64,
    pub order_data: serde_json::Value,
}
