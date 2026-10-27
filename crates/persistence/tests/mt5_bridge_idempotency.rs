//! Durable MT5 idempotency and sequence checkpoint acceptance tests.

#![allow(clippy::unwrap_used)]

use persistence::{
    Mt5DurableOrderState, Mt5OrderClaim, Mt5SequenceObservation, Mt5StreamReadiness, TradingStore,
};
use serde_json::json;
use sqlx::PgPool;

async fn persist_reconciliation_snapshot(
    store: &TradingStore,
    account_id: &str,
    terminal_id: &str,
    sequence: u64,
) {
    let captured_at = chrono::Utc::now();
    store
        .persist_mt5_reconciliation_snapshot(
            account_id,
            terminal_id,
            sequence,
            "mt5_authoritative_snapshot_v1",
            captured_at.clone(),
            json!({
                "account_id": account_id,
                "terminal_id": terminal_id,
                "server": "broker-demo",
                "source": "mt5_authoritative_snapshot_v1",
                "sequence": sequence,
                "captured_at": captured_at,
                "account_state": {
                    "currency": "USD",
                    "balance": "1000.00",
                    "equity": "1000.00",
                    "margin_used": "0.00",
                    "free_margin": "1000.00"
                },
                "open_orders": [],
                "positions": []
            }),
        )
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn mt5_order_claim_is_durable_and_payload_conflicts_fail_closed(pool: PgPool) {
    let store = TradingStore::new(pool.clone());
    let payload = json!({
        "type": "submit_order",
        "client_order_id": "clid-restart-01",
        "instrument_id": "EURUSD",
        "quantity": "0.01"
    });

    let first = store
        .claim_mt5_order("account-restart-01", "clid-restart-01", payload.clone())
        .await
        .unwrap();
    assert_eq!(first, Mt5OrderClaim::New);

    // A fresh store handle models a process restart. Existing prepared claims
    // are ambiguous and must never authorize a blind resend.
    let restarted = TradingStore::new(pool);
    let replay = restarted
        .claim_mt5_order("account-restart-01", "clid-restart-01", payload)
        .await
        .unwrap();
    assert_eq!(
        replay,
        Mt5OrderClaim::Existing {
            state: Mt5DurableOrderState::Prepared,
            venue_order_id: None,
            reason_code: None,
        }
    );

    let conflict = restarted
        .claim_mt5_order(
            "account-restart-01",
            "clid-restart-01",
            json!({
                "type": "submit_order",
                "client_order_id": "clid-restart-01",
                "instrument_id": "EURUSD",
                "quantity": "0.02"
            }),
        )
        .await;
    assert!(conflict.is_err());
}

#[sqlx::test(migrations = "../../migrations")]
async fn mt5_unknown_outcome_can_reconcile_but_terminal_outcomes_are_immutable(pool: PgPool) {
    let store = TradingStore::new(pool);
    let payload = json!({"type":"submit_order","quantity":"0.01"});
    assert_eq!(
        store
            .claim_mt5_order("account-outcome-01", "clid-outcome-01", payload)
            .await
            .unwrap(),
        Mt5OrderClaim::New
    );

    store
        .record_mt5_order_outcome(
            "account-outcome-01",
            "clid-outcome-01",
            Mt5DurableOrderState::Unknown,
            None,
            Some("response_lost"),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .claim_mt5_order(
                "account-outcome-01",
                "clid-outcome-01",
                json!({"type":"submit_order","quantity":"0.01"}),
            )
            .await
            .unwrap(),
        Mt5OrderClaim::Existing {
            state: Mt5DurableOrderState::Unknown,
            venue_order_id: None,
            reason_code: Some("response_lost".into()),
        }
    );

    store
        .record_mt5_order_outcome(
            "account-outcome-01",
            "clid-outcome-01",
            Mt5DurableOrderState::Accepted,
            Some("broker-ticket-42"),
            None,
        )
        .await
        .unwrap();
    // Exact duplicate observation is idempotent.
    store
        .record_mt5_order_outcome(
            "account-outcome-01",
            "clid-outcome-01",
            Mt5DurableOrderState::Accepted,
            Some("broker-ticket-42"),
            None,
        )
        .await
        .unwrap();
    // A contradictory late response must not overwrite the accepted state.
    assert!(store
        .record_mt5_order_outcome(
            "account-outcome-01",
            "clid-outcome-01",
            Mt5DurableOrderState::Rejected,
            None,
            Some("late_reject"),
        )
        .await
        .is_err());
}

#[sqlx::test(migrations = "../../migrations")]
async fn mt5_sequence_gaps_degrade_stream_until_explicit_reconciliation(pool: PgPool) {
    let store = TradingStore::new(pool);
    assert_eq!(
        store
            .observe_mt5_sequence("account-stream-01", "terminal-01", 4)
            .await
            .unwrap(),
        Mt5SequenceObservation::ReconciliationRequired
    );
    assert_eq!(
        store
            .mt5_stream_readiness("account-stream-01", "terminal-01")
            .await
            .unwrap(),
        Mt5StreamReadiness::Degraded
    );
    assert!(store
        .mark_mt5_stream_reconciled("account-stream-01", "terminal-01", 4)
        .await
        .is_err());

    persist_reconciliation_snapshot(&store, "account-stream-01", "terminal-01", 4).await;
    assert_eq!(
        store
            .observe_mt5_sequence("account-stream-01", "terminal-01", 5)
            .await
            .unwrap(),
        Mt5SequenceObservation::Accepted
    );
    assert_eq!(
        store
            .observe_mt5_sequence("account-stream-01", "terminal-01", 7)
            .await
            .unwrap(),
        Mt5SequenceObservation::ReconciliationRequired
    );
    assert_eq!(
        store
            .mt5_stream_readiness("account-stream-01", "terminal-01")
            .await
            .unwrap(),
        Mt5StreamReadiness::Degraded
    );
    assert_eq!(
        store
            .observe_mt5_sequence("account-stream-01", "terminal-01", 8)
            .await
            .unwrap(),
        Mt5SequenceObservation::ReconciliationRequired
    );

    persist_reconciliation_snapshot(&store, "account-stream-01", "terminal-01", 8).await;
    assert_eq!(
        store
            .observe_mt5_sequence("account-stream-01", "terminal-01", 9)
            .await
            .unwrap(),
        Mt5SequenceObservation::Accepted
    );
    store
        .mark_mt5_stream_degraded("account-stream-01", "terminal-01", 10)
        .await
        .unwrap();
    assert_eq!(
        store
            .mt5_stream_readiness("account-stream-01", "terminal-01")
            .await
            .unwrap(),
        Mt5StreamReadiness::Degraded
    );
    persist_reconciliation_snapshot(&store, "account-stream-01", "terminal-01", 9).await;
    assert_eq!(
        store
            .observe_mt5_sequence("account-stream-01", "terminal-01", 10)
            .await
            .unwrap(),
        Mt5SequenceObservation::Accepted
    );
    assert!(store
        .observe_mt5_sequence("account-stream-01", "terminal-01", u64::MAX)
        .await
        .is_err());
}

#[sqlx::test(migrations = "../../migrations")]
async fn reconciliation_snapshot_persists_external_orders_positions_and_account_state_across_restart(
    pool: PgPool,
) {
    use chrono::Utc;
    use serde_json::json;
    use sqlx::types::Json;

    let store = TradingStore::new(pool.clone());
    let captured_at = Utc::now();
    let snapshot = json!({
        "account_id": "account-reconcile-01",
        "terminal_id": "terminal-reconcile-01",
        "server": "broker-demo",
        "source": "mt5_authoritative_snapshot_v1",
        "sequence": 42,
        "captured_at": captured_at,
        "account_state": {
            "currency": "USD",
            "balance": "10000.00",
            "equity": "9975.50",
            "margin_used": "500.00",
            "free_margin": "9475.50"
        },
        "open_orders": [{
            "venue_order_id": "ticket-7001",
            "client_order_id": "client-7001",
            "instrument_id": "EURUSD.a",
            "side": "buy",
            "order_type": "limit",
            "quantity": "0.10",
            "status": "working"
        }],
        "positions": [{
            "venue_position_id": "position-8001",
            "instrument_id": "EURUSD.a",
            "side": "buy",
            "quantity": "0.10",
            "average_price": "1.08000",
            "unrealized_pnl": "-24.50"
        }]
    });

    store
        .persist_mt5_reconciliation_snapshot(
            "account-reconcile-01",
            "terminal-reconcile-01",
            42,
            "mt5_authoritative_snapshot_v1",
            captured_at,
            snapshot.clone(),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .mt5_stream_readiness("account-reconcile-01", "terminal-reconcile-01")
            .await
            .unwrap(),
        Mt5StreamReadiness::Ready
    );

    // A new store handle simulates a process restart. The persisted external
    // state is available for recovery, and the next heartbeat must be sequential.
    let restarted = TradingStore::new(pool.clone());
    let persisted = sqlx::query_scalar::<_, Json<serde_json::Value>>(
        "SELECT snapshot FROM mt5_bridge_reconciliation_snapshots WHERE account_id = $1 AND terminal_id = $2 ORDER BY id DESC LIMIT 1",
    )
    .bind("account-reconcile-01")
    .bind("terminal-reconcile-01")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(persisted.0, snapshot);
    assert_eq!(
        restarted
            .observe_mt5_sequence("account-reconcile-01", "terminal-reconcile-01", 43)
            .await
            .unwrap(),
        Mt5SequenceObservation::Accepted
    );

    let stale = restarted
        .persist_mt5_reconciliation_snapshot(
            "account-reconcile-01",
            "terminal-reconcile-01",
            41,
            "mt5_authoritative_snapshot_v1",
            captured_at,
            snapshot,
        )
        .await;
    assert!(stale.is_err());
}

#[sqlx::test(migrations = "../../migrations")]
async fn reconciliation_snapshot_rejects_incomplete_external_state(pool: PgPool) {
    use chrono::Utc;
    use serde_json::json;

    let store = TradingStore::new(pool);
    let incomplete = json!({
        "account_id": "account-reconcile-02",
        "terminal_id": "terminal-reconcile-02",
        "source": "mt5_authoritative_snapshot_v1",
        "account_state": { "currency": "USD", "balance": "1000.00" },
        "open_orders": []
    });
    assert!(store
        .persist_mt5_reconciliation_snapshot(
            "account-reconcile-02",
            "terminal-reconcile-02",
            1,
            "mt5_authoritative_snapshot_v1",
            Utc::now(),
            incomplete,
        )
        .await
        .is_err());
}


#[sqlx::test(migrations = "../../migrations")]
async fn market_data_snapshot_is_persisted_without_marking_stream_ready(pool: PgPool) {
    use chrono::Utc;
    use sqlx::types::Json;

    let store = TradingStore::new(pool.clone());
    let captured_at = Utc::now();
    let snapshot = json!({
        "terminal_id": "terminal-market-01",
        "server": "broker-demo",
        "source": "mt5_python_readonly_v1",
        "sequence": 12,
        "captured_at": captured_at,
        "account_currency": "USD",
        "instruments": [{
            "symbol": "EURUSD.a",
            "base_currency": "EUR",
            "quote_currency": "USD",
            "digits": 5,
            "point": "0.00001",
            "tick_size": "0.00001",
            "tick_value_account_currency": "1.0",
            "contract_size": "100000",
            "volume_min": "0.01",
            "volume_max": "100",
            "volume_step": "0.01",
            "trade_mode": "4"
        }],
        "quotes": [{
            "symbol": "EURUSD.a",
            "bid": "1.0845",
            "ask": "1.08462",
            "tick_time": captured_at
        }]
    });

    store.persist_mt5_market_data_snapshot(
        "account-market-01",
        "terminal-market-01",
        "broker-demo",
        12,
        "mt5_python_readonly_v1",
        "USD",
        captured_at,
        snapshot.clone(),
    ).await.unwrap();

    let persisted = sqlx::query_scalar::<_, Json<serde_json::Value>>(
        "SELECT snapshot FROM mt5_bridge_market_data_snapshots WHERE account_id = $1 AND terminal_id = $2 ORDER BY id DESC LIMIT 1",
    )
    .bind("account-market-01")
    .bind("terminal-market-01")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(persisted.0, snapshot);
    assert_eq!(
        store.mt5_stream_readiness("account-market-01", "terminal-market-01").await.unwrap(),
        Mt5StreamReadiness::Unknown
    );

    let invalid = json!({
        "terminal_id": "terminal-market-01",
        "server": "broker-demo",
        "source": "mt5_python_readonly_v1",
        "sequence": 13,
        "captured_at": captured_at,
        "account_currency": "USD",
        "instruments": [],
        "quotes": []
    });
    assert!(store.persist_mt5_market_data_snapshot(
        "account-market-01",
        "terminal-market-01",
        "broker-demo",
        13,
        "mt5_python_readonly_v1",
        "USD",
        captured_at,
        invalid,
    ).await.is_err());
}
