use persistence::migrate;
use sqlx::PgPool;

#[sqlx::test]
async fn fresh_install_bootstraps_an_empty_schema(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
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
async fn existing_tables_without_history_are_rejected_with_target_diagnostics(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query("CREATE TABLE unrelated_app_metadata (id BIGINT PRIMARY KEY, value TEXT NOT NULL)")
        .execute(&pool)
        .await?;

    let error = migrate(&pool)
        .await
        .expect_err("any untracked application table must block fresh bootstrap");
    let message = error.to_string();

    assert!(message.contains("without recognized SQLx history"));
    assert!(message.contains("unrelated_app_metadata"));
    assert!(message.contains("database"));
    assert!(message.contains("schema"));

    let history_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(&pool)
            .await?;
    assert!(!history_exists);
    Ok(())
}
