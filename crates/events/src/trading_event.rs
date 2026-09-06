//! Immutable trading event facts and correlation envelope.

use chrono::{DateTime, Utc};
use domain::{
    AccountId, AccountState, ClientOrderId, DeploymentId, Fill, FillId, InstrumentId, Order,
    OrderId, OrderSide, Position, PositionId, RiskDecision, RiskRejectReason, StrategyId,
    StrategyVersion, TradeIntent, TradeIntentId, VenueId,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::EventsResult;
use crate::ids::{CorrelationId, EventId};
use crate::market_data::MarketDataEvent;
use crate::market_data_health::MarketDataHealth;

/// Immutable wrapper around a trading event fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: EventId,
    /// Stable id spanning intent → risk → order → fill.
    pub correlation_id: Option<CorrelationId>,
    /// Prior event id that caused this fact (audit chain).
    pub causation_id: Option<EventId>,
    pub occurred_at: DateTime<Utc>,
    pub event: TradingEvent,
}

impl EventEnvelope {
    pub fn wrap(event: TradingEvent, occurred_at: DateTime<Utc>) -> EventsResult<Self> {
        Ok(Self {
            id: EventId::generate("evt")?,
            correlation_id: None,
            causation_id: None,
            occurred_at,
            event,
        })
    }

    pub fn with_id(id: EventId, event: TradingEvent, occurred_at: DateTime<Utc>) -> Self {
        Self {
            id,
            correlation_id: None,
            causation_id: None,
            occurred_at,
            event,
        }
    }

    pub fn with_correlation(mut self, correlation_id: CorrelationId) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn with_causation(mut self, causation_id: EventId) -> Self {
        self.causation_id = Some(causation_id);
        self
    }

    pub fn correlation_trade_intent_id(&self) -> Option<&TradeIntentId> {
        match &self.event {
            TradingEvent::TradeIntentCreated { intent } => Some(&intent.id),
            TradingEvent::RiskCheckRequested {
                trade_intent_id, ..
            }
            | TradingEvent::RiskApproved {
                trade_intent_id, ..
            }
            | TradingEvent::RiskResized {
                trade_intent_id, ..
            }
            | TradingEvent::RiskRejected {
                trade_intent_id, ..
            } => Some(trade_intent_id),
            TradingEvent::OrderCreated { order }
            | TradingEvent::OrderSubmitted { order, .. }
            | TradingEvent::OrderAccepted { order, .. }
            | TradingEvent::OrderRejected { order, .. }
            | TradingEvent::OrderPartiallyFilled { order, .. }
            | TradingEvent::OrderFilled { order, .. }
            | TradingEvent::OrderCancelled { order, .. }
            | TradingEvent::OrderFailed { order, .. }
            | TradingEvent::OrderUnknown { order, .. } => order.trade_intent_id.as_ref(),
            _ => None,
        }
    }
}

