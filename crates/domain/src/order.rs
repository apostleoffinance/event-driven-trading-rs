//! Order domain model and state machine.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::{DomainError, DomainResult};
use crate::ids::{
    AccountId, ClientOrderId, DeploymentId, InstrumentId, OrderId, StrategyId, TradeIntentId,
    VenueId,
};
use crate::money::{require_non_negative, require_positive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    Day,
    Gtc,
    Ioc,
    Fok,
}

/// Institutional order lifecycle states.
///
/// [`OrderStatus::Unknown`] means venue outcome is ambiguous (e.g. timeout after
/// submit). It is **not** terminal — reconciliation must resolve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderStatus {
    Created,
    PendingRisk,
    Approved,
    Rejected,
    Submitted,
    /// Venue response ambiguous; do not treat as Failed.
    Unknown,
    Accepted,
    PartiallyFilled,
    Filled,
    CancelPending,
    Cancelled,
    Failed,
}

impl OrderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::PendingRisk => "PENDING_RISK",
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
            Self::Submitted => "SUBMITTED",
            Self::Unknown => "UNKNOWN",
            Self::Accepted => "ACCEPTED",
            Self::PartiallyFilled => "PARTIALLY_FILLED",
            Self::Filled => "FILLED",
            Self::CancelPending => "CANCEL_PENDING",
            Self::Cancelled => "CANCELLED",
            Self::Failed => "FAILED",
        }
    }

    /// Whether a transition from `self` to `to` is legal.
    pub fn can_transition_to(self, to: Self) -> bool {
        use OrderStatus::*;
        matches!(
            (self, to),
            (Created, PendingRisk)
                | (Created, Rejected)
                | (Created, Failed)
                | (PendingRisk, Approved)
                | (PendingRisk, Rejected)
                | (PendingRisk, Failed)
                | (Approved, Submitted)
                | (Approved, Rejected)
                | (Approved, Failed)
                | (Submitted, Accepted)
                | (Submitted, Rejected)
                | (Submitted, Failed)
                | (Submitted, Unknown)
                | (Submitted, CancelPending)
                // Reconciliation resolves Unknown — never guess Failed without evidence.
                | (Unknown, Accepted)
                | (Unknown, Rejected)
                | (Unknown, Cancelled)
                | (Unknown, PartiallyFilled)
                | (Unknown, Filled)
                | (Unknown, Failed)
                | (Accepted, PartiallyFilled)
                | (Accepted, Filled)
                | (Accepted, CancelPending)
                | (Accepted, Failed)
                | (PartiallyFilled, PartiallyFilled)
                | (PartiallyFilled, Filled)
                | (PartiallyFilled, CancelPending)
                | (PartiallyFilled, Failed)
                | (CancelPending, Cancelled)
                | (CancelPending, PartiallyFilled)
                | (CancelPending, Filled)
                | (CancelPending, Failed)
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Rejected | Self::Filled | Self::Cancelled | Self::Failed
        )
    }

    /// Ambiguous venue outcome — requires reconciliation, not silent recovery.
    pub fn is_unknown(self) -> bool {
        matches!(self, Self::Unknown)
    }
}

/// Executable order after risk approval. Always carries account + venue + client id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub client_order_id: ClientOrderId,
    pub account_id: AccountId,
    pub venue_id: VenueId,
    pub deployment_id: DeploymentId,
    pub strategy_id: StrategyId,
    pub trade_intent_id: Option<TradeIntentId>,
    pub instrument_id: InstrumentId,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub filled_quantity: Decimal,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
}

