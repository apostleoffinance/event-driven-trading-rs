//! Transport-independent MT5 session identity and readiness gate.
//!
//! The concrete transport must authenticate its peer before creating this gate.
//! This module binds that authenticated connection to one allow-listed account,
//! terminal and server, then fails closed on identity/capability/sequence faults.

use std::collections::BTreeSet;

use persistence::{Mt5SequenceObservation, Mt5StreamReadiness};

use crate::mt5_v1::{
    BridgeCapabilities, BridgeFrame, BridgePayload, ConnectionState, OrderType, SubmitOrder,
};
use crate::{BridgeError, BridgeResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mt5SessionState {
    AwaitingHeartbeat,
    ReconciliationRequired,
    Ready,
    Degraded,
    Closed,
}

/// Immutable policy supplied by the host service, never by a terminal frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mt5SessionPolicy {
    allowed_accounts: BTreeSet<String>,
}

impl Mt5SessionPolicy {
    pub fn new<I, S>(allowed_accounts: I) -> BridgeResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let accounts: BTreeSet<String> = allowed_accounts.into_iter().map(Into::into).collect();
        if accounts.is_empty()
            || accounts
                .iter()
                .any(|account| account.trim().is_empty() || account.trim() != account)
        {
            return Err(BridgeError::Invalid(
                "MT5 session policy requires non-empty allow-listed account IDs".into(),
            ));
        }
        Ok(Self {
            allowed_accounts: accounts,
        })
    }

    pub fn allows_account(&self, account_id: &str) -> bool {
        self.allowed_accounts.contains(account_id)
    }
}

/// Per-connection state. It cannot change its account/terminal/server binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mt5SessionGate {
    policy: Mt5SessionPolicy,
    account_id: Option<String>,
    terminal_id: Option<String>,
    server: Option<String>,
    capabilities: Option<BridgeCapabilities>,
    connection_state: Option<ConnectionState>,
    state: Mt5SessionState,
}

impl Mt5SessionGate {
    /// Create only after the transport has authenticated its peer. This method
    /// does not itself authenticate a terminal or protect an unauthenticated socket.
    pub fn after_transport_authentication(policy: Mt5SessionPolicy) -> Self {
        Self {
            policy,
            account_id: None,
            terminal_id: None,
            server: None,
            capabilities: None,
            connection_state: None,
            state: Mt5SessionState::AwaitingHeartbeat,
        }
    }

    pub fn state(&self) -> Mt5SessionState {
        self.state
    }

    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    pub fn terminal_id(&self) -> Option<&str> {
        self.terminal_id.as_deref()
    }

    pub fn server(&self) -> Option<&str> {
        self.server.as_deref()
    }

    pub fn capabilities(&self) -> Option<&BridgeCapabilities> {
        self.capabilities.as_ref()
    }

    /// Consume a validated heartbeat alongside the result of the durable
    /// sequence-checkpoint API. A fresh/gapped/degraded stream never becomes
    /// ready from a heartbeat alone; reconciliation must happen separately.
    pub fn observe_heartbeat(
        &mut self,
        frame: &BridgeFrame,
        sequence_observation: Mt5SequenceObservation,
        persisted_readiness: Mt5StreamReadiness,
    ) -> BridgeResult<Mt5SessionState> {
        if self.state == Mt5SessionState::Closed {
            return Err(BridgeError::Invalid("MT5 session is closed".into()));
        }
        let heartbeat = match &frame.payload {
            BridgePayload::Heartbeat(value) => value,
            _ => return Err(BridgeError::Invalid("expected MT5 heartbeat frame".into())),
        };

        if frame.account_id.trim().is_empty()
            || heartbeat.terminal_id.trim().is_empty()
            || heartbeat.server.trim().is_empty()
        {
            self.state = Mt5SessionState::Closed;
            return Err(BridgeError::Invalid(
                "MT5 account, terminal and server identities must be non-empty and unpadded".into(),
            ));
        }

        if !self.policy.allows_account(&frame.account_id) {
            self.state = Mt5SessionState::Closed;
            return Err(BridgeError::Invalid(
                "MT5 heartbeat account is not allow-listed".into(),
            ));
        }

        if let Some(bound) = self.account_id.as_deref() {
            if bound != frame.account_id
                || self.terminal_id.as_deref() != Some(heartbeat.terminal_id.as_str())
                || self.server.as_deref() != Some(heartbeat.server.as_str())
            {
                self.state = Mt5SessionState::Closed;
                return Err(BridgeError::Invalid(
                    "MT5 connection identity changed after session binding".into(),
                ));
            }
        } else {
            self.account_id = Some(frame.account_id.clone());
            self.terminal_id = Some(heartbeat.terminal_id.clone());
            self.server = Some(heartbeat.server.clone());
        }

        self.capabilities = Some(heartbeat.capabilities.clone());
        self.connection_state = Some(heartbeat.connection);
        if heartbeat.connection != ConnectionState::Connected
            || !capabilities_support_safe_dispatch(&heartbeat.capabilities)
        {
            self.state = Mt5SessionState::Degraded;
            return Ok(self.state);
        }

        // A fresh session or a previously degraded session must pass through
        // explicit reconciliation; a heartbeat cannot clear that safety latch.
        if self.state != Mt5SessionState::Ready
            || sequence_observation != Mt5SequenceObservation::Accepted
            || persisted_readiness != Mt5StreamReadiness::Ready
        {
            self.state = Mt5SessionState::ReconciliationRequired;
        }
        Ok(self.state)
    }

