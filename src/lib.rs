use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::env;

pub mod models;
pub mod schema;

use self::models::*;

#[derive(Debug)]
pub struct WebhookInfo {
    pub webhook_url: String,
    pub is_new: bool,
}

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

pub struct WebhookGetOrCreateInput<'a> {
    pub telegram_chat_id: &'a str,
    pub telegram_thread_id: Option<&'a str>,
}

pub fn get_webhook_url_or_create(input: WebhookGetOrCreateInput) -> WebhookInfo {
    let WebhookGetOrCreateInput {
        telegram_chat_id,
        telegram_thread_id,
    } = input;

    use self::schema::chats;

    let conn = &mut establish_connection();

    let result: Option<Chat> = chats::dsl::chats
        .filter(chats::dsl::telegram_id.eq(telegram_chat_id.to_string()))
        .first::<Chat>(conn)
        .optional()
        .expect("Error loading webhooks");

    if let Some(chat) = result {
        let webhook = find_webhook_by_chat_id(chat.id);

        if telegram_thread_id.is_some() {
            let chat = find_chat_by_id(chat.id).expect("Error loading chat");
            if chat.thread_id.is_none() {
                update_chat_thread_id(&chat, telegram_thread_id.unwrap());
            }
        }

        WebhookInfo {
            webhook_url: webhook.expect("Error loading webhook").webhook_url,
            is_new: false,
        }
    } else {
        let random_string = create_random_string();
        let name = "new_chat";
        let new_chat = create_chat(CreateChatInput {
            telegram_chat_id,
            name,
            webhook_url: &random_string,
            telegram_thread_id,
        });
        let new_webhook = create_webhook(&random_string, name, new_chat.id);

        WebhookInfo {
            webhook_url: new_webhook.webhook_url,
            is_new: true,
        }
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

pub struct CreateChatInput<'a> {
    pub telegram_chat_id: &'a str,
    pub name: &'a str,
    pub webhook_url: &'a str,
    pub telegram_thread_id: Option<&'a str>,
}

pub fn create_chat(create_chat_input: CreateChatInput) -> Chat {
    let CreateChatInput {
        telegram_chat_id,
        name,
        webhook_url,
        telegram_thread_id,
    } = create_chat_input;

    use self::schema::chats::table;

    let conn = &mut establish_connection();

    let new_chat = NewChat {
        telegram_id: telegram_chat_id,
        name,
        webhook_url,
        thread_id: telegram_thread_id,
    };

    diesel::insert_into(table)
        .values(&new_chat)
        .get_result(conn)
        .expect("Error saving new chat")
}

pub fn update_chat_thread_id(chat: &Chat, telegram_thread_id: &str) -> Chat {
    use self::schema::chats::dsl::*;

    let conn = &mut establish_connection();

    diesel::update(chat)
        // .filter(id.eq(chat.id))
        .set(thread_id.eq(telegram_thread_id))
        .get_result::<Chat>(conn)
        .expect("Error updating chat")
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

pub fn find_chat_by_telegram_chat_id(telegram_chat_id: &str) -> Option<Chat> {
    use schema::chats::dsl::*;

    let conn = &mut establish_connection();

    chats
        .filter(telegram_id.eq(telegram_chat_id))
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

pub fn create_health_url(new_url: &str, chat_id: i32, status_code: i32) -> HealthUrl {
    use self::schema::health_urls;

    let conn = &mut establish_connection();

    let new_health_endpoint = NewHealthUrl {
        url: new_url,
        status_code,
        chat_id,
    };

    diesel::insert_into(health_urls::table)
        .values(&new_health_endpoint)
        .get_result(conn)
        .expect("Error saving new health endpoint")
}

fn create_random_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

pub fn find_chat_by_chat_id(chat_id: i32) -> Option<Chat> {
    use schema::chats::dsl::*;

    let conn = &mut establish_connection();

    chats
        .filter(id.eq(chat_id))
        .first::<Chat>(conn)
        .optional()
        .expect("Error loading chat")
}

pub fn get_all_health_urls() -> Vec<HealthUrl> {
    use self::schema::health_urls::dsl::*;

    let conn = &mut establish_connection();

    health_urls
        .load::<HealthUrl>(conn)
        .expect("Error loading health URLs")
}

pub fn update_health_url_status(id_to_update: i32, new_status_code: i32) -> HealthUrl {
    use self::schema::health_urls::dsl::*;

    let conn = &mut establish_connection();

    diesel::update(health_urls.filter(id.eq(id_to_update)))
        .set(status_code.eq(new_status_code))
        .get_result(conn)
        .expect("Error updating health URL status")
}

pub fn get_health_url_by_chat_id_and_url(chat_id_value: i64, url_value: &str) -> Option<HealthUrl> {
    use self::schema::health_urls::dsl::*;

    let conn = &mut establish_connection();

    health_urls
        .filter(chat_id.eq(&(chat_id_value as i32)))
        .filter(url.eq(url_value))
        .first::<HealthUrl>(conn)
        .optional()
        .expect("Error loading health URL")
}

pub fn get_all_chats() -> Vec<Chat> {
    use self::schema::chats::dsl::*;

    let conn = &mut establish_connection();

    chats.load::<Chat>(conn).expect("Error loading chats")
}
