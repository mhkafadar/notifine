use std::env;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;

pub mod models;
pub mod schema;

use self::models::*;


pub fn establish_connection() -> PgConnection {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub fn create_webhook(webhook_url: &str, name: &str) -> Webhook {
    use self::schema::webhooks;

    let conn = &mut establish_connection();

    let new_webhook = NewWebhook {
        webhook_url,
        name,
    };

    diesel::insert_into(webhooks::table)
        .values(&new_webhook)
        .get_result(conn)
        .expect("Error saving new webhook")
}

pub fn show_webhooks() -> Vec<Webhook> {
    use schema::webhooks::dsl::*;

    let conn = &mut establish_connection();

    webhooks
        .limit(5)
        .load::<Webhook>(conn)
        .expect("Error loading webhooks")
}