    /// Restore readiness only after the caller has reconciled external orders,
    /// positions and account state and persisted the checkpoint. A heartbeat
    /// or reconnect alone must never call this method.
    pub fn confirm_reconciled(
        &mut self,
        persisted_readiness: Mt5StreamReadiness,
    ) -> BridgeResult<()> {
        if self.state == Mt5SessionState::Closed {
            return Err(BridgeError::Invalid("MT5 session is closed".into()));
        }
        if self.account_id.is_none()
            || self.terminal_id.is_none()
            || self.server.is_none()
            || self.connection_state != Some(ConnectionState::Connected)
            || !self
                .capabilities
                .as_ref()
                .is_some_and(capabilities_support_safe_dispatch)
            || persisted_readiness != Mt5StreamReadiness::Ready
        {
            self.state = Mt5SessionState::ReconciliationRequired;
            return Err(BridgeError::Invalid(
                "MT5 session cannot become ready before identity, capabilities, connection and durable reconciliation are valid".into(),
            ));
        }
        self.state = Mt5SessionState::Ready;
        Ok(())
    }

    /// Check a submit against the bound account, readiness and declared order
    /// capabilities. This is a gate only; it does not replace risk/OMS checks.
    pub fn authorize_submit<'a>(&self, frame: &'a BridgeFrame) -> BridgeResult<&'a SubmitOrder> {
        self.require_ready_account(&frame.account_id)?;
        let order = match &frame.payload {
            BridgePayload::SubmitOrder(value) => value,
            _ => return Err(BridgeError::Invalid("expected submit_order frame".into())),
        };
        let capabilities = self.capabilities.as_ref().ok_or_else(|| {
            BridgeError::Invalid("MT5 capabilities have not been observed".into())
        })?;
        let supported = match order.order_type {
            OrderType::Market => capabilities.market_orders,
            OrderType::Limit => capabilities.limit_orders,
            OrderType::Stop | OrderType::StopLimit => capabilities.stop_orders,
        };
        if !supported {
            return Err(BridgeError::Invalid(format!(
                "MT5 terminal does not declare support for {:?} orders",
                order.order_type
            )));
        }
        Ok(order)
    }

    pub fn authorize_cancel(&self, frame: &BridgeFrame) -> BridgeResult<()> {
        self.require_ready_account(&frame.account_id)?;
        if !matches!(&frame.payload, BridgePayload::CancelOrder(_)) {
            return Err(BridgeError::Invalid("expected cancel_order frame".into()));
        }
        if !self
            .capabilities
            .as_ref()
            .is_some_and(|caps| caps.cancel_requests)
        {
            return Err(BridgeError::Invalid(
                "MT5 terminal does not declare cancellation support".into(),
            ));
        }
        Ok(())
    }

    /// Any transport loss, malformed frame, heartbeat timeout or uncertain
    /// state must call this before accepting further exposure.
    pub fn degrade(&mut self) {
        if self.state != Mt5SessionState::Closed {
            self.connection_state = Some(ConnectionState::Degraded);
            self.state = Mt5SessionState::Degraded;
        }
    }

    pub fn close(&mut self) {
        self.state = Mt5SessionState::Closed;
    }

    fn require_ready_account(&self, account_id: &str) -> BridgeResult<()> {
        if self.state != Mt5SessionState::Ready {
            return Err(BridgeError::Invalid(format!(
                "MT5 session is not ready for execution: {:?}",
                self.state
            )));
        }
        if self.account_id.as_deref() != Some(account_id) {
            return Err(BridgeError::Invalid(
                "MT5 frame account does not match bound session account".into(),
            ));
        }
        Ok(())
    }
}

