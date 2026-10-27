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
use crate::mt5_reconciliation::{persist_and_confirm_reconciliation, Mt5ReconciliationSnapshot};
use crate::mt5_session::{Mt5SessionGate, Mt5SessionState};
use crate::mt5_transport::{spawn_heartbeat_watchdog, AuthenticatedMt5Connection};
use crate::mt5_v1::BridgeFrame;
use crate::BridgeResult;

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
            .read_frame_with_session_checkpoint(
                &self.store,
                &mut session,
                now,
                allowed_clock_skew,
            )
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
        persist_and_confirm_reconciliation(&self.store, &mut session, snapshot, now, max_age)
            .await
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
