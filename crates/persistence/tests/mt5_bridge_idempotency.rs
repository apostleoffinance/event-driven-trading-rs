//! Durable MT5 idempotency and sequence checkpoint acceptance tests.

#![allow(clippy::unwrap_used)]

use persistence::{
    Mt5DurableOrderState, Mt5OrderClaim, Mt5SequenceObservation, Mt5StreamReadiness, TradingStore,
};
use serde_json::json;
use sqlx::PgPool;

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

    store
        .mark_mt5_stream_reconciled("account-stream-01", "terminal-01", 4)
        .await
        .unwrap();
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

    store
        .mark_mt5_stream_reconciled("account-stream-01", "terminal-01", 8)
        .await
        .unwrap();
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
    store
        .mark_mt5_stream_reconciled("account-stream-01", "terminal-01", 9)
        .await
        .unwrap();
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
