// use crate::bots::gitlab_bot::run_gitlab_bot;
use crate::http_server::run_http_server;

use dotenv::dotenv;
use std::env;
use tokio::task;

pub mod bots;
pub mod http_server;
pub mod utils;
pub mod webhooks;

use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::PgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use crate::bots::bot_service::{BotConfig, BotService};

pub type PgPool = Pool<ConnectionManager<PgConnection>>;
pub type PgPooledConnection = PooledConnection<ConnectionManager<PgConnection>>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    // Make sure the database is up-to-date (create if it doesn't exist, or run the migrations)
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = diesel::r2d2::Pool::new(manager).unwrap();
    let mut connection = pool.get().unwrap();
    connection.run_pending_migrations(MIGRATIONS).expect("Migrations failed");

    // task::spawn(run_gitlab_bot());
    task::spawn(BotService::new(
        BotConfig {
            bot_name: "Gitlab".to_string(),
            token: env::var("GITLAB_TELOXIDE_TOKEN").expect("GITLAB_TELOXIDE_TOKEN must be set"),
        }
    ).run_bot());
    task::spawn(BotService::new(
        BotConfig {
            bot_name: "Github".to_string(),
            token: env::var("GITHUB_TELOXIDE_TOKEN").expect("GITHUB_TELOXIDE_TOKEN must be set"),
        }
    ).run_bot());
    task::spawn(BotService::new(
        BotConfig {
            bot_name: "Beep".to_string(),
            token: env::var("BEEP_TELOXIDE_TOKEN").expect("BEEP_TELOXIDE_TOKEN must be set"),
        }
    ).run_bot());
    // task::spawn(run_github_bot());
    // task::spawn(run_beep_bot());
    // task::spawn(run_trello_bot());
    run_http_server().await.expect("Http server error");

    log::info!("Main");
}
