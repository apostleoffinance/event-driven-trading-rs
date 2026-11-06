//! Journal schema integration tests.
//!
//! These run against disposable PostgreSQL in CI and apply the full SQLx migration chain.

use sqlx::PgPool;

#[sqlx::test(migrations = "../../migrations")]
async fn journal_schema_preserves_owner_scope_and_import_idempotency(pool: PgPool) {
    sqlx::query(
        r#"
        INSERT INTO journal_accounts (id, owner_id, name, currency)
        VALUES ('ja-owner-a', 'owner-a', 'Owner A Journal', 'USD')
        "#,
    )
    .execute(&pool)
    .await
    .expect("journal account should insert");

    sqlx::query(
        r#"
        INSERT INTO journal_setups (id, owner_id, name)
        VALUES ('setup-owner-a', 'owner-a', 'Breakout')
        "#,
    )
    .execute(&pool)
    .await
    .expect("journal setup should insert");

    sqlx::query(
        r#"
        INSERT INTO journal_trades (
            id, owner_id, journal_account_id, setup_id, symbol, direction, status,
            source, source_system, source_trade_id, opened_at, closed_at,
            quantity, entry_price, exit_price, gross_pnl, commission, swap,
            other_fees, net_pnl, pnl_currency
        ) VALUES (
            'trade-import-1', 'owner-a', 'ja-owner-a', 'setup-owner-a', 'EURUSD',
            'long', 'closed', 'legacy_import', 'TraderOS', 'legacy-trade-100',
            '2026-01-01T10:00:00Z', '2026-01-01T11:00:00Z',
            0.10, 1.10000, 1.10500, 50.00, -2.00, 0.00, 0.00, 48.00, 'USD'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("journal trade should insert");

    // The owner/account composite FK must reject cross-owner account links.
    let cross_owner_account = sqlx::query(
        r#"
        INSERT INTO journal_trades (
            id, owner_id, journal_account_id, symbol, direction, status, source,
            opened_at, quantity, entry_price, pnl_currency
        ) VALUES (
            'cross-owner-trade', 'owner-b', 'ja-owner-a', 'EURUSD', 'long',
            'open', 'manual', '2026-01-01T10:00:00Z', 0.10, 1.10000, 'USD'
        )
        "#,
    )
    .execute(&pool)
    .await;
    assert!(cross_owner_account.is_err());

    // Source provenance is a unique idempotency key for legacy import replay.
    let duplicate_source = sqlx::query(
        r#"
        INSERT INTO journal_trades (
            id, owner_id, journal_account_id, symbol, direction, status, source,
            source_system, source_trade_id, opened_at, closed_at, quantity,
            entry_price, exit_price, pnl_currency
        ) VALUES (
            'trade-import-duplicate', 'owner-a', 'ja-owner-a', 'EURUSD', 'long',
            'closed', 'legacy_import', 'TraderOS', 'legacy-trade-100',
            '2026-01-01T10:00:00Z', '2026-01-01T11:00:00Z', 0.10,
            1.10000, 1.10500, 'USD'
        )
        "#,
    )
    .execute(&pool)
    .await;
    assert!(duplicate_source.is_err());

    let row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_trades WHERE owner_id = 'owner-a'",
    )
    .fetch_one(&pool)
    .await
    .expect("journal count query should succeed");
    assert_eq!(row_count, 1);
}
