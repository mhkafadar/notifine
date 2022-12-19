use crate::schema::{chats, trello_tokens, webhooks};
use diesel::data_types::PgTimestamp;
use diesel::prelude::*;

#[derive(Queryable, Identifiable)]
pub struct Chat {
    pub id: i32,
    pub name: String,
    pub telegram_id: String,
    pub webhook_url: String,
}

#[derive(Insertable)]
#[diesel(table_name = chats)]
pub struct NewChat<'a> {
    pub name: &'a str,
    pub telegram_id: &'a str,
    pub webhook_url: &'a str,
}

#[derive(Queryable, Associations, Identifiable)]
#[belongs_to(Chat)]
#[diesel(table_name = webhooks)]
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
pub struct NewWebhook<'a> {
    pub name: &'a str,
    pub webhook_url: &'a str,
    pub chat_id: Option<i32>,
}

#[derive(Queryable, Identifiable)]
#[diesel(table_name = trello_tokens)]
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
pub struct NewTrelloToken<'a> {
    pub access_token: Option<&'a str>,
    pub access_token_secret: Option<&'a str>,
    pub token_key: Option<&'a str>,
    pub token_secret: Option<&'a str>,
    pub telegram_user_id: Option<&'a str>,
}