fn capabilities_support_safe_dispatch(capabilities: &BridgeCapabilities) -> bool {
    capabilities.order_lookup
        && capabilities.idempotent_client_order_id
        && capabilities.sequence_numbers
        && (capabilities.market_orders || capabilities.limit_orders || capabilities.stop_orders)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::mt5_v1::{
        CancelOrder, Heartbeat, OrderSide, SubmitOrder, TimeInForce, MT5_BRIDGE_SCHEMA_VERSION,
    };
    use chrono::{TimeZone, Utc};

    fn capabilities() -> BridgeCapabilities {
        BridgeCapabilities {
            market_orders: true,
            limit_orders: true,
            stop_orders: true,
            cancel_requests: true,
            order_lookup: true,
            idempotent_client_order_id: true,
            sequence_numbers: true,
        }
    }

    fn heartbeat(
        account: &str,
        terminal: &str,
        server: &str,
        connection: ConnectionState,
    ) -> BridgeFrame {
        BridgeFrame {
            schema_version: MT5_BRIDGE_SCHEMA_VERSION,
            message_id: "message-1".into(),
            correlation_id: "correlation-1".into(),
            account_id: account.into(),
            sent_at: Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap(),
            payload: BridgePayload::Heartbeat(Heartbeat {
                sequence: 2,
                terminal_id: terminal.into(),
                server: server.into(),
                terminal_time: Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap(),
                connection,
                capabilities: capabilities(),
            }),
        }
    }

    fn ready_gate() -> Mt5SessionGate {
        let policy = Mt5SessionPolicy::new(["acct-1"]).unwrap();
        let mut gate = Mt5SessionGate::after_transport_authentication(policy);
        let frame = heartbeat(
            "acct-1",
            "terminal-1",
            "demo-server",
            ConnectionState::Connected,
        );
        assert_eq!(
            gate.observe_heartbeat(
                &frame,
                Mt5SequenceObservation::Accepted,
                Mt5StreamReadiness::Ready,
            )
            .unwrap(),
            Mt5SessionState::ReconciliationRequired
        );
        gate.confirm_reconciled(Mt5StreamReadiness::Ready).unwrap();
        gate
    }

    fn submit(account: &str) -> BridgeFrame {
        BridgeFrame {
            schema_version: MT5_BRIDGE_SCHEMA_VERSION,
            message_id: "message-2".into(),
            correlation_id: "correlation-1".into(),
            account_id: account.into(),
            sent_at: Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 1).unwrap(),
            payload: BridgePayload::SubmitOrder(SubmitOrder {
                client_order_id: "stable-order-1".into(),
                instrument_id: "EURUSD.a".into(),
                side: OrderSide::Buy,
                order_type: OrderType::Market,
                quantity: "0.01".into(),
                price: None,
                stop_loss: Some("1.08000".into()),
                take_profit: Some("1.09000".into()),
                time_in_force: TimeInForce::Day,
            }),
        }
    }

    #[test]
    fn fresh_or_gapped_stream_cannot_enable_submit() {
        let policy = Mt5SessionPolicy::new(["acct-1"]).unwrap();
        let mut gate = Mt5SessionGate::after_transport_authentication(policy);
        let frame = heartbeat(
            "acct-1",
            "terminal-1",
            "demo-server",
            ConnectionState::Connected,
        );
        gate.observe_heartbeat(
            &frame,
            Mt5SequenceObservation::ReconciliationRequired,
            Mt5StreamReadiness::Degraded,
        )
        .unwrap();
        assert!(gate.authorize_submit(&submit("acct-1")).is_err());
    }

    #[test]
    fn ready_session_accepts_supported_submit_and_cancel() {
        let gate = ready_gate();
        assert!(gate.authorize_submit(&submit("acct-1")).is_ok());
        let cancel = BridgeFrame {
            schema_version: MT5_BRIDGE_SCHEMA_VERSION,
            message_id: "message-3".into(),
            correlation_id: "correlation-1".into(),
            account_id: "acct-1".into(),
            sent_at: Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 1).unwrap(),
            payload: BridgePayload::CancelOrder(CancelOrder {
                client_order_id: "stable-order-1".into(),
                venue_order_id: None,
            }),
        };
        assert!(gate.authorize_cancel(&cancel).is_ok());
    }

    #[test]
    fn account_switch_and_terminal_rebinding_close_session() {
        let policy = Mt5SessionPolicy::new(["acct-1", "acct-2"]).unwrap();
        let mut gate = Mt5SessionGate::after_transport_authentication(policy);
        let first = heartbeat(
            "acct-1",
            "terminal-1",
            "demo-server",
            ConnectionState::Connected,
        );
        gate.observe_heartbeat(
            &first,
            Mt5SequenceObservation::ReconciliationRequired,
            Mt5StreamReadiness::Degraded,
        ).unwrap();
        let switched = heartbeat(
            "acct-2",
            "terminal-1",
            "demo-server",
            ConnectionState::Connected,
        );
        assert!(gate
            .observe_heartbeat(
                &switched,
                Mt5SequenceObservation::Accepted,
                Mt5StreamReadiness::Ready,
            )
            .is_err());
        assert_eq!(gate.state(), Mt5SessionState::Closed);
    }

    #[test]
    fn disconnect_and_degrade_block_execution_until_reconciled() {
        let mut gate = ready_gate();
        gate.degrade();
        assert!(gate.authorize_submit(&submit("acct-1")).is_err());
        assert!(gate.confirm_reconciled(Mt5StreamReadiness::Ready).is_err());
        let connected = heartbeat(
            "acct-1",
            "terminal-1",
            "demo-server",
            ConnectionState::Connected,
        );
        assert_eq!(
            gate.observe_heartbeat(
                &connected,
                Mt5SequenceObservation::Accepted,
                Mt5StreamReadiness::Ready,
            ).unwrap(),
            Mt5SessionState::ReconciliationRequired
        );
        assert!(gate.authorize_submit(&submit("acct-1")).is_err());
        gate.confirm_reconciled(Mt5StreamReadiness::Ready).unwrap();
        assert!(gate.authorize_submit(&submit("acct-1")).is_ok());
        assert!(gate.authorize_submit(&submit("acct-2")).is_err());
        assert!(gate
            .confirm_reconciled(Mt5StreamReadiness::Degraded)
            .is_err());
    }

    #[test]
    fn non_allowlisted_account_and_missing_safe_capabilities_fail_closed() {
        let policy = Mt5SessionPolicy::new(["acct-1"]).unwrap();
        let mut gate = Mt5SessionGate::after_transport_authentication(policy);
        let unauthorized = heartbeat(
            "acct-evil",
            "terminal-1",
            "demo-server",
            ConnectionState::Connected,
        );
        assert!(gate
            .observe_heartbeat(
                &unauthorized,
                Mt5SequenceObservation::Accepted,
                Mt5StreamReadiness::Ready,
            )
            .is_err());
        assert_eq!(gate.state(), Mt5SessionState::Closed);

        let mut gate = ready_gate();
        let mut missing = heartbeat(
            "acct-1",
            "terminal-1",
            "demo-server",
            ConnectionState::Connected,
        );
        if let BridgePayload::Heartbeat(ref mut hb) = missing.payload {
            hb.capabilities.order_lookup = false;
        }
        assert_eq!(
            gate.observe_heartbeat(
                &missing,
                Mt5SequenceObservation::Accepted,
                Mt5StreamReadiness::Ready,
            ).unwrap(),
            Mt5SessionState::Degraded
        );
        assert!(gate.authorize_submit(&submit("acct-1")).is_err());
    }
}
