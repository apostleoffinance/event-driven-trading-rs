//! Risk domain types. Evaluation logic lives in `risk-engine` (Phase 4).
//!
//! Given the same RiskRequest + AccountState + RiskPolicy, the evaluator
//! must return a deterministic RiskDecision.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::account::AccountState;
use crate::error::DomainResult;
use crate::ids::{AccountId, DeploymentId, InstrumentId, RiskProfileId, StrategyId, TradeIntentId};
use crate::money::{require_non_negative, require_percent, require_positive};
use crate::order::OrderSide;
use crate::trade_intent::TradeIntent;

/// Named risk profile metadata (limits live on [`RiskPolicy`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskProfile {
    pub id: RiskProfileId,
    pub name: String,
    pub policy: RiskPolicy,
}

/// Configurable risk limits. Prop-firm specifics are data, not hardcoded logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskPolicy {
    pub max_risk_per_trade_pct: Decimal,
    pub max_position_size_pct: Decimal,
    pub max_total_exposure_pct: Decimal,
    pub max_leverage: Decimal,
    pub max_daily_loss_pct: Decimal,
    pub max_drawdown_pct: Decimal,
    pub max_open_positions: u32,
    pub allowed_instruments: Option<Vec<InstrumentId>>,
    pub restricted_instruments: Vec<InstrumentId>,
    pub global_kill_switch: bool,
    pub account_kill_switch: bool,
    pub strategy_kill_switch: bool,
}

impl RiskPolicy {
    pub fn try_new(
        max_risk_per_trade_pct: Decimal,
        max_position_size_pct: Decimal,
        max_total_exposure_pct: Decimal,
        max_leverage: Decimal,
        max_daily_loss_pct: Decimal,
        max_drawdown_pct: Decimal,
        max_open_positions: u32,
    ) -> DomainResult<Self> {
        Ok(Self {
            max_risk_per_trade_pct: require_percent(
                max_risk_per_trade_pct,
                "max_risk_per_trade_pct",
            )?,
            max_position_size_pct: require_percent(max_position_size_pct, "max_position_size_pct")?,
            max_total_exposure_pct: require_percent(
                max_total_exposure_pct,
                "max_total_exposure_pct",
            )?,
            max_leverage: require_positive(max_leverage, "max_leverage")?,
            max_daily_loss_pct: require_percent(max_daily_loss_pct, "max_daily_loss_pct")?,
            max_drawdown_pct: require_percent(max_drawdown_pct, "max_drawdown_pct")?,
            max_open_positions,
            allowed_instruments: None,
            restricted_instruments: Vec::new(),
            global_kill_switch: false,
            account_kill_switch: false,
            strategy_kill_switch: false,
        })
    }
}

/// Input to a deterministic risk evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskRequest {
    pub trade_intent_id: TradeIntentId,
    pub strategy_id: StrategyId,
    pub deployment_id: DeploymentId,
    pub account_id: AccountId,
    pub instrument_id: InstrumentId,
    pub side: OrderSide,
    pub requested_quantity: Option<Decimal>,
    pub requested_notional: Option<Decimal>,
    pub entry_price: Option<Decimal>,
    pub stop_loss: Option<Decimal>,
    pub account_state: AccountState,
    pub policy: RiskPolicy,
    pub requested_at: DateTime<Utc>,
}

impl RiskRequest {
    pub fn from_intent(
        intent: &TradeIntent,
        account_id: AccountId,
        account_state: AccountState,
        policy: RiskPolicy,
        requested_at: DateTime<Utc>,
    ) -> Self {
        Self {
            trade_intent_id: intent.id.clone(),
            strategy_id: intent.strategy_id.clone(),
            deployment_id: intent.deployment_id.clone(),
            account_id,
            instrument_id: intent.instrument_id.clone(),
            side: intent.side,
            requested_quantity: intent.target_quantity,
            requested_notional: intent.target_notional,
            entry_price: intent.entry_price,
            stop_loss: intent.stop_loss,
            account_state,
            policy,
            requested_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskRejectReason {
    GlobalHalt,
    AccountHalt,
    StrategyHalt,
    InstrumentRestricted,
    SessionNotAllowed,
    DrawdownBreached,
    DailyLossBreached,
    ExposureLimit,
    PositionLimit,
    LeverageLimit,
    TradeRiskLimit,
    InvalidRequest,
    Other(String),
}

/// Deterministic outcome of risk evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskDecision {
    Approved { quantity: Decimal },
    Resized { quantity: Decimal, reason: String },
    Rejected { reason: RiskRejectReason },
    Halted { reason: String },
}

impl RiskDecision {
    pub fn approved_quantity(&self) -> Option<Decimal> {
        match self {
            Self::Approved { quantity } | Self::Resized { quantity, .. } => Some(*quantity),
            Self::Rejected { .. } | Self::Halted { .. } => None,
        }
    }

    pub fn is_executable(&self) -> bool {
        matches!(self, Self::Approved { .. } | Self::Resized { .. })
    }
}

/// Helper to validate approved/resized quantities are non-negative.
pub fn validated_decision_quantity(quantity: Decimal) -> DomainResult<Decimal> {
    require_non_negative(quantity, "decision_quantity")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn policy_rejects_invalid_percent() {
        let err = RiskPolicy::try_new(
            Decimal::from(200),
            Decimal::from(2),
            Decimal::from(50),
            Decimal::from(1),
            Decimal::from(10),
            Decimal::from(20),
            5,
        )
        .unwrap_err();
        assert!(matches!(err, crate::error::DomainError::InvalidMoney(_)));
    }

    #[test]
    fn decision_executable_only_when_approved_or_resized() {
        assert!(RiskDecision::Approved {
            quantity: Decimal::ONE
        }
        .is_executable());
        assert!(!RiskDecision::Rejected {
            reason: RiskRejectReason::GlobalHalt
        }
        .is_executable());
    }
}
