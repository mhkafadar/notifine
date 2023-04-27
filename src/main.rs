use crate::bots::gitlab_bot::run_gitlab_bot;
use crate::http_server::run_http_server;

use dotenv::dotenv;
use std::env;
use tokio::task;

pub mod bots;
pub mod http_server;
pub mod webhooks;

use crate::bots::beep_bot::run_beep_bot;
use crate::bots::github_bot::run_github_bot;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::PgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

pub type PgPool = Pool<ConnectionManager<PgConnection>>;
pub type PgPooledConnection = PooledConnection<ConnectionManager<PgConnection>>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    // Make sure the database is up to date (create if it doesn't exist, or run the migrations)
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = diesel::r2d2::Pool::new(manager).unwrap();
    let mut connection = pool.get().unwrap();
    connection.run_pending_migrations(MIGRATIONS);

    task::spawn(run_gitlab_bot());
    task::spawn(run_github_bot());
    task::spawn(run_beep_bot());
    // task::spawn(run_trello_bot());
    run_http_server().await.expect("Http server error");

    log::info!("Main");
}
