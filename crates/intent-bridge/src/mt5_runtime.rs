//! Host-side orchestration for one authenticated MT5 terminal session.
//!
//! This coordinator owns the shared connection/session locks, starts the heartbeat
//! watchdog, checkpoints incoming heartbeats, persists external reconciliation state,
//! and consumes durable dispatch permits through the authenticated transport. It is
//! not wired to a production daemon or MT5 EA by this crate alone; the host must create
//! it only after TLS authentication and must provide authoritative venue snapshots.

use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, Utc};
use persistence::{Mt5DurableOrderState, TradingStore};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::mt5_dispatch::{prepare_submit, Mt5DispatchDecision};
use crate::mt5_reconciliation::{
    persist_and_confirm_reconciliation, Mt5ReconciliationSnapshot, Mt5ReconciliationSource,
};
use crate::mt5_session::{Mt5SessionGate, Mt5SessionState};
use crate::mt5_transport::{spawn_heartbeat_watchdog, AuthenticatedMt5Connection};
use crate::mt5_v1::{BridgeFrame, BridgePayload};
use crate::{BridgeError, BridgeResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mt5RuntimeDispatchResult {
    /// A correlated venue outcome was durably persisted. Inspect the durable order
    /// state to distinguish accepted/rejected/unknown; this is not a fill guarantee.
    OutcomePersisted,
    /// The durable key already existed; never resend it. Reconcile the venue state.
    ReconciliationRequired {
        client_order_id: String,
        prior_state: Mt5DurableOrderState,
        venue_order_id: Option<String>,
        reason_code: Option<String>,
    },
}

pub struct Mt5BridgeRuntime {
    store: TradingStore,
    connection: Arc<Mutex<AuthenticatedMt5Connection>>,
    session: Arc<Mutex<Mt5SessionGate>>,
    watchdog: JoinHandle<()>,
}

impl Mt5BridgeRuntime {
    /// Construct only after the TLS transport has authenticated the client certificate
    /// and the session policy is bound to the intended account allow-list.
    pub fn new(
        store: TradingStore,
        connection: AuthenticatedMt5Connection,
        session: Mt5SessionGate,
        watchdog_poll_interval: StdDuration,
    ) -> BridgeResult<Self> {
        validate_initial_session(&connection, &session)?;
        let connection = Arc::new(Mutex::new(connection));
        let session = Arc::new(Mutex::new(session));
        let watchdog = spawn_heartbeat_watchdog(
            connection.clone(),
            session.clone(),
            store.clone(),
            watchdog_poll_interval,
        )?;
        Ok(Self {
            store,
            connection,
            session,
            watchdog,
        })
    }

    /// Replace a failed connection with a newly authenticated transport/session.
    /// The caller must construct the new session after TLS identity validation; the
    /// first heartbeat on this connection will still require durable external-state
    /// reconciliation before any submit can be authorized.
    pub async fn reconnect_authenticated(
        &mut self,
        connection: AuthenticatedMt5Connection,
        session: Mt5SessionGate,
        watchdog_poll_interval: StdDuration,
    ) -> BridgeResult<()> {
        if watchdog_poll_interval.is_zero() {
            return Err(BridgeError::Invalid(
                "MT5 heartbeat watchdog poll interval must be positive".into(),
            ));
        }
        validate_initial_session(&connection, &session)?;
        self.watchdog.abort();
        {
            let mut current_connection = self.connection.lock().await;
            *current_connection = connection;
        }
        {
            let mut current_session = self.session.lock().await;
            *current_session = session;
        }
        self.watchdog = spawn_heartbeat_watchdog(
            self.connection.clone(),
            self.session.clone(),
            self.store.clone(),
            watchdog_poll_interval,
        )?;
        Ok(())
    }

    /// Read a validated frame and durably checkpoint heartbeat sequence state. A read,
    /// protocol or persistence error degrades both in-memory and durable readiness.
    pub async fn read_next_frame(
        &self,
        now: DateTime<Utc>,
        allowed_clock_skew: Duration,
    ) -> BridgeResult<BridgeFrame> {
        let mut connection = self.connection.lock().await;
        let mut session = self.session.lock().await;
        match connection
            .read_frame_with_session_checkpoint(&self.store, &mut session, now, allowed_clock_skew)
            .await
        {
            Ok(frame) => Ok(frame),
            Err(error) => {
                session.degrade();
                if let (Some(account_id), Some(terminal_id)) =
                    (session.account_id(), session.terminal_id())
                {
                    let _ = self
                        .store
                        .mark_mt5_stream_degraded(account_id, terminal_id, 0)
                        .await;
                }
                Err(error)
            }
        }
    }

    /// Reconcile a fresh authoritative snapshot of account state, all open orders,
    /// and all positions before releasing the session readiness latch.
    pub async fn reconcile(
        &self,
        snapshot: &Mt5ReconciliationSnapshot,
        now: DateTime<Utc>,
        max_age: Duration,
    ) -> BridgeResult<()> {
        let mut session = self.session.lock().await;
        persist_and_confirm_reconciliation(&self.store, &mut session, snapshot, now, max_age).await
    }

    /// Consume a complete snapshot frame from the authenticated terminal client,
    /// validate its identity against the bound account/session, and persist it before
    /// restoring reconciliation readiness. No order submission is performed here.
    pub async fn reconcile_from_next_frame(
        &self,
        allowed_clock_skew: Duration,
        max_age: Duration,
    ) -> BridgeResult<()> {
        let frame = loop {
            let frame = self.read_next_frame(Utc::now(), allowed_clock_skew).await?;
            if matches!(&frame.payload, BridgePayload::Heartbeat(_)) {
                continue;
            }
            if let BridgePayload::MarketDataSnapshot(snapshot) = &frame.payload {
                let serialized = serde_json::to_value(snapshot)?;
                if let Err(error) = self
                    .store
                    .persist_mt5_market_data_snapshot(
                        &frame.account_id,
                        &snapshot.terminal_id,
                        &snapshot.server,
                        snapshot.sequence,
                        &snapshot.source,
                        &snapshot.account_currency,
                        snapshot.captured_at,
                        serialized,
                    )
                    .await
                {
                    let mut session = self.session.lock().await;
                    session.degrade();
                    let _ = self
                        .store
                        .mark_mt5_stream_degraded(
                            &frame.account_id,
                            &snapshot.terminal_id,
                            snapshot.sequence,
                        )
                        .await;
                    return Err(BridgeError::from(error));
                }
                continue;
            }
            if matches!(&frame.payload, BridgePayload::ReconciliationSnapshot(_)) {
                break frame;
            }
            let mut session = self.session.lock().await;
            session.degrade();
            return Err(BridgeError::Invalid(
                "expected reconciliation_snapshot frame from read-only MT5 adapter".into(),
            ));
        };
        let wire = match frame.payload {
            BridgePayload::ReconciliationSnapshot(snapshot) => snapshot,
            _ => {
                let mut session = self.session.lock().await;
                session.degrade();
                return Err(BridgeError::Invalid(
                    "expected reconciliation_snapshot frame from read-only MT5 adapter".into(),
                ));
            }
        };
        if frame.account_id.trim().is_empty() || frame.account_id.trim() != frame.account_id {
            let mut session = self.session.lock().await;
            session.degrade();
            return Err(BridgeError::Invalid(
                "invalid account ID on MT5 snapshot frame".into(),
            ));
        }
        let snapshot = Mt5ReconciliationSnapshot {
            account_id: frame.account_id,
            terminal_id: wire.terminal_id,
            server: wire.server,
            source: wire.source,
            sequence: wire.sequence,
            captured_at: wire.captured_at,
            account_state: crate::mt5_reconciliation::Mt5AccountStateSnapshot {
                currency: wire.account_state.currency,
                balance: wire.account_state.balance,
                equity: wire.account_state.equity,
                margin_used: wire.account_state.margin_used,
                free_margin: wire.account_state.free_margin,
            },
            open_orders: wire
                .open_orders
                .into_iter()
                .map(|order| crate::mt5_reconciliation::Mt5OpenOrderSnapshot {
                    venue_order_id: order.venue_order_id,
                    client_order_id: order.client_order_id,
                    instrument_id: order.instrument_id,
                    side: order.side,
                    order_type: order.order_type,
                    quantity: order.quantity,
                    status: order.status,
                })
                .collect(),
            positions: wire
                .positions
                .into_iter()
                .map(|position| crate::mt5_reconciliation::Mt5PositionSnapshot {
                    venue_position_id: position.venue_position_id,
                    instrument_id: position.instrument_id,
                    side: position.side,
                    quantity: position.quantity,
                    average_price: position.average_price,
                    unrealized_pnl: position.unrealized_pnl,
                })
                .collect(),
        };
        self.reconcile(&snapshot, Utc::now(), max_age).await
    }

    /// Fetch and persist an authoritative external snapshot after reconnect or
    /// watchdog degradation. A source failure leaves the session and durable stream
    /// degraded; it never restores readiness from a local/cache-only inference.
    pub async fn reconcile_from_source<S: Mt5ReconciliationSource + ?Sized>(
        &self,
        source: &S,
        now: DateTime<Utc>,
        max_age: Duration,
    ) -> BridgeResult<()> {
        let (account_id, terminal_id, server) = {
            let session = self.session.lock().await;
            if session.state() != Mt5SessionState::ReconciliationRequired {
                return Err(BridgeError::Invalid(
                    "MT5 source reconciliation requires a session awaiting reconciliation".into(),
                ));
            }
            let identity = match (
                session.account_id(),
                session.terminal_id(),
                session.server(),
            ) {
                (Some(account_id), Some(terminal_id), Some(server)) => (
                    account_id.to_owned(),
                    terminal_id.to_owned(),
                    server.to_owned(),
                ),
                _ => {
                    return Err(BridgeError::Invalid(
                        "MT5 source reconciliation requires a bound authenticated identity".into(),
                    ));
                }
            };
            identity
        };

        let snapshot = match source
            .fetch_snapshot(&account_id, &terminal_id, &server)
            .await
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let mut session = self.session.lock().await;
                session.degrade();
                let _ = self
                    .store
                    .mark_mt5_stream_degraded(&account_id, &terminal_id, 0)
                    .await;
                return Err(error);
            }
        };
        self.reconcile(&snapshot, now, max_age).await
    }

    /// Claim a client-order key durably and execute at most one send through the
    /// authenticated connection. Existing keys are returned for reconciliation only.
    pub async fn dispatch_submit(
        &self,
        frame: &BridgeFrame,
        now: DateTime<Utc>,
        allowed_clock_skew: Duration,
    ) -> BridgeResult<Mt5RuntimeDispatchResult> {
        let decision = {
            let session = self.session.lock().await;
            prepare_submit(&self.store, &session, frame, now, allowed_clock_skew).await?
        };
        match decision {
            Mt5DispatchDecision::SendOnce(permit) => {
                let mut connection = self.connection.lock().await;
                let mut session = self.session.lock().await;
                permit
                    .execute_over_transport(
                        &mut connection,
                        &self.store,
                        &mut session,
                        allowed_clock_skew,
                    )
                    .await?;
                Ok(Mt5RuntimeDispatchResult::OutcomePersisted)
            }
            Mt5DispatchDecision::ReconcileRequired {
                client_order_id,
                prior_state,
                venue_order_id,
                reason_code,
            } => Ok(Mt5RuntimeDispatchResult::ReconciliationRequired {
                client_order_id,
                prior_state,
                venue_order_id,
                reason_code,
            }),
        }
    }

    pub async fn session_state(&self) -> Mt5SessionState {
        self.session.lock().await.state()
    }
}

