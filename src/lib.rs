use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::env;

pub mod models;
pub mod schema;

use self::models::*;

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub fn create_webhook(webhook_url: &str, name: &str, chat_id: i32) -> Webhook {
    use self::schema::webhooks;

    let conn = &mut establish_connection();

    let new_webhook = NewWebhook {
        webhook_url,
        name,
        chat_id: Some(chat_id),
    };

    diesel::insert_into(webhooks::table)
        .values(&new_webhook)
        .get_result(conn)
        .expect("Error saving new webhook")
}

pub fn get_webhook_url_or_create(telegram_chat_id: i32) -> String {
    // find webhook by chat_id or create new one
    use self::schema::chats;

    let conn = &mut establish_connection();

    let result: Option<Chat> = chats::dsl::chats
        .filter(chats::dsl::telegram_id.eq(telegram_chat_id.to_string()))
        .first::<Chat>(conn)
        .optional()
        .expect("Error loading webhooks");

    if let Some(chat) = result {
        let webhook = find_webhook_by_chat_id(chat.id);
        webhook.expect("Error loading webhook").webhook_url
    } else {
        let random_string = create_random_string();
        let name = "new_chat";
        let new_chat = create_chat(&telegram_chat_id.to_string(), name, &random_string);
        let new_webhook = create_webhook(&random_string, name, new_chat.id);

        new_webhook.webhook_url
    }
}

pub fn show_webhooks() -> Vec<Webhook> {
    use schema::webhooks::dsl::*;

    let conn = &mut establish_connection();

    webhooks
        .limit(5)
        .load::<Webhook>(conn)
        .expect("Error loading webhooks")
}

pub fn create_chat(telegram_chat_id: &str, name: &str, webhook_url: &str) -> Chat {
    use self::schema::chats;

    let conn = &mut establish_connection();

    let new_chat = NewChat {
        telegram_id: telegram_chat_id,
        name,
        webhook_url,
    };

    diesel::insert_into(chats::table)
        .values(&new_chat)
        .get_result(conn)
        .expect("Error saving new chat")
}

pub fn find_webhook_by_webhook_url(url: &str) -> Option<Webhook> {
    use schema::webhooks::dsl::*;

    let conn = &mut establish_connection();

    webhooks
        .filter(webhook_url.eq(url))
        .first::<Webhook>(conn)
        .optional()
        .expect("Error loading webhook")
}

pub fn find_chat_by_id(chat_id: i32) -> Option<Chat> {
    use schema::chats::dsl::*;

    let conn = &mut establish_connection();

    chats
        .filter(id.eq(chat_id))
        .first::<Chat>(conn)
        .optional()
        .expect("Error loading chat")
}

pub fn find_webhook_by_chat_id(chat_id: i32) -> Option<Webhook> {
    use schema::webhooks;

    let conn = &mut establish_connection();

    webhooks::dsl::webhooks
        .filter(webhooks::dsl::chat_id.eq(chat_id))
        .first::<Webhook>(conn)
        .optional()
        .expect("Error loading webhook")
}

pub fn create_trello_token(
    new_token_key: &str,
    new_token_secret: &str,
    new_telegram_user_id: &str,
) -> TrelloToken {
    use self::schema::trello_tokens;

    let conn = &mut establish_connection();

    let new_trello_token = NewTrelloToken {
        token_key: Some(new_token_key),
        token_secret: Some(new_token_secret),
        access_token: None,
        access_token_secret: None,
        telegram_user_id: Some(new_telegram_user_id),
    };

    diesel::insert_into(trello_tokens::table)
        .values(&new_trello_token)
        .get_result(conn)
        .expect("Error saving new trello token")
}

pub fn find_trello_token_by_token_key(token_key: &str) -> Option<TrelloToken> {
    use schema::trello_tokens;

    let conn = &mut establish_connection();

    trello_tokens::dsl::trello_tokens
        .filter(trello_tokens::dsl::token_key.eq(token_key))
        .first::<TrelloToken>(conn)
        .optional()
        .expect("Error loading trello token")
}

pub fn find_trello_token_by_telegram_user_id(telegram_user_id: &str) -> Option<TrelloToken> {
    use schema::trello_tokens;

    let conn = &mut establish_connection();

    trello_tokens::dsl::trello_tokens
        .filter(trello_tokens::dsl::telegram_user_id.eq(telegram_user_id))
        .first::<TrelloToken>(conn)
        .optional()
        .expect("Error loading trello token")
}

pub fn update_trello_token_access_token(
    trello_token: &TrelloToken,
    access_token: &str,
    access_token_secret: &str,
) -> TrelloToken {
    use self::schema::trello_tokens;

    let conn = &mut establish_connection();

    diesel::update(trello_token)
        .set((
            trello_tokens::access_token.eq(access_token),
            trello_tokens::access_token_secret.eq(access_token_secret),
        ))
        .get_result(conn)
        .expect("Error updating trello token")
}

fn create_random_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}
