use std::path::PathBuf;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::error::PersistenceResult;

/// Connect to PostgreSQL using `DATABASE_URL`.
pub async fn connect(database_url: &str) -> PersistenceResult<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}

fn migrations_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../migrations")
}

/// Run SQL migrations from the workspace `migrations/` directory.
pub async fn migrate(pool: &PgPool) -> PersistenceResult<()> {
    let migrator = sqlx::migrate::Migrator::new(migrations_path()).await?;
    migrator.run(pool).await?;
    tracing::info!("database migrations applied");
    Ok(())
}