/// Meaningful state-transition events for the trading pipeline.
///
/// Keep this set operational — do not add variants that carry no action value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradingEvent {
    MarketDataReceived {
        market_data: MarketDataEvent,
    },

    StrategySignalGenerated {
        strategy_id: StrategyId,
        strategy_version: StrategyVersion,
        deployment_id: DeploymentId,
        instrument_id: InstrumentId,
        side: OrderSide,
        price: Decimal,
    },

    TradeIntentCreated {
        intent: TradeIntent,
    },

    RiskCheckRequested {
        trade_intent_id: TradeIntentId,
        account_id: AccountId,
        deployment_id: DeploymentId,
    },

    RiskApproved {
        trade_intent_id: TradeIntentId,
        account_id: AccountId,
        quantity: Decimal,
        decision: RiskDecision,
    },

    RiskResized {
        trade_intent_id: TradeIntentId,
        account_id: AccountId,
        quantity: Decimal,
        reason: String,
        decision: RiskDecision,
    },

    RiskRejected {
        trade_intent_id: TradeIntentId,
        account_id: AccountId,
        reason: RiskRejectReason,
        decision: RiskDecision,
    },

    TradingHalted {
        account_id: Option<AccountId>,
        strategy_id: Option<StrategyId>,
        reason: String,
    },

    OrderCreated {
        order: Order,
    },

    OrderSubmitted {
        order: Order,
        venue_id: VenueId,
        client_order_id: ClientOrderId,
    },

    OrderAccepted {
        order: Order,
        venue_order_id: Option<String>,
    },

    OrderRejected {
        order: Order,
        reason: String,
    },

    OrderPartiallyFilled {
        order: Order,
        fill: Fill,
    },

    OrderFilled {
        order: Order,
        fill: Fill,
    },

    OrderCancelled {
        order: Order,
    },

    OrderFailed {
        order: Order,
        reason: String,
    },

    /// Ambiguous venue outcome — requires reconciliation.
    OrderUnknown {
        order: Order,
        reason: String,
    },

    /// Market data not valid for creating new strategy risk.
    MarketDataUnhealthy {
        health: MarketDataHealth,
    },

    PositionOpened {
        position: Position,
    },

    PositionUpdated {
        position: Position,
    },

    PositionClosed {
        position_id: PositionId,
        account_id: AccountId,
        instrument_id: InstrumentId,
        realized_pnl: Decimal,
    },

    AccountUpdated {
        account_id: AccountId,
        state: AccountState,
    },

    RiskLimitBreached {
        account_id: AccountId,
        reason: String,
    },

    ReconciliationStarted {
        account_id: AccountId,
        venue_id: VenueId,
    },

    ReconciliationCompleted {
        account_id: AccountId,
        venue_id: VenueId,
    },

    ReconciliationFailed {
        account_id: AccountId,
        venue_id: VenueId,
        reason: String,
    },

    /// Alert-only: internal/external state mismatch. Do not silently overwrite.
    ReconciliationAlert {
        account_id: AccountId,
        venue_id: VenueId,
        detail: String,
    },

    SystemError {
        context: String,
        message: String,
        trade_intent_id: Option<TradeIntentId>,
        order_id: Option<OrderId>,
        fill_id: Option<FillId>,
        account_id: Option<AccountId>,
    },
}

impl TradingEvent {
    pub fn name(&self) -> &'static str {
        match self {
            Self::MarketDataReceived { .. } => "MarketDataReceived",
            Self::StrategySignalGenerated { .. } => "StrategySignalGenerated",
            Self::TradeIntentCreated { .. } => "TradeIntentCreated",
            Self::RiskCheckRequested { .. } => "RiskCheckRequested",
            Self::RiskApproved { .. } => "RiskApproved",
            Self::RiskResized { .. } => "RiskResized",
            Self::RiskRejected { .. } => "RiskRejected",
            Self::TradingHalted { .. } => "TradingHalted",
            Self::OrderCreated { .. } => "OrderCreated",
            Self::OrderSubmitted { .. } => "OrderSubmitted",
            Self::OrderAccepted { .. } => "OrderAccepted",
            Self::OrderRejected { .. } => "OrderRejected",
            Self::OrderPartiallyFilled { .. } => "OrderPartiallyFilled",
            Self::OrderFilled { .. } => "OrderFilled",
            Self::OrderCancelled { .. } => "OrderCancelled",
            Self::OrderFailed { .. } => "OrderFailed",
            Self::OrderUnknown { .. } => "OrderUnknown",
            Self::MarketDataUnhealthy { .. } => "MarketDataUnhealthy",
            Self::PositionOpened { .. } => "PositionOpened",
            Self::PositionUpdated { .. } => "PositionUpdated",
            Self::PositionClosed { .. } => "PositionClosed",
            Self::AccountUpdated { .. } => "AccountUpdated",
            Self::RiskLimitBreached { .. } => "RiskLimitBreached",
            Self::ReconciliationStarted { .. } => "ReconciliationStarted",
            Self::ReconciliationCompleted { .. } => "ReconciliationCompleted",
            Self::ReconciliationFailed { .. } => "ReconciliationFailed",
            Self::ReconciliationAlert { .. } => "ReconciliationAlert",
            Self::SystemError { .. } => "SystemError",
        }
    }
}
