//! Durable recovery from an authoritative MT5 account snapshot.
//!
//! Reconciliation readiness is restored only after a fresh, identity-bound snapshot
//! containing account state, open orders and positions is durably stored together
//! with its sequence checkpoint. This module does not query MT5 itself; the host must
//! supply a snapshot fetched from the authenticated venue adapter/EA.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, Utc};
use persistence::{Mt5StreamReadiness, TradingStore};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::mt5_session::{Mt5SessionGate, Mt5SessionState};
use crate::{BridgeError, BridgeResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mt5AccountStateSnapshot {
    pub currency: String,
    pub balance: String,
    pub equity: String,
    pub margin_used: String,
    pub free_margin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mt5OpenOrderSnapshot {
    pub venue_order_id: String,
    pub client_order_id: Option<String>,
    pub instrument_id: String,
    pub side: String,
    pub order_type: String,
    pub quantity: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mt5PositionSnapshot {
    pub venue_position_id: String,
    pub instrument_id: String,
    pub side: String,
    pub quantity: String,
    pub average_price: String,
    pub unrealized_pnl: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mt5ReconciliationSnapshot {
    pub account_id: String,
    pub terminal_id: String,
    pub server: String,
    pub source: String,
    pub sequence: u64,
    pub captured_at: DateTime<Utc>,
    pub account_state: Mt5AccountStateSnapshot,
    pub open_orders: Vec<Mt5OpenOrderSnapshot>,
    pub positions: Vec<Mt5PositionSnapshot>,
}

/// Adapter contract for fetching an authoritative account/order/position snapshot
/// from the authenticated MT5 EA/client. Production implementations must query the
/// venue rather than return cached strategy state or a locally inferred position set.
#[async_trait::async_trait]
pub trait Mt5ReconciliationSource: Send + Sync {
    async fn fetch_snapshot(
        &self,
        account_id: &str,
        terminal_id: &str,
        server: &str,
    ) -> BridgeResult<Mt5ReconciliationSnapshot>;
}

impl Mt5ReconciliationSnapshot {
    pub fn validate(&self, now: DateTime<Utc>, max_age: Duration) -> BridgeResult<()> {
        if self.account_id.trim().is_empty()
            || self.account_id.trim() != self.account_id
            || self.terminal_id.trim().is_empty()
            || self.terminal_id.trim() != self.terminal_id
            || self.server.trim().is_empty()
            || self.server.trim() != self.server
            || self.source.trim().is_empty()
            || self.source.trim() != self.source
            || max_age < Duration::zero()
            || self.captured_at > now
            || now.signed_duration_since(self.captured_at) > max_age
            || self.account_state.currency.trim().is_empty()
            || self.account_state.currency.trim() != self.account_state.currency
        {
            return Err(BridgeError::Invalid(
                "MT5 reconciliation snapshot identity, sequence, source, currency or freshness is invalid".into(),
            ));
        }
        for amount in [
            &self.account_state.balance,
            &self.account_state.equity,
            &self.account_state.margin_used,
            &self.account_state.free_margin,
        ] {
            parse_decimal(amount, "account-state amount")?;
        }

        let mut order_ids = BTreeSet::new();
        for order in &self.open_orders {
            if order.venue_order_id.trim().is_empty()
                || order.instrument_id.trim().is_empty()
                || order.side.trim().is_empty()
                || order.order_type.trim().is_empty()
                || order.status.trim().is_empty()
                || !order_ids.insert(order.venue_order_id.as_str())
            {
                return Err(BridgeError::Invalid(
                    "MT5 reconciliation contains an invalid or duplicate external order".into(),
                ));
            }
            if order
                .client_order_id
                .as_deref()
                .is_some_and(|id| id.trim().is_empty())
            {
                return Err(BridgeError::Invalid(
                    "MT5 reconciliation contains an empty client order ID".into(),
                ));
            }
            if parse_decimal(&order.quantity, "open-order quantity")? <= Decimal::ZERO {
                return Err(BridgeError::Invalid(
                    "MT5 open-order quantity must be positive".into(),
                ));
            }
        }

        let mut position_ids = BTreeSet::new();
        for position in &self.positions {
            if position.venue_position_id.trim().is_empty()
                || position.instrument_id.trim().is_empty()
                || position.side.trim().is_empty()
                || !position_ids.insert(position.venue_position_id.as_str())
            {
                return Err(BridgeError::Invalid(
                    "MT5 reconciliation contains an invalid or duplicate external position".into(),
                ));
            }
            if parse_decimal(&position.quantity, "position quantity")? <= Decimal::ZERO {
                return Err(BridgeError::Invalid(
                    "MT5 position quantity must be positive".into(),
                ));
            }
            parse_decimal(&position.average_price, "position average price")?;
            parse_decimal(&position.unrealized_pnl, "position unrealized PnL")?;
        }
        Ok(())
    }
}

/// Persist authoritative external state and then release the session's reconciliation
/// latch. This requires the caller to have fetched the snapshot from the authenticated
/// venue and to supply all open orders, positions and account state—not a partial cache.
pub async fn persist_and_confirm_reconciliation(
    store: &TradingStore,
    session: &mut Mt5SessionGate,
    snapshot: &Mt5ReconciliationSnapshot,
    now: DateTime<Utc>,
    max_age: Duration,
) -> BridgeResult<()> {
    if let Err(error) = snapshot.validate(now, max_age) {
        degrade_session_and_stream(store, session).await;
        return Err(error);
    }
    if session.state() != Mt5SessionState::ReconciliationRequired
        || session.account_id() != Some(snapshot.account_id.as_str())
        || session.terminal_id() != Some(snapshot.terminal_id.as_str())
        || session.server() != Some(snapshot.server.as_str())
        || !session.capabilities().is_some_and(|caps| {
            caps.order_lookup && caps.sequence_numbers
        })
    {
        degrade_session_and_stream(store, session).await;
        return Err(BridgeError::Invalid(
            "MT5 external snapshot does not match a connected session awaiting reconciliation"
                .into(),
        ));
    }

    let value = match serde_json::to_value(snapshot) {
        Ok(value) => value,
        Err(error) => {
            degrade_session_and_stream(store, session).await;
            return Err(BridgeError::from(error));
        }
    };
    if let Err(error) = store
        .persist_mt5_reconciliation_snapshot(
            &snapshot.account_id,
            &snapshot.terminal_id,
            snapshot.sequence,
            &snapshot.source,
            snapshot.captured_at,
            value,
        )
        .await
    {
        degrade_session_and_stream(store, session).await;
        return Err(BridgeError::from(error));
    }

    if let Err(error) = session.confirm_reconciled(Mt5StreamReadiness::Ready) {
        degrade_session_and_stream(store, session).await;
        return Err(error);
    }
    Ok(())
}

async fn degrade_session_and_stream(store: &TradingStore, session: &mut Mt5SessionGate) {
    let account_id = session.account_id().map(str::to_owned);
    let terminal_id = session.terminal_id().map(str::to_owned);
    session.degrade();
    if let (Some(account_id), Some(terminal_id)) = (account_id, terminal_id) {
        let _ = store
            .mark_mt5_stream_degraded(&account_id, &terminal_id, 0)
            .await;
    }
}

fn parse_decimal(value: &str, field: &str) -> BridgeResult<Decimal> {
    if value.trim().is_empty() || value.trim() != value {
        return Err(BridgeError::Invalid(format!(
            "MT5 reconciliation {field} must be an exact base-10 decimal string"
        )));
    }
    Decimal::from_str_exact(value).map_err(|_| {
        BridgeError::Invalid(format!(
            "MT5 reconciliation {field} must be an exact base-10 decimal string"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn snapshot(now: DateTime<Utc>) -> Mt5ReconciliationSnapshot {
        Mt5ReconciliationSnapshot {
            account_id: "account-001".into(),
            terminal_id: "terminal-01".into(),
            server: "broker-demo".into(),
            source: "mt5_authoritative_snapshot_v1".into(),
            sequence: 42,
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

    #[test]
    fn reconciliation_requires_fresh_complete_external_state() {
        let now = Utc::now();
        let mut valid = snapshot(now);
        assert!(valid.validate(now, Duration::seconds(30)).is_ok());
        valid.open_orders[0].quantity = "0".into();
        assert!(valid.validate(now, Duration::seconds(30)).is_err());

        let mut stale = snapshot(now - Duration::seconds(31));
        assert!(stale.validate(now, Duration::seconds(30)).is_err());
        stale.captured_at = now + Duration::seconds(1);
        assert!(stale.validate(now, Duration::seconds(30)).is_err());
    }

    #[test]
    fn reconciliation_rejects_duplicate_external_order_and_position_ids() {
        let now = Utc::now();
        let mut duplicate_order = snapshot(now);
        duplicate_order
            .open_orders
            .push(duplicate_order.open_orders[0].clone());
        assert!(duplicate_order
            .validate(now, Duration::seconds(30))
            .is_err());

        let mut duplicate_position = snapshot(now);
        duplicate_position
            .positions
            .push(duplicate_position.positions[0].clone());
        assert!(duplicate_position
            .validate(now, Duration::seconds(30))
            .is_err());
    }
}
