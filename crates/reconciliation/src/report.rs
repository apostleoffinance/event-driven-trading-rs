//! Snapshot and break report types.

use chrono::{DateTime, Utc};
use domain::{
    AccountId, AccountState, InstrumentId, Order, OrderId, Position, PositionSide, VenueId,
};
use rust_decimal::Decimal;

/// Caller-owned view of internal (engine) state for a recon pass.
///
/// Positions are not owned by AccountEngine — pass the book the runtime/store
/// believes is authoritative internally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalSnapshot {
    pub account_id: AccountId,
    pub venue_id: VenueId,
    pub account_state: AccountState,
    pub positions: Vec<Position>,
    pub open_orders: Vec<Order>,
}

impl InternalSnapshot {
    pub fn new(account_id: AccountId, venue_id: VenueId, account_state: AccountState) -> Self {
        Self {
            account_id,
            venue_id,
            account_state,
            positions: Vec::new(),
            open_orders: Vec::new(),
        }
    }

    pub fn with_positions(mut self, positions: Vec<Position>) -> Self {
        self.positions = positions;
        self
    }

    pub fn with_open_orders(mut self, open_orders: Vec<Order>) -> Self {
        self.open_orders = open_orders;
        self
    }
}

/// A single detected mismatch. Never auto-applied to internal state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationBreak {
    PositionMissingInternal {
        instrument_id: InstrumentId,
        external_quantity: Decimal,
        external_side: PositionSide,
    },
    PositionMissingExternal {
        instrument_id: InstrumentId,
        internal_quantity: Decimal,
        internal_side: PositionSide,
    },
    PositionQuantityMismatch {
        instrument_id: InstrumentId,
        internal_quantity: Decimal,
        external_quantity: Decimal,
    },
    PositionSideMismatch {
        instrument_id: InstrumentId,
        internal_side: PositionSide,
        external_side: PositionSide,
    },
    AccountEquityMismatch {
        internal: Decimal,
        external: Decimal,
    },
    AccountBalanceMismatch {
        internal: Decimal,
        external: Decimal,
    },
    AccountOpenPositionCountMismatch {
        internal: u32,
        external: u32,
    },
    OpenOrderMissingInternal {
        order_id: OrderId,
    },
    OpenOrderMissingExternal {
        order_id: OrderId,
    },
}

impl ReconciliationBreak {
    pub fn detail(&self) -> String {
        match self {
            Self::PositionMissingInternal {
                instrument_id,
                external_quantity,
                external_side,
            } => format!(
                "position missing internally for {instrument_id}: external qty={external_quantity} side={external_side:?}"
            ),
            Self::PositionMissingExternal {
                instrument_id,
                internal_quantity,
                internal_side,
            } => format!(
                "position missing at venue for {instrument_id}: internal qty={internal_quantity} side={internal_side:?}"
            ),
            Self::PositionQuantityMismatch {
                instrument_id,
                internal_quantity,
                external_quantity,
            } => format!(
                "quantity mismatch for {instrument_id}: internal={internal_quantity} external={external_quantity}"
            ),
            Self::PositionSideMismatch {
                instrument_id,
                internal_side,
                external_side,
            } => format!(
                "side mismatch for {instrument_id}: internal={internal_side:?} external={external_side:?}"
            ),
            Self::AccountEquityMismatch { internal, external } => {
                format!("equity mismatch: internal={internal} external={external}")
            }
            Self::AccountBalanceMismatch { internal, external } => {
                format!("balance mismatch: internal={internal} external={external}")
            }
            Self::AccountOpenPositionCountMismatch { internal, external } => {
                format!(
                    "open_position_count mismatch: internal={internal} external={external}"
                )
            }
            Self::OpenOrderMissingInternal { order_id } => {
                format!("open order at venue missing internally: {order_id}")
            }
            Self::OpenOrderMissingExternal { order_id } => {
                format!("open order internal missing at venue: {order_id}")
            }
        }
    }
}

/// Result of one reconciliation pass (compare only — no mutations).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationReport {
    pub account_id: AccountId,
    pub venue_id: VenueId,
    pub breaks: Vec<ReconciliationBreak>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

impl ReconciliationReport {
    pub fn is_clean(&self) -> bool {
        self.breaks.is_empty()
    }

    pub fn break_count(&self) -> usize {
        self.breaks.len()
    }
}
