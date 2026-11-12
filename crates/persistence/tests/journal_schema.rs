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

    let row_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM journal_trades WHERE owner_id = 'owner-a'")
            .fetch_one(&pool)
            .await
            .expect("journal count query should succeed");
    assert_eq!(row_count, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn journal_get_by_id_returns_all_trade_annotations(pool: PgPool) {
    sqlx::query(
        "INSERT INTO journal_accounts (id, owner_id, name, currency) VALUES ('get-account', 'get-owner', 'Journal', 'USD')",
    )
    .execute(&pool)
    .await
    .expect("account insert");

    sqlx::query(
        r#"INSERT INTO journal_trades (
            id, owner_id, journal_account_id, symbol, direction, status, source,
            opened_at, closed_at, quantity, entry_price, exit_price, pnl_currency,
            session, timeframe, timezone, mfe_r, mae_r, notes, tags,
            setup_valid, rules_followed, emotional_trade, mistake, mistake_notes,
            discipline_score, acknowledged_warnings
        ) VALUES (
            'get-trade', 'get-owner', 'get-account', 'EURUSD', 'long', 'closed', 'manual',
            '2026-01-01T10:00:00Z', '2026-01-01T11:00:00Z', 0.1, 1.1, 1.105, 'USD',
            'London', 'M15', 'Africa/Lagos', 1.5, -0.5, 'review notes', '["breakout"]',
            TRUE, TRUE, FALSE, FALSE, NULL, 90, '[]'
        )"#,
    )
    .execute(&pool)
    .await
    .expect("trade insert");

    let store = persistence::TradingStore::new(pool);
    let trade = store
        .get_journal_trade("get-owner", "get-trade")
        .await
        .expect("owner-scoped trade lookup");

    assert_eq!(trade.symbol, "EURUSD");
    assert_eq!(trade.session.as_deref(), Some("London"));
    assert_eq!(trade.timeframe.as_deref(), Some("M15"));
    assert_eq!(trade.timezone.as_deref(), Some("Africa/Lagos"));
    assert_eq!(trade.mfe_r, Some(rust_decimal::Decimal::new(15, 1)));
    assert_eq!(trade.mae_r, Some(rust_decimal::Decimal::new(-5, 1)));
    assert_eq!(trade.notes.as_deref(), Some("review notes"));
    assert_eq!(trade.tags, vec!["breakout"]);
    assert_eq!(trade.setup_valid, Some(true));
    assert_eq!(trade.rules_followed, Some(true));
    assert_eq!(trade.emotional_trade, Some(false));
    assert_eq!(trade.mistake, Some(false));
    assert_eq!(trade.discipline_score, Some(90));
}

#[sqlx::test(migrations = "../../migrations")]
async fn analytics_saved_views_are_owner_and_account_scoped(pool: PgPool) {
    sqlx::query(
        "INSERT INTO journal_accounts (id, owner_id, name, currency) VALUES ('saved-a1', 'saved-owner-a', 'A1', 'USD'), ('saved-a2', 'saved-owner-a', 'A2', 'USD'), ('saved-b1', 'saved-owner-b', 'B1', 'USD')",
    )
    .execute(&pool)
    .await
    .expect("accounts should insert");

    sqlx::query(
        "INSERT INTO journal_analytics_saved_views (id, owner_id, scope_key, name, filters) VALUES ('view-a1', 'saved-owner-a', 'saved-a1', 'London setup', '{\"datePreset\":\"30d\"}'), ('view-a2', 'saved-owner-a', 'saved-a2', 'London setup', '{\"datePreset\":\"7d\"}'), ('view-b1', 'saved-owner-b', 'saved-a1', 'London setup', '{\"datePreset\":\"all\"}')",
    )
    .execute(&pool)
    .await
    .expect("same view name is valid across owner/account scopes");

    let owner_a_account_a: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_analytics_saved_views WHERE owner_id='saved-owner-a' AND scope_key='saved-a1'",
    )
    .fetch_one(&pool)
    .await
    .expect("owner/account scoped query should succeed");
    assert_eq!(owner_a_account_a, 1);

    let owner_a_account_b: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_analytics_saved_views WHERE owner_id='saved-owner-a' AND scope_key='saved-a2'",
    )
    .fetch_one(&pool)
    .await
    .expect("second account scope should query");
    assert_eq!(owner_a_account_b, 1);

    let owner_b: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_analytics_saved_views WHERE owner_id='saved-owner-b' AND scope_key='saved-a1'",
    )
    .fetch_one(&pool)
    .await
    .expect("second owner scope should query");
    assert_eq!(owner_b, 1);

    let duplicate_same_scope = sqlx::query(
        "INSERT INTO journal_analytics_saved_views (id, owner_id, scope_key, name, filters) VALUES ('view-a1-duplicate', 'saved-owner-a', 'saved-a1', 'London setup', '{\"datePreset\":\"all\"}')",
    )
    .execute(&pool)
    .await;
    assert!(
        duplicate_same_scope.is_err(),
        "a view name must be unique within one owner/account scope"
    );

    let invalid_filters = sqlx::query(
        "INSERT INTO journal_analytics_saved_views (id, owner_id, scope_key, name, filters) VALUES ('view-invalid', 'saved-owner-a', 'saved-a1', 'Invalid', '[]')",
    )
    .execute(&pool)
    .await;
    assert!(
        invalid_filters.is_err(),
        "filters must remain a JSON object"
    );
}
