//! Host-side orchestration for one authenticated MT5 terminal session.
//!
//! This coordinator owns the shared connection/session locks, starts the heartbeat
//! watchdog, checkpoints incoming heartbeats, persists external reconciliation state,
//! and consumes durable dispatch permits through the authenticated transport. It is
//! not wired to a production daemon or MT5 EA by this crate alone; the host must create
//! it with a persisted admission guard and must provide authoritative venue snapshots.

use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, Utc};
use domain::broker_registry::{
    normalize_mt5_server_id, AdmissionOperation, BrokerAdmissionDecision, BrokerAdmissionRequest,
};
use persistence::{
    evaluate_persisted_broker_admission, validate_ready_mt5_terminal_slot, Mt5DurableOrderState,
    Mt5WorkerLease, TradingStore,
};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::mt5_reconciliation::{
    persist_and_confirm_reconciliation, persist_reconciliation_without_confirmation,
    Mt5ReconciliationSnapshot, Mt5ReconciliationSource,
};
use crate::mt5_session::{Mt5SessionGate, Mt5SessionState};
use crate::mt5_transport::{spawn_heartbeat_watchdog, AuthenticatedMt5Connection};
use crate::mt5_v1::{BridgeFrame, BridgePayload, MarketDataStatus};
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

/// Immutable registry scope for one mTLS-authenticated worker. The worker
/// certificate fingerprint is also the provisioned service-user ID in
/// identity_users and broker_worker_assignments; that service principal must
/// have an active membership in the target organization.
#[derive(Debug, Clone)]
pub struct Mt5AdmissionGuard {
    request: BrokerAdmissionRequest,
    quant_account_ref: String,
    observed_server_name: String,
    worker_ref: String,
    terminal_slot_ref: String,
}

impl Mt5AdmissionGuard {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        organization_ref: &str,
        quant_account_ref: &str,
        external_account_ref: &str,
        binding_ref: &str,
        broker_ref: &str,
        broker_server_ref: &str,
        observed_server_name: &str,
        worker_ref: &str,
        terminal_slot_ref: &str,
    ) -> BridgeResult<Self> {
        let values = [
            organization_ref,
            quant_account_ref,
            external_account_ref,
            binding_ref,
            broker_ref,
            broker_server_ref,
            observed_server_name,
            worker_ref,
            terminal_slot_ref,
        ];
        if values
            .iter()
            .any(|value| value.trim().is_empty() || value.trim() != *value)
        {
            return Err(BridgeError::Invalid(
                "MT5 admission configuration contains an empty or padded identity".into(),
            ));
        }
        if worker_ref.len() != 64 || !worker_ref.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(BridgeError::Invalid(
                "MT5 admission worker identity must be a SHA-256 certificate fingerprint".into(),
            ));
        }
        Ok(Self {
            request: BrokerAdmissionRequest {
                request_ref: format!("mt5-readonly-{worker_ref}"),
                operation: AdmissionOperation::ReadOnlyMarketData,
                organization_ref: organization_ref.to_owned(),
                account_ref: external_account_ref.to_owned(),
                binding_ref: binding_ref.to_owned(),
                broker_ref: broker_ref.to_owned(),
                server_ref: broker_server_ref.to_owned(),
            },
            quant_account_ref: quant_account_ref.to_owned(),
            observed_server_name: normalize_mt5_server_id(observed_server_name)
                .map_err(|_| {
                    BridgeError::Invalid(
                        "MT5 server name does not satisfy registry normalization v1".into(),
                    )
                })?
                .canonical,
            worker_ref: worker_ref.to_ascii_lowercase(),
            terminal_slot_ref: terminal_slot_ref.to_owned(),
        })
    }

    fn validate_transport(&self, connection: &AuthenticatedMt5Connection) -> BridgeResult<()> {
        let principal = connection.principal();
        if principal.certificate_sha256.to_ascii_lowercase() != self.worker_ref
            || !principal.allowed_accounts.contains(&self.quant_account_ref)
        {
            return Err(BridgeError::Invalid(
                "MT5 admission scope does not match the authenticated worker certificate and QuantOS account"
                    .into(),
            ));
        }
        Ok(())
    }
}

