//! Runtime wiring configuration (orchestration knobs only).

use domain::{AccountId, OrderType, RiskPolicy, TimeInForce, VenueId};
use rust_decimal::Decimal;

/// Static configuration for a continuous trading session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub account_id: AccountId,
    pub venue_id: VenueId,
    pub risk_policy: RiskPolicy,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    /// Open notional exposure fed into risk requests.
    pub current_exposure: Decimal,
    pub session_allowed: bool,
}

impl RuntimeConfig {
    pub fn new(account_id: AccountId, venue_id: VenueId, risk_policy: RiskPolicy) -> Self {
        Self {
            account_id,
            venue_id,
            risk_policy,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Ioc,
            current_exposure: Decimal::ZERO,
            session_allowed: true,
        }
    }

    pub fn with_order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = order_type;
        self
    }

    pub fn with_time_in_force(mut self, time_in_force: TimeInForce) -> Self {
        self.time_in_force = time_in_force;
        self
    }

    pub fn with_exposure(mut self, current_exposure: Decimal) -> Self {
        self.current_exposure = current_exposure;
        self
    }

    pub fn with_session_allowed(mut self, session_allowed: bool) -> Self {
        self.session_allowed = session_allowed;
        self
    }
}
