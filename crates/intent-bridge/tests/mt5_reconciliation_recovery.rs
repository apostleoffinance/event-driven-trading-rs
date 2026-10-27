use std::error::Error;

use chrono::{Duration, Utc};
use intent_bridge::mt5_reconciliation::{
    persist_and_confirm_reconciliation, Mt5AccountStateSnapshot, Mt5OpenOrderSnapshot,
    Mt5PositionSnapshot, Mt5ReconciliationSnapshot,
};
use intent_bridge::mt5_session::{Mt5SessionGate, Mt5SessionPolicy, Mt5SessionState};
use intent_bridge::mt5_v1::{
    BridgeCapabilities, BridgeFrame, BridgePayload, ConnectionState, Heartbeat,
    MT5_BRIDGE_SCHEMA_VERSION,
};
use persistence::{Mt5SequenceObservation, Mt5StreamReadiness, TradingStore};
use sqlx::PgPool;

fn heartbeat_frame(now: chrono::DateTime<Utc>) -> BridgeFrame {
    BridgeFrame {
        schema_version: MT5_BRIDGE_SCHEMA_VERSION,
        message_id: "recovery-heartbeat-1".into(),
        correlation_id: "recovery-correlation-1".into(),
        account_id: "account-recovery-01".into(),
        sent_at: now,
        payload: BridgePayload::Heartbeat(Heartbeat {
            terminal_id: "terminal-recovery-01".into(),
            server: "broker-demo".into(),
            sequence: 1,
            terminal_time: now,
            connection: ConnectionState::Connected,
            capabilities: BridgeCapabilities {
                market_orders: true,
                limit_orders: true,
                stop_orders: true,
                cancel_requests: true,
                order_lookup: true,
                idempotent_client_order_id: true,
                sequence_numbers: true,
            },
        }),
    }
}

fn external_snapshot(now: chrono::DateTime<Utc>) -> Mt5ReconciliationSnapshot {
    Mt5ReconciliationSnapshot {
        account_id: "account-recovery-01".into(),
        terminal_id: "terminal-recovery-01".into(),
        server: "broker-demo".into(),
        source: "mt5_authoritative_snapshot_v1".into(),
        sequence: 1,
        captured_at: now,
        account_state: Mt5AccountStateSnapshot {
            currency: "USD".into(),
            balance: "10000.00".into(),
            equity: "9975.50".into(),
            margin_used: "500.00".into(),
            free_margin: "9475.50".into(),
        },
        open_orders: vec![Mt5OpenOrderSnapshot {
            venue_order_id: "ticket-7001".into(),
            client_order_id: Some("client-7001".into()),
            instrument_id: "EURUSD.a".into(),
            side: "buy".into(),
            order_type: "limit".into(),
            quantity: "0.10".into(),
            status: "working".into(),
        }],
        positions: vec![Mt5PositionSnapshot {
            venue_position_id: "position-8001".into(),
            instrument_id: "EURUSD.a".into(),
            side: "buy".into(),
            quantity: "0.10".into(),
            average_price: "1.08000".into(),
            unrealized_pnl: "-24.50".into(),
        }],
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn authoritative_snapshot_restores_session_after_restart(
    pool: PgPool,
) -> Result<(), Box<dyn Error>> {
    let now = Utc::now();
    let store = TradingStore::new(pool.clone());
    let heartbeat = heartbeat_frame(now);
    let (terminal_id, sequence) = match &heartbeat.payload {
        BridgePayload::Heartbeat(value) => (value.terminal_id.as_str(), value.sequence),
        _ => return Err("test fixture must be heartbeat".into()),
    };
    let observation = store
        .observe_mt5_sequence(&heartbeat.account_id, terminal_id, sequence)
        .await?;
    assert_eq!(observation, Mt5SequenceObservation::ReconciliationRequired);
    let readiness = store
        .mt5_stream_readiness(&heartbeat.account_id, terminal_id)
        .await?;
    assert_eq!(readiness, Mt5StreamReadiness::Degraded);

    let policy = Mt5SessionPolicy::new(["account-recovery-01"])?;
    let mut session = Mt5SessionGate::after_transport_authentication(policy);
    assert_eq!(
        session.observe_heartbeat(&heartbeat, observation, readiness)?,
        Mt5SessionState::ReconciliationRequired
    );
    persist_and_confirm_reconciliation(
        &store,
        &mut session,
        &external_snapshot(now),
        now,
        Duration::seconds(30),
    )
    .await?;
    assert_eq!(session.state(), Mt5SessionState::Ready);

    // A new store instance simulates a process restart. Readiness and the exact
    // external order/position/account snapshot must survive independently of RAM.
    let restarted = TradingStore::new(pool.clone());
    assert_eq!(
        restarted
            .mt5_stream_readiness("account-recovery-01", "terminal-recovery-01")
            .await?,
        Mt5StreamReadiness::Ready
    );
    let persisted = sqlx::query_scalar::<_, sqlx::types::Json<serde_json::Value>>(
        "SELECT snapshot FROM mt5_bridge_reconciliation_snapshots WHERE account_id = $1 AND terminal_id = $2 ORDER BY id DESC LIMIT 1",
    )
    .bind("account-recovery-01")
    .bind("terminal-recovery-01")
    .fetch_one(&pool)
    .await?;
    assert_eq!(persisted.0["account_state"]["equity"], "9975.50");
    assert_eq!(
        persisted.0["open_orders"][0]["venue_order_id"],
        "ticket-7001"
    );
    assert_eq!(
        persisted.0["positions"][0]["venue_position_id"],
        "position-8001"
    );
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn identity_mismatch_cannot_restore_reconciliation_readiness(
    pool: PgPool,
) -> Result<(), Box<dyn Error>> {
    let now = Utc::now();
    let store = TradingStore::new(pool);
    let heartbeat = heartbeat_frame(now);
    let policy = Mt5SessionPolicy::new(["account-recovery-01"])?;
    let mut session = Mt5SessionGate::after_transport_authentication(policy);
    session.observe_heartbeat(
        &heartbeat,
        Mt5SequenceObservation::ReconciliationRequired,
        Mt5StreamReadiness::Degraded,
    )?;
    let mut mismatched = external_snapshot(now);
    mismatched.server = "other-broker".into();
    assert!(persist_and_confirm_reconciliation(
        &store,
        &mut session,
        &mismatched,
        now,
        Duration::seconds(30),
    )
    .await
    .is_err());
    assert_eq!(session.state(), Mt5SessionState::Degraded);
    assert_eq!(
        store
            .mt5_stream_readiness("account-recovery-01", "terminal-recovery-01")
            .await?,
        Mt5StreamReadiness::Degraded
    );
    Ok(())
}
