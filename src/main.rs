use crate::observability::startup::{
    alert_database_error, alert_http_server_error, alert_migration_error, alert_startup_success,
};
use crate::services::broadcast::BroadcastWorker;
use crate::services::reminder_scheduler::run_reminder_scheduler;
use crate::{http_server::run_http_server, services::uptime_checker::run_uptime_checker};

use dotenv::dotenv;
use std::sync::Arc;
use tokio::task;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod bots;
pub mod config;
pub mod http_server;
pub mod observability;
pub mod services;
pub mod utils;
pub mod webhooks;

use crate::bots::bot_service::{BotConfig, BotService};
use crate::config::AppConfig;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use notifine::db::{create_pool, DbPool};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = match AppConfig::from_env() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            tracing::error!("{}", e);
            std::process::exit(1);
        }
    };

    let pool = match create_pool(&config.database_url) {
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

    if let Some(token) = config.gitlab_token.clone() {
        let pool = pool.clone();
        let webhook_base_url = config.webhook_base_url.clone();
        let admin_chat_id = config.admin_chat_id;
        task::spawn(
            BotService::new(
                BotConfig {
                    bot_name: "Gitlab".to_string(),
                    token,
                    webhook_base_url,
                    admin_chat_id,
                },
                pool,
            )
            .run_bot(),
        );
        tracing::info!("GitLab bot enabled");
    }

    if let Some(token) = config.github_token.clone() {
        let pool = pool.clone();
        let webhook_base_url = config.webhook_base_url.clone();
        let admin_chat_id = config.admin_chat_id;
        task::spawn(
            BotService::new(
                BotConfig {
                    bot_name: "Github".to_string(),
                    token,
                    webhook_base_url,
                    admin_chat_id,
                },
                pool,
            )
            .run_bot(),
        );
        tracing::info!("GitHub bot enabled");
    }

    if let Some(token) = config.beep_token.clone() {
        let pool = pool.clone();
        let webhook_base_url = config.webhook_base_url.clone();
        let admin_chat_id = config.admin_chat_id;
        task::spawn(
            BotService::new(
                BotConfig {
                    bot_name: "Beep".to_string(),
                    token,
                    webhook_base_url,
                    admin_chat_id,
                },
                pool,
            )
            .run_bot(),
        );
        tracing::info!("Beep bot enabled");
    }

    if let Some(token) = config.uptime_token.clone() {
        let pool = pool.clone();
        task::spawn(bots::uptime_bot::run_bot(pool, token));
        tracing::info!("Uptime bot enabled");
    }

    if let Some(token) = config.agreement_bot_token.clone() {
        let pool = pool.clone();
        task::spawn(bots::agreement_bot::run_bot(pool, token));
        tracing::info!("Agreement bot enabled");
    }

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

    task::spawn({
        let pool = pool.clone();
        let admin_chat_id = config.admin_chat_id;
        async move {
            let worker = BroadcastWorker::new(pool, admin_chat_id);
            worker.run().await;
        }
    });
    tracing::info!("Broadcast worker enabled");

    alert_startup_success().await;

    if let Err(e) = run_http_server(pool).await {
        let error_msg = format!("{}", e);
        alert_http_server_error(&error_msg).await;
        std::process::exit(1);
    }

    tracing::info!("Main");
}
