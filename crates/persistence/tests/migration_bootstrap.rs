use persistence::migrate;
use sqlx::PgPool;

#[sqlx::test]
async fn fresh_install_ignores_unrelated_tables_in_the_active_schema(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "CREATE TABLE unrelated_app_metadata (id BIGINT PRIMARY KEY, value TEXT NOT NULL)",
    )
    .execute(&pool)
    .await?;

    migrate(&pool).await?;

    let history_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(&pool)
            .await?;
    let edgenest_schema_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.accounts') IS NOT NULL AND to_regclass('public.venues') IS NOT NULL")
            .fetch_one(&pool)
            .await?;

    assert!(history_exists);
    assert!(edgenest_schema_exists);
    Ok(())
}

#[sqlx::test]
async fn existing_edgenest_tables_without_history_are_still_rejected(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query("CREATE TABLE accounts (id TEXT PRIMARY KEY)")
        .execute(&pool)
        .await?;

    let error = migrate(&pool)
        .await
        .expect_err("an untracked EdgeNest schema must never be silently adopted");
    let message = error.to_string();

    assert!(message.contains("without recognized SQLx history"));
    assert!(message.contains("accounts"));

    let history_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(&pool)
            .await?;
    assert!(!history_exists);
    Ok(())
}
