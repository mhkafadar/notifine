use crate::{
    http_server::run_http_server,
    services::{tesla_monitor::start_tesla_monitoring, uptime_checker::run_uptime_checker},
};

use dotenv::dotenv;
use std::env;
use tokio::task;

pub mod bots;
pub mod http_server;
pub mod services;
pub mod utils;
pub mod webhooks;

use crate::bots::bot_service::{BotConfig, BotService};
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::PgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use teloxide::Bot;

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
    connection
        .run_pending_migrations(MIGRATIONS)
        .expect("Migrations failed");

    task::spawn(
        BotService::new(BotConfig {
            bot_name: "Gitlab".to_string(),
            token: env::var("GITLAB_TELOXIDE_TOKEN").expect("GITLAB_TELOXIDE_TOKEN must be set"),
        })
        .run_bot(),
    );
    task::spawn(
        BotService::new(BotConfig {
            bot_name: "Github".to_string(),
            token: env::var("GITHUB_TELOXIDE_TOKEN").expect("GITHUB_TELOXIDE_TOKEN must be set"),
        })
        .run_bot(),
    );
    task::spawn(
        BotService::new(BotConfig {
            bot_name: "Beep".to_string(),
            token: env::var("BEEP_TELOXIDE_TOKEN").expect("BEEP_TELOXIDE_TOKEN must be set"),
        })
        .run_bot(),
    );

    task::spawn(bots::uptime_bot::run_bot());

    // Only spawn Tesla bot if token is available
    if let Ok(tesla_token) = env::var("TESLA_TELOXIDE_TOKEN") {
        task::spawn(async move {
            let bot = Bot::new(tesla_token);
            bots::tesla_bot::run_tesla_bot(bot).await;
        });
    }

    task::spawn(async {
        run_uptime_checker().await;
    });

    task::spawn(async {
        log::info!("Starting Tesla monitoring service");
        if let Err(e) = start_tesla_monitoring().await {
            log::error!("Tesla monitoring service error: {}", e);
        }
    });

    // task::spawn(run_github_bot());
    // task::spawn(run_beep_bot());
    // task::spawn(run_trello_bot());
    run_http_server().await.expect("Http server error");

    log::info!("Main");
}
