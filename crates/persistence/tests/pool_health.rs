use persistence::check_database_health;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

#[sqlx::test]
async fn health_probe_detects_closed_pool_and_a_new_pool_recovers(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    check_database_health(&pool).await?;
    let connect_options = pool.connect_options().as_ref().clone();

    pool.close().await;
    assert!(check_database_health(&pool).await.is_err());

    let recovered_pool = PgPoolOptions::new().connect_with(connect_options).await?;
    check_database_health(&recovered_pool).await?;
    recovered_pool.close().await;
    Ok(())
}
