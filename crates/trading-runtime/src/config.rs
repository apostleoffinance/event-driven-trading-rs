//! Runtime wiring configuration (orchestration knobs only).

use domain::{AccountId, InstrumentSpec, OrderType, RiskPolicy, TimeInForce, VenueId};
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
    /// Default instrument constraints applied before venue submit.
    pub instrument_spec: Option<InstrumentSpec>,
    /// Max age of market data before strategy risk is blocked (seconds).
    pub market_data_max_age_secs: i64,
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
            instrument_spec: InstrumentSpec::crypto_spot_default().ok(),
            market_data_max_age_secs: 30,
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

    pub fn with_instrument_spec(mut self, spec: InstrumentSpec) -> Self {
        self.instrument_spec = Some(spec);
        self
    }

    pub fn with_market_data_max_age_secs(mut self, secs: i64) -> Self {
        self.market_data_max_age_secs = secs;
        self
    }
}