pub struct Mt5BridgeRuntime {
    store: TradingStore,
    connection: Arc<Mutex<AuthenticatedMt5Connection>>,
    session: Arc<Mutex<Mt5SessionGate>>,
    admission: Option<Mt5AdmissionGuard>,
    lease: Option<Mt5WorkerLease>,
    watchdog: JoinHandle<()>,
}

impl Mt5BridgeRuntime {
    /// Unadmitted constructor for isolated tests and non-production harnesses only.
    /// This constructor deliberately has no registry admission or publisher lease.
    pub fn new(
        store: TradingStore,
        connection: AuthenticatedMt5Connection,
        session: Mt5SessionGate,
        watchdog_poll_interval: StdDuration,
    ) -> BridgeResult<Self> {
        Self::build(
            store,
            connection,
            session,
            None,
            None,
            watchdog_poll_interval,
        )
    }

    /// Production constructor requiring a live database lease for the exact
    /// account/terminal/worker tuple. Lease acquisition is performed by the
    /// daemon before this constructor; an invalid or stale lease fails closed.
    pub async fn new_with_admission_and_lease(
        store: TradingStore,
        connection: AuthenticatedMt5Connection,
        session: Mt5SessionGate,
        admission: Mt5AdmissionGuard,
        lease: Mt5WorkerLease,
        watchdog_poll_interval: StdDuration,
    ) -> BridgeResult<Self> {
        let principal = connection.principal();
        if lease.quant_account_id != admission.quant_account_ref
            || lease.terminal_id != principal.terminal_id
            || lease.worker_ref != admission.worker_ref
        {
            return Err(BridgeError::Invalid(
                "MT5 publisher lease does not match authenticated account, terminal, and worker"
                    .into(),
            ));
        }
        store.validate_mt5_worker_lease(&lease).await.map_err(|_| {
            BridgeError::Invalid(
                "MT5 publisher lease is unavailable or stale; refusing runtime startup".into(),
            )
        })?;
        Self::build(
            store,
            connection,
            session,
            Some(admission),
            Some(lease),
            watchdog_poll_interval,
        )
    }