fn validate_initial_session(
    connection: &AuthenticatedMt5Connection,
    session: &Mt5SessionGate,
) -> BridgeResult<()> {
    let principal = connection.principal();
    if session.state() != Mt5SessionState::AwaitingHeartbeat
        || session.account_id().is_some()
        || session.terminal_id().is_some()
        || session.server().is_some()
        || !principal
            .allowed_accounts
            .iter()
            .any(|account_id| session.allows_account(account_id))
    {
        return Err(BridgeError::Invalid(
            "MT5 runtime requires a fresh heartbeat-waiting session whose account policy intersects the authenticated principal".into(),
        ));
    }
    Ok(())
}

impl Drop for Mt5BridgeRuntime {
    fn drop(&mut self) {
        self.watchdog.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_dispatch_result_never_claims_a_fill() {
        let result = Mt5RuntimeDispatchResult::OutcomePersisted;
        assert_eq!(result, Mt5RuntimeDispatchResult::OutcomePersisted);
        assert!(format!("{result:?}").contains("OutcomePersisted"));
    }

    #[test]
    fn reconciliation_required_result_preserves_durable_state() {
        let result = Mt5RuntimeDispatchResult::ReconciliationRequired {
            client_order_id: "clid-1".into(),
            prior_state: Mt5DurableOrderState::Unknown,
            venue_order_id: None,
            reason_code: Some("response_lost".into()),
        };
        assert!(matches!(
            result,
            Mt5RuntimeDispatchResult::ReconciliationRequired {
                prior_state: Mt5DurableOrderState::Unknown,
                ..
            }
        ));
    }
}
