use crate::observability::startup::{
    alert_database_error, alert_http_server_error, alert_migration_error, alert_startup_success,
};
use crate::services::reminder_scheduler::run_reminder_scheduler;
use crate::{http_server::run_http_server, services::uptime_checker::run_uptime_checker};

use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use tokio::task;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod bots;
pub mod http_server;
pub mod observability;
pub mod services;
pub mod utils;
pub mod webhooks;

use crate::bots::bot_service::{BotConfig, BotService};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use notifine::db::{create_pool, DbPool};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

fn validate_env_vars() {
    let required_vars = [
        "DATABASE_URL",
        "GITLAB_TELOXIDE_TOKEN",
        "GITHUB_TELOXIDE_TOKEN",
        "BEEP_TELOXIDE_TOKEN",
        "UPTIME_TELOXIDE_TOKEN",
        "AGREEMENT_BOT_TOKEN",
        "WEBHOOK_BASE_URL",
        "ADMIN_LOGS",
        "TELEGRAM_ADMIN_CHAT_ID",
    ];

    let missing: Vec<&str> = required_vars
        .iter()
        .filter(|var| env::var(var).is_err())
        .copied()
        .collect();

    if !missing.is_empty() {
        eprintln!("Missing required environment variables:");
        for var in &missing {
            eprintln!("  - {}", var);
        }
        std::process::exit(1);
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    validate_env_vars();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = match create_pool(&database_url) {
        Ok(p) => p,
        Err(e) => {
            let error_msg = format!("{}", e);
            alert_database_error(&error_msg).await;
            std::process::exit(1);
        }
    };

    let mut connection = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            let error_msg = format!("Failed to get connection from pool: {}", e);
            alert_database_error(&error_msg).await;
            std::process::exit(1);
        }
    };

    if let Err(e) = connection.run_pending_migrations(MIGRATIONS) {
        let error_msg = format!("{}", e);
        alert_migration_error(&error_msg).await;
        std::process::exit(1);
    }

    let pool: DbPool = Arc::new(pool);

    task::spawn({
        let pool = pool.clone();
        BotService::new(
            BotConfig {
                bot_name: "Gitlab".to_string(),
                token: env::var("GITLAB_TELOXIDE_TOKEN")
                    .expect("GITLAB_TELOXIDE_TOKEN must be set"),
            },
            pool,
        )
        .run_bot()
    });
    task::spawn({
        let pool = pool.clone();
        BotService::new(
            BotConfig {
                bot_name: "Github".to_string(),
                token: env::var("GITHUB_TELOXIDE_TOKEN")
                    .expect("GITHUB_TELOXIDE_TOKEN must be set"),
            },
            pool,
        )
        .run_bot()
    });
    task::spawn({
        let pool = pool.clone();
        BotService::new(
            BotConfig {
                bot_name: "Beep".to_string(),
                token: env::var("BEEP_TELOXIDE_TOKEN").expect("BEEP_TELOXIDE_TOKEN must be set"),
            },
            pool,
        )
        .run_bot()
    });

    task::spawn({
        let pool = pool.clone();
        bots::uptime_bot::run_bot(pool)
    });

    task::spawn({
        let pool = pool.clone();
        bots::agreement_bot::run_bot(pool)
    });

    task::spawn({
        let pool = pool.clone();
        async move {
            run_uptime_checker(pool).await;
        }
    });

    task::spawn({
        let pool = pool.clone();
        async move {
            run_reminder_scheduler(pool).await;
        }
    });

    alert_startup_success().await;

    if let Err(e) = run_http_server(pool).await {
        let error_msg = format!("{}", e);
        alert_http_server_error(&error_msg).await;
        std::process::exit(1);
    }

    tracing::info!("Main");
}