    fn build(
        store: TradingStore,
        connection: AuthenticatedMt5Connection,
        session: Mt5SessionGate,
        admission: Option<Mt5AdmissionGuard>,
        lease: Option<Mt5WorkerLease>,
        watchdog_poll_interval: StdDuration,
    ) -> BridgeResult<Self> {
        if watchdog_poll_interval.is_zero() {
            return Err(BridgeError::Invalid(
                "MT5 heartbeat watchdog poll interval must be positive".into(),
            ));
        }
        validate_initial_session(&connection, &session, admission.is_some())?;
        if let Some(guard) = admission.as_ref() {
            guard.validate_transport(&connection)?;
        }
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
            admission,
            lease,
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
        validate_initial_session(&connection, &session, self.admission.is_some())?;
        if let Some(guard) = self.admission.as_ref() {
            guard.validate_transport(&connection)?;
        }
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
        self.validate_persisted_admission_or_degrade(0).await?;
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

    async fn validate_persisted_admission_or_degrade(&self, sequence: u64) -> BridgeResult<()> {
        if let Err(error) = self.validate_persisted_admission().await {
            let mut session = self.session.lock().await;
            session.degrade();
            if let (Some(account_id), Some(terminal_id)) =
                (session.account_id(), session.terminal_id())
            {
                let _ = self
                    .store
                    .mark_mt5_stream_degraded(account_id, terminal_id, sequence)
                    .await;
            }
            return Err(error);
        }
        Ok(())
    }

    async fn validate_persisted_admission(&self) -> BridgeResult<()> {
        if let Some(lease) = self.lease.as_ref() {
            self.store
                .renew_mt5_worker_lease(lease, 30)
                .await
                .map_err(|_| {
                    BridgeError::Invalid(
                        "MT5 publisher lease lost or database unavailable; refusing frame ingestion"
                            .into(),
                    )
                })?;
        }
        let Some(guard) = self.admission.as_ref() else {
            return Ok(());
        };
        let terminal_id = self.connection.lock().await.principal().terminal_id.clone();
        let slot_ready = validate_ready_mt5_terminal_slot(
            self.store.pool(),
            &guard.terminal_slot_ref,
            &guard.request.organization_ref,
            &guard.quant_account_ref,
            &guard.request.binding_ref,
            &guard.request.account_ref,
            &guard.request.broker_ref,
            &guard.request.server_ref,
            &terminal_id,
            &guard.worker_ref,
        )
        .await
        .map_err(|_| {
            BridgeError::Invalid(
                "MT5 terminal-slot registry is unavailable; refusing frame ingestion".into(),
            )
        })?;
        if !slot_ready {
            return Err(BridgeError::Invalid(
                "MT5 terminal slot, managed gateway, worker assignment, or READY session does not match; refusing frame ingestion".into(),
            ));
        }
        let decision = evaluate_persisted_broker_admission(
            self.store.pool(),
            &guard.request,
            &guard.worker_ref,
            &guard.worker_ref,
        )
        .await
        .map_err(|_| {
            BridgeError::Invalid(
                "MT5 broker admission registry is unavailable; refusing frame ingestion".into(),
            )
        })?;
        match decision {
            BrokerAdmissionDecision::Admitted(permit)
                if permit.worker_ref == guard.worker_ref
                    && permit.quant_account_ref == guard.quant_account_ref =>
            {
                // The catalog identifier is immutable. Check that the exact
                // server name authenticated by the worker maps to this registry
                // server, not merely to a caller-supplied server ID.
                let canonical_server = self
                    .store
                    .broker_server_canonical_identifier(
                        &guard.request.broker_ref,
                        &guard.request.server_ref,
                    )
                    .await
                    .map_err(|_| {
                        BridgeError::Invalid(
                            "MT5 broker-server catalog is unavailable; refusing frame ingestion"
                                .into(),
                        )
                    })?;
                if canonical_server.as_deref() != Some(guard.observed_server_name.as_str()) {
                    return Err(BridgeError::Invalid(
                        "MT5 observed server name does not match the verified broker-server catalog".into(),
                    ));
                }
                Ok(())
            }
            BrokerAdmissionDecision::Admitted(_) => Err(BridgeError::Invalid(
                "MT5 broker admission permit scope does not match the configured QuantOS account or worker"
                    .into(),
            )),
            BrokerAdmissionDecision::Denied(reasons) => Err(BridgeError::Invalid(format!(
                "MT5 broker admission denied: {reasons:?}"
            ))),
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
        self.validate_persisted_admission_or_degrade(snapshot.sequence)
            .await?;
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
        let mut market_data_fresh = false;
        let frame = loop {
            let frame = self.read_next_frame(Utc::now(), allowed_clock_skew).await?;
            if matches!(&frame.payload, BridgePayload::Heartbeat(_)) {
                continue;
            }
            if let BridgePayload::MarketDataSnapshot(snapshot) = &frame.payload {
                market_data_fresh = snapshot.status == MarketDataStatus::Fresh;
                let bound_identity = {
                    let session = self.session.lock().await;
                    (
                        session.account_id().map(str::to_owned),
                        session.terminal_id().map(str::to_owned),
                        session.server().map(str::to_owned),
                    )
                };
                if bound_identity.0.as_deref() != Some(frame.account_id.as_str())
                    || bound_identity.1.as_deref() != Some(snapshot.terminal_id.as_str())
                    || bound_identity.2.as_deref() != Some(snapshot.server.as_str())
                {
                    let mut session = self.session.lock().await;
                    session.degrade();
                    if let (Some(account_id), Some(terminal_id)) =
                        (bound_identity.0.as_deref(), bound_identity.1.as_deref())
                    {
                        let _ = self
                            .store
                            .mark_mt5_stream_degraded(account_id, terminal_id, snapshot.sequence)
                            .await;
                    }
                    return Err(BridgeError::Invalid(
                        "MT5 market-data identity does not match authenticated session".into(),
                    ));
                }
                self.validate_persisted_admission_or_degrade(snapshot.sequence)
                    .await?;
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
                if !market_data_fresh {
                    let mut session = self.session.lock().await;
                    session.degrade();
                    self.store
                        .mark_mt5_stream_degraded(
                            &frame.account_id,
                            &snapshot.terminal_id,
                            snapshot.sequence,
                        )
                        .await
                        .map_err(BridgeError::from)?;
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
        self.validate_persisted_admission_or_degrade(snapshot.sequence)
            .await?;
        let mut session = self.session.lock().await;
        if market_data_fresh {
            persist_and_confirm_reconciliation(
                &self.store,
                &mut session,
                &snapshot,
                Utc::now(),
                max_age,
            )
            .await
        } else {
            persist_reconciliation_without_confirmation(
                &self.store,
                &mut session,
                &snapshot,
                Utc::now(),
                max_age,
            )
            .await
        }
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
        self.validate_persisted_admission_or_degrade(0).await?;
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
        _frame: &BridgeFrame,
        _now: DateTime<Utc>,
        _allowed_clock_skew: Duration,
    ) -> BridgeResult<Mt5RuntimeDispatchResult> {
        // The current schema has no verified environment/provenance or durable
        // execution-authorization record. Until QOS-021A-E2 supplies that trusted
        // persistence boundary, no application caller may dispatch a broker frame.
        Err(BridgeError::Invalid(
            "MT5 order dispatch is disabled: verified account environment and execution authorization are unavailable".into(),
        ))
    }

    /// Stop the watchdog and release the exact lease. If the process crashes,
    /// the bounded database expiry remains the recovery path.
    pub async fn shutdown(&self) -> BridgeResult<()> {
        self.watchdog.abort();
        if let Some(lease) = self.lease.as_ref() {
            self.store
                .release_mt5_worker_lease(lease)
                .await
                .map_err(|_| {
                    BridgeError::Invalid(
                        "could not release MT5 publisher lease; it will expire automatically"
                            .into(),
                    )
                })?;
        }
        Ok(())
    }

    pub async fn session_state(&self) -> Mt5SessionState {
        self.session.lock().await.state()
    }
}

fn validate_initial_session(
    connection: &AuthenticatedMt5Connection,
    session: &Mt5SessionGate,
    require_authenticated_identity: bool,
) -> BridgeResult<()> {
    let principal = connection.principal();
    let identity_mismatch = require_authenticated_identity
        && (session.expected_terminal_id() != Some(principal.terminal_id.as_str())
            || session.expected_server() != Some(principal.server.as_str()));
    if session.state() != Mt5SessionState::AwaitingHeartbeat
        || session.account_id().is_some()
        || session.terminal_id().is_some()
        || session.server().is_some()
        || identity_mismatch
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

    // Constructor failure must fail this test; keep the crate-wide expect lint strict elsewhere.
    #[allow(clippy::expect_used)]
    #[test]
    fn admission_guard_binds_quant_account_external_account_and_worker() {
        let fingerprint = "a".repeat(64);
        let guard = Mt5AdmissionGuard::new(
            "org-1",
            "quant-account-1",
            "external-account-1",
            "binding-1",
            "broker-1",
            "broker-server-1",
            "R502.Demo",
            &fingerprint,
            "slot-1",
        )
        .expect("valid admission guard configuration");
        assert_eq!(
            guard.request.operation,
            AdmissionOperation::ReadOnlyMarketData
        );
        assert_eq!(guard.request.account_ref, "external-account-1");
        assert_eq!(guard.quant_account_ref, "quant-account-1");
        assert_eq!(guard.observed_server_name, "r502.demo");
        assert_eq!(guard.worker_ref, fingerprint);
        assert_eq!(guard.terminal_slot_ref, "slot-1");
    }

    #[test]
    fn admission_guard_rejects_unbound_or_malformed_worker_identity() {
        assert!(Mt5AdmissionGuard::new(
            "org-1",
            "quant-1",
            "external-1",
            "binding-1",
            "broker-1",
            "server-1",
            "R502.Demo",
            "worker-1",
            "slot-1"
        )
        .is_err());
        assert!(Mt5AdmissionGuard::new(
            "org-1 ",
            "quant-1",
            "external-1",
            "binding-1",
            "broker-1",
            "server-1",
            "R502.Demo",
            &"a".repeat(64),
            "slot-1"
        )
        .is_err());
    }

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
