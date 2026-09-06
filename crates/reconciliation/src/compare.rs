//! Pure compare logic: internal snapshot vs venue-fetched state.

use std::collections::HashMap;

use domain::{AccountState, InstrumentId, Order, OrderId, Position};
use rust_decimal::Decimal;

use crate::report::ReconciliationBreak;

/// Tolerances for Decimal comparisons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareTolerances {
    pub quantity: Decimal,
    pub money: Decimal,
}

impl Default for CompareTolerances {
    fn default() -> Self {
        Self {
            quantity: Decimal::new(1, 4), // 0.0001
            money: Decimal::new(1, 8),    // 0.00000001
        }
    }
}

fn differs(a: Decimal, b: Decimal, tol: Decimal) -> bool {
    (a - b).abs() > tol
}

fn by_instrument(positions: &[Position]) -> HashMap<InstrumentId, &Position> {
    let mut map = HashMap::new();
    for p in positions {
        map.insert(p.instrument_id.clone(), p);
    }
    map
}

fn open_order_ids(orders: &[Order]) -> HashMap<OrderId, &Order> {
    let mut map = HashMap::new();
    for o in orders {
        if !o.status.is_terminal() {
            map.insert(o.id.clone(), o);
        }
    }
    map
}

/// Compare internal vs external books. Does not mutate either side.
pub fn compare_snapshots(
    internal_state: &AccountState,
    internal_positions: &[Position],
    internal_orders: &[Order],
    external_state: &AccountState,
    external_positions: &[Position],
    external_orders: &[Order],
    tolerances: &CompareTolerances,
) -> Vec<ReconciliationBreak> {
    let mut breaks = Vec::new();

    if differs(
        internal_state.equity,
        external_state.equity,
        tolerances.money,
    ) {
        breaks.push(ReconciliationBreak::AccountEquityMismatch {
            internal: internal_state.equity,
            external: external_state.equity,
        });
    }

    if differs(
        internal_state.balance,
        external_state.balance,
        tolerances.money,
    ) {
        breaks.push(ReconciliationBreak::AccountBalanceMismatch {
            internal: internal_state.balance,
            external: external_state.balance,
        });
    }

    if internal_state.open_position_count != external_state.open_position_count {
        breaks.push(ReconciliationBreak::AccountOpenPositionCountMismatch {
            internal: internal_state.open_position_count,
            external: external_state.open_position_count,
        });
    }

    let internal_pos = by_instrument(internal_positions);
    let external_pos = by_instrument(external_positions);

    for (instrument_id, ext) in &external_pos {
        match internal_pos.get(instrument_id) {
            None => breaks.push(ReconciliationBreak::PositionMissingInternal {
                instrument_id: instrument_id.clone(),
                external_quantity: ext.quantity,
                external_side: ext.side,
            }),
            Some(int) => {
                if int.side != ext.side {
                    breaks.push(ReconciliationBreak::PositionSideMismatch {
                        instrument_id: instrument_id.clone(),
                        internal_side: int.side,
                        external_side: ext.side,
                    });
                }
                if differs(int.quantity, ext.quantity, tolerances.quantity) {
                    breaks.push(ReconciliationBreak::PositionQuantityMismatch {
                        instrument_id: instrument_id.clone(),
                        internal_quantity: int.quantity,
                        external_quantity: ext.quantity,
                    });
                }
            }
        }
    }

    for (instrument_id, int) in &internal_pos {
        if !external_pos.contains_key(instrument_id) {
            breaks.push(ReconciliationBreak::PositionMissingExternal {
                instrument_id: instrument_id.clone(),
                internal_quantity: int.quantity,
                internal_side: int.side,
            });
        }
    }

    let internal_orders = open_order_ids(internal_orders);
    let external_orders = open_order_ids(external_orders);

    for order_id in external_orders.keys() {
        if !internal_orders.contains_key(order_id) {
            breaks.push(ReconciliationBreak::OpenOrderMissingInternal {
                order_id: order_id.clone(),
            });
        }
    }

    for order_id in internal_orders.keys() {
        if !external_orders.contains_key(order_id) {
            breaks.push(ReconciliationBreak::OpenOrderMissingExternal {
                order_id: order_id.clone(),
            });
        }
    }

    breaks
}