impl Order {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        id: OrderId,
        client_order_id: ClientOrderId,
        account_id: AccountId,
        venue_id: VenueId,
        deployment_id: DeploymentId,
        strategy_id: StrategyId,
        trade_intent_id: Option<TradeIntentId>,
        instrument_id: InstrumentId,
        side: OrderSide,
        order_type: OrderType,
        time_in_force: TimeInForce,
        quantity: Decimal,
        price: Option<Decimal>,
        created_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        let quantity = require_positive(quantity, "quantity")?;
        if let Some(px) = price {
            require_positive(px, "price")?;
        }
        Ok(Self {
            id,
            client_order_id,
            account_id,
            venue_id,
            deployment_id,
            strategy_id,
            trade_intent_id,
            instrument_id,
            side,
            order_type,
            time_in_force,
            quantity,
            price,
            filled_quantity: Decimal::ZERO,
            status: OrderStatus::Created,
            created_at,
            updated_at: created_at,
            submitted_at: None,
            filled_at: None,
        })
    }

    pub fn transition_to(&mut self, next: OrderStatus, at: DateTime<Utc>) -> DomainResult<()> {
        if !self.status.can_transition_to(next) {
            return Err(DomainError::IllegalOrderTransition {
                from: self.status.as_str().to_string(),
                to: next.as_str().to_string(),
            });
        }
        if next == OrderStatus::Submitted {
            self.submitted_at = Some(at);
        }
        if next == OrderStatus::Filled {
            self.filled_at = Some(at);
        }
        self.status = next;
        self.updated_at = at;
        Ok(())
    }

    pub fn apply_fill(&mut self, fill_qty: Decimal, at: DateTime<Utc>) -> DomainResult<()> {
        let fill_qty = require_positive(fill_qty, "fill_qty")?;
        let new_filled = self.filled_quantity + fill_qty;
        if new_filled > self.quantity {
            return Err(DomainError::Invariant(format!(
                "fill would exceed order quantity: filled={new_filled} qty={}",
                self.quantity
            )));
        }
        self.filled_quantity = require_non_negative(new_filled, "filled_quantity")?;
        let next = if self.filled_quantity == self.quantity {
            OrderStatus::Filled
        } else {
            OrderStatus::PartiallyFilled
        };

        // From Accepted or PartiallyFilled (or CancelPending residual fills).
        if self.status == OrderStatus::Accepted
            || self.status == OrderStatus::PartiallyFilled
            || self.status == OrderStatus::CancelPending
        {
            self.transition_to(next, at)?;
            Ok(())
        } else {
            Err(DomainError::IllegalOrderTransition {
                from: self.status.as_str().to_string(),
                to: next.as_str().to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::ids::*;
    use rust_decimal::Decimal;

    fn sample_order() -> Order {
        Order::create(
            OrderId::new("ord-1").unwrap(),
            ClientOrderId::new("clid-1").unwrap(),
            AccountId::new("prop-account-001").unwrap(),
            VenueId::new("simulated").unwrap(),
            DeploymentId::new("deployment-001").unwrap(),
            StrategyId::new("btc-mean-reversion").unwrap(),
            Some(TradeIntentId::new("ti-1").unwrap()),
            InstrumentId::new("BTCUSDT").unwrap(),
            OrderSide::Buy,
            OrderType::Market,
            TimeInForce::Ioc,
            Decimal::from(2),
            Some(Decimal::from(100)),
            Utc::now(),
        )
        .unwrap()
    }

    #[test]
    fn valid_happy_path_transitions() {
        let mut order = sample_order();
        let now = Utc::now();
        order.transition_to(OrderStatus::PendingRisk, now).unwrap();
        order.transition_to(OrderStatus::Approved, now).unwrap();
        order.transition_to(OrderStatus::Submitted, now).unwrap();
        order.transition_to(OrderStatus::Accepted, now).unwrap();
        order.apply_fill(Decimal::ONE, now).unwrap();
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        order.apply_fill(Decimal::ONE, now).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert!(order.status.is_terminal());
    }

    #[test]
    fn rejects_illegal_transition() {
        let mut order = sample_order();
        let err = order
            .transition_to(OrderStatus::Filled, Utc::now())
            .unwrap_err();
        assert!(matches!(err, DomainError::IllegalOrderTransition { .. }));
    }

    #[test]
    fn create_requires_account_and_venue_ids() {
        let order = sample_order();
        assert_eq!(order.account_id.as_str(), "prop-account-001");
        assert_eq!(order.venue_id.as_str(), "simulated");
        assert_eq!(order.client_order_id.as_str(), "clid-1");
    }

    #[test]
    fn unknown_resolves_via_recon_transitions() {
        let mut order = sample_order();
        let now = Utc::now();
        order.transition_to(OrderStatus::PendingRisk, now).unwrap();
        order.transition_to(OrderStatus::Approved, now).unwrap();
        order.transition_to(OrderStatus::Submitted, now).unwrap();
        order.transition_to(OrderStatus::Unknown, now).unwrap();
        assert!(!order.status.is_terminal());
        order.transition_to(OrderStatus::Accepted, now).unwrap();
        assert_eq!(order.status, OrderStatus::Accepted);
    }
}
