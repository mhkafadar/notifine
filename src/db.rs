use diesel::r2d2::{ConnectionManager, Pool, PoolError, PooledConnection};
use diesel::PgConnection;
use std::sync::Arc;

pub type PgPool = Pool<ConnectionManager<PgConnection>>;
pub type PgPooledConnection = PooledConnection<ConnectionManager<PgConnection>>;
pub type DbPool = Arc<PgPool>;

#[derive(Debug)]
pub enum DbError {
    PoolError(PoolError),
    DieselError(diesel::result::Error),
    TaskJoinError(String),
}

impl From<PoolError> for DbError {
    fn from(err: PoolError) -> Self {
        DbError::PoolError(err)
    }
}

impl From<diesel::result::Error> for DbError {
    fn from(err: diesel::result::Error) -> Self {
        DbError::DieselError(err)
    }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::PoolError(e) => write!(f, "Pool error: {}", e),
            DbError::DieselError(e) => write!(f, "Database error: {}", e),
            DbError::TaskJoinError(e) => write!(f, "Task join error: {}", e),
        }
    }
}

impl std::error::Error for DbError {}

pub fn create_pool(database_url: &str) -> Result<PgPool, PoolError> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder().build(manager)
}
