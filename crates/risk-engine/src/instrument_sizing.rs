//! Instrument-aware, deterministic sizing for instruments with explicit loss models.
//!
//! This API is deliberately separate from the legacy `size_order` function:
//! callers must provide an explicit model and fresh snapshot status. It does not
//! infer contract mechanics from a symbol or call a venue/network API.

use domain::{OrderSide, RiskDecision, RiskRejectReason, RiskRequest};
use rust_decimal::Decimal;

/// Explicit loss mechanics supplied by a trusted instrument/market-data layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossModel {
    /// Linear P&L: price distance × contract multiplier × P&L-to-account conversion.
    Linear {
        contract_multiplier: Decimal,
        pnl_to_account_rate: Decimal,
    },
    /// Tick-value P&L: exact stop ticks × tick value per quantity unit × conversion.
    TickValue {
        tick_size: Decimal,
        tick_value_per_quantity: Decimal,
        tick_value_to_account_rate: Decimal,
    },
}

/// Constraints and freshness for a single account/instrument sizing calculation.
///
/// All currency conversion rates are explicit and must be directionally correct
/// for the supplied account and instrument. Rates must be 1 when already in the
/// account currency. Cost and buffer values are account-currency loss per unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentSizingContext {
    pub loss_model: LossModel,
    pub inputs_fresh: bool,
    pub notional_to_account_rate: Decimal,
    pub quantity_step: Decimal,
    pub min_quantity: Decimal,
    pub max_quantity: Option<Decimal>,
    pub min_notional_account_currency: Option<Decimal>,
    pub estimated_cost_per_quantity: Decimal,
    pub slippage_gap_buffer_per_quantity: Decimal,
    /// Account-currency notional already reserved by pending orders.
    pub pending_order_exposure: Decimal,
    /// Remaining free margin from the same fresh account snapshot, when margin applies.
    pub available_margin: Option<Decimal>,
    /// Account-currency margin already reserved by pending orders.
    pub pending_order_margin: Decimal,
    /// Account-currency margin required per unit of this instrument.
    pub margin_per_quantity: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizingRejectReason {
    InvalidInput,
    InvalidStopGeometry,
    StaleInputs,
    UnsupportedOrInvalidLossModel,
    StopDistanceNotOnTick,
    RiskBudgetExhausted,
    ExposureExhausted,
    LeverageExhausted,
    MarginExhausted,
    BelowMinimumQuantity,
    MinimumNotional,
    ArithmeticOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizingLimitReason {
    RiskBudget,
    PositionLimit,
    ExposureLimit,
    LeverageLimit,
    MarginLimit,
    VenueMaximum,
    VolumeStep,
}

/// Auditable result of the pure sizing calculation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentSizingOutcome {
    Approved {
        quantity: Decimal,
        risk_budget: Decimal,
        modeled_loss: Decimal,
    },
    Resized {
        quantity: Decimal,
        risk_budget: Decimal,
        modeled_loss: Decimal,
        reason: SizingLimitReason,
    },
    Rejected {
        reason: SizingRejectReason,
    },
}

impl InstrumentSizingOutcome {
    pub fn quantity(&self) -> Option<Decimal> {
        match self {
            Self::Approved { quantity, .. } | Self::Resized { quantity, .. } => Some(*quantity),
            Self::Rejected { .. } => None,
        }
    }

    pub fn risk_decision(&self) -> RiskDecision {
        match self {
            Self::Approved { quantity, .. } => RiskDecision::Approved {
                quantity: *quantity,
            },
            Self::Resized {
                quantity, reason, ..
            } => RiskDecision::Resized {
                quantity: *quantity,
                reason: format!("instrument-aware sizing limited quantity: {reason:?}"),
            },
            Self::Rejected { reason } => RiskDecision::Rejected {
                reason: match reason {
                    SizingRejectReason::InvalidInput | SizingRejectReason::InvalidStopGeometry => {
                        RiskRejectReason::InvalidRequest
                    }
                    SizingRejectReason::StaleInputs
                    | SizingRejectReason::UnsupportedOrInvalidLossModel
                    | SizingRejectReason::StopDistanceNotOnTick
                    | SizingRejectReason::ArithmeticOverflow => {
                        RiskRejectReason::DependencyUnavailable
                    }
                    SizingRejectReason::RiskBudgetExhausted => RiskRejectReason::TradeRiskLimit,
                    SizingRejectReason::ExposureExhausted => RiskRejectReason::ExposureLimit,
                    SizingRejectReason::LeverageExhausted => RiskRejectReason::LeverageLimit,
                    SizingRejectReason::MarginExhausted => RiskRejectReason::DependencyUnavailable,
                    SizingRejectReason::BelowMinimumQuantity
                    | SizingRejectReason::MinimumNotional => RiskRejectReason::InstrumentConstraint,
                },
            },
        }
    }
}

/// Compute a size from explicit instrument mechanics and account-currency inputs.
///
/// Missing stops, stale inputs, unsupported model parameters, invalid geometry,
/// invalid volume constraints, and arithmetic failures all reject. Quantity is
/// rounded down to the exact configured step and modeled loss is rechecked after
/// rounding. This function is pure and never submits an order.
pub fn size_order_instrument_aware(
    request: &RiskRequest,
    context: &InstrumentSizingContext,
) -> InstrumentSizingOutcome {
    if !context.inputs_fresh {
        return reject(SizingRejectReason::StaleInputs);
    }

    let entry = match request.entry_price {
        Some(value) if value > Decimal::ZERO => value,
        _ => return reject(SizingRejectReason::InvalidInput),
    };
    let stop = match request.stop_loss {
        Some(value) if value > Decimal::ZERO => value,
        _ => return reject(SizingRejectReason::InvalidStopGeometry),
    };
    let equity = request.account_state.equity;
    if equity <= Decimal::ZERO
        || request.current_exposure < Decimal::ZERO
        || request.policy.max_risk_per_trade_pct <= Decimal::ZERO
        || request.policy.max_risk_per_trade_pct > Decimal::from(100)
        || request.policy.max_position_size_pct <= Decimal::ZERO
        || request.policy.max_position_size_pct > Decimal::ONE_HUNDRED
        || request.policy.max_total_exposure_pct <= Decimal::ZERO
        || request.policy.max_total_exposure_pct > Decimal::ONE_HUNDRED
        || request.policy.max_leverage <= Decimal::ZERO
        || context.notional_to_account_rate <= Decimal::ZERO
        || context.quantity_step <= Decimal::ZERO
        || context.min_quantity <= Decimal::ZERO
        || context.estimated_cost_per_quantity < Decimal::ZERO
        || context.slippage_gap_buffer_per_quantity < Decimal::ZERO
        || context.pending_order_exposure < Decimal::ZERO
        || context.pending_order_margin < Decimal::ZERO
        || context
            .available_margin
            .is_some_and(|margin| margin < Decimal::ZERO)
        || context
            .margin_per_quantity
            .is_some_and(|margin| margin <= Decimal::ZERO)
        || context.available_margin.is_some() != context.margin_per_quantity.is_some()
    {
        return reject(SizingRejectReason::InvalidInput);
    }
    if context
        .max_quantity
        .is_some_and(|maximum| maximum < context.min_quantity || maximum <= Decimal::ZERO)
        || context
            .min_notional_account_currency
            .is_some_and(|minimum| minimum < Decimal::ZERO)
    {
        return reject(SizingRejectReason::InvalidInput);
    }

    let valid_geometry = match request.side {
        OrderSide::Buy => stop < entry,
        OrderSide::Sell => stop > entry,
    };
    if !valid_geometry {
        return reject(SizingRejectReason::InvalidStopGeometry);
    }
    if !is_multiple_of(context.min_quantity, context.quantity_step) {
        return reject(SizingRejectReason::InvalidInput);
    }

    let stop_distance = match entry.checked_sub(stop) {
        Some(delta) if delta != Decimal::ZERO => delta.abs(),
        _ => return reject(SizingRejectReason::ArithmeticOverflow),
    };
    let loss_per_quantity = match loss_per_quantity(context, stop_distance) {
        Ok(loss) if loss > Decimal::ZERO => loss,
        Ok(_) => return reject(SizingRejectReason::UnsupportedOrInvalidLossModel),
        Err(reason) => return reject(reason),
    };
    let risk_budget = match checked_mul(equity, request.policy.max_risk_per_trade_pct)
        .and_then(|value| value.checked_div(Decimal::from(100)))
    {
        Some(value) if value > Decimal::ZERO => value,
        _ => return reject(SizingRejectReason::RiskBudgetExhausted),
    };
    let risk_cap = match risk_budget.checked_div(loss_per_quantity) {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };

    let notional_per_quantity = match entry.checked_mul(context.notional_to_account_rate) {
        Some(value) if value > Decimal::ZERO => value,
        _ => return reject(SizingRejectReason::ArithmeticOverflow),
    };
    let position_notional = match checked_mul(equity, request.policy.max_position_size_pct)
        .and_then(|value| value.checked_div(Decimal::from(100)))
    {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };
    let max_by_position = match position_notional.checked_div(notional_per_quantity) {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };

    let max_exposure = match checked_mul(equity, request.policy.max_total_exposure_pct)
        .and_then(|value| value.checked_div(Decimal::from(100)))
    {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };
    let effective_exposure = match request
        .current_exposure
        .checked_add(context.pending_order_exposure)
    {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };
    let remaining_exposure = (max_exposure - effective_exposure).max(Decimal::ZERO);
    let max_by_exposure = match remaining_exposure.checked_div(notional_per_quantity) {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };
    let max_levered_notional = match equity.checked_mul(request.policy.max_leverage) {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };
    let remaining_levered = (max_levered_notional - effective_exposure).max(Decimal::ZERO);
    let max_by_leverage = match remaining_levered.checked_div(notional_per_quantity) {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };

    let max_by_margin = match (context.available_margin, context.margin_per_quantity) {
        (Some(available), Some(per_quantity)) => match available
            .checked_sub(context.pending_order_margin)
            .map(|remaining| remaining.max(Decimal::ZERO))
            .and_then(|remaining| remaining.checked_div(per_quantity))
        {
            Some(cap) => Some(cap),
            None => return reject(SizingRejectReason::ArithmeticOverflow),
        },
        (None, None) => None,
        _ => return reject(SizingRejectReason::InvalidInput),
    };

    let explicit_request =
        request.requested_quantity.is_some() || request.requested_notional.is_some();
    let desired = if let Some(quantity) = request.requested_quantity {
        if quantity <= Decimal::ZERO {
            return reject(SizingRejectReason::InvalidInput);
        }
        quantity
    } else if let Some(notional) = request.requested_notional {
        if notional <= Decimal::ZERO {
            return reject(SizingRejectReason::InvalidInput);
        }
        match notional.checked_div(notional_per_quantity) {
            Some(quantity) => quantity,
            None => return reject(SizingRejectReason::ArithmeticOverflow),
        }
    } else {
        max_by_position
    };

    let mut allowed = desired
        .min(risk_cap)
        .min(max_by_position)
        .min(max_by_exposure)
        .min(max_by_leverage);
    if let Some(margin_cap) = max_by_margin {
        allowed = allowed.min(margin_cap);
    }
    if let Some(maximum) = context.max_quantity {
        allowed = allowed.min(maximum);
    }
    if allowed <= Decimal::ZERO {
        let reason = if max_by_exposure <= Decimal::ZERO {
            SizingRejectReason::ExposureExhausted
        } else if max_by_leverage <= Decimal::ZERO {
            SizingRejectReason::LeverageExhausted
        } else if max_by_margin.is_some_and(|cap| cap <= Decimal::ZERO) {
            SizingRejectReason::MarginExhausted
        } else {
            SizingRejectReason::RiskBudgetExhausted
        };
        return reject(reason);
    }

    let rounded = match round_down_to_step(allowed, context.quantity_step) {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };
    if rounded < context.min_quantity {
        return reject(SizingRejectReason::BelowMinimumQuantity);
    }

    let notional = match rounded.checked_mul(notional_per_quantity) {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };
    if context
        .min_notional_account_currency
        .is_some_and(|minimum| notional < minimum)
    {
        return reject(SizingRejectReason::MinimumNotional);
    }

    let modeled_loss = match rounded.checked_mul(loss_per_quantity) {
        Some(value) => value,
        None => return reject(SizingRejectReason::ArithmeticOverflow),
    };
    if modeled_loss > risk_budget {
        return reject(SizingRejectReason::RiskBudgetExhausted);
    }

    if explicit_request && rounded < desired {
        let reason = if risk_cap <= max_by_position
            && risk_cap <= max_by_exposure
            && risk_cap <= max_by_leverage
            && context
                .max_quantity
                .is_none_or(|maximum| risk_cap <= maximum)
        {
            SizingLimitReason::RiskBudget
        } else if max_by_margin.is_some_and(|cap| {
            cap <= risk_cap
                && cap <= max_by_position
                && cap <= max_by_exposure
                && cap <= max_by_leverage
                && context.max_quantity.is_none_or(|maximum| cap <= maximum)
        }) {
            SizingLimitReason::MarginLimit
        } else if max_by_exposure <= max_by_position && max_by_exposure <= max_by_leverage {
            SizingLimitReason::ExposureLimit
        } else if max_by_leverage <= max_by_position && max_by_leverage <= max_by_exposure {
            SizingLimitReason::LeverageLimit
        } else if context
            .max_quantity
            .is_some_and(|maximum| maximum <= allowed)
        {
            SizingLimitReason::VenueMaximum
        } else if max_by_position < desired {
            SizingLimitReason::PositionLimit
        } else {
            SizingLimitReason::VolumeStep
        };
        InstrumentSizingOutcome::Resized {
            quantity: rounded,
            risk_budget,
            modeled_loss,
            reason,
        }
    } else {
        InstrumentSizingOutcome::Approved {
            quantity: rounded,
            risk_budget,
            modeled_loss,
        }
    }
}

fn loss_per_quantity(
    context: &InstrumentSizingContext,
    stop_distance: Decimal,
) -> Result<Decimal, SizingRejectReason> {
    let gross = match context.loss_model {
        LossModel::Linear {
            contract_multiplier,
            pnl_to_account_rate,
        } => {
            if contract_multiplier <= Decimal::ZERO || pnl_to_account_rate <= Decimal::ZERO {
                return Err(SizingRejectReason::UnsupportedOrInvalidLossModel);
            }
            stop_distance
                .checked_mul(contract_multiplier)
                .and_then(|value| value.checked_mul(pnl_to_account_rate))
        }
        LossModel::TickValue {
            tick_size,
            tick_value_per_quantity,
            tick_value_to_account_rate,
        } => {
            if tick_size <= Decimal::ZERO
                || tick_value_per_quantity <= Decimal::ZERO
                || tick_value_to_account_rate <= Decimal::ZERO
            {
                return Err(SizingRejectReason::UnsupportedOrInvalidLossModel);
            }
            let ticks = stop_distance
                .checked_div(tick_size)
                .ok_or(SizingRejectReason::ArithmeticOverflow)?;
            if ticks != ticks.trunc() {
                return Err(SizingRejectReason::StopDistanceNotOnTick);
            }
            ticks
                .checked_mul(tick_value_per_quantity)
                .and_then(|value| value.checked_mul(tick_value_to_account_rate))
        }
    }
    .ok_or(SizingRejectReason::ArithmeticOverflow)?;

    gross
        .checked_add(context.estimated_cost_per_quantity)
        .and_then(|value| value.checked_add(context.slippage_gap_buffer_per_quantity))
        .ok_or(SizingRejectReason::ArithmeticOverflow)
}

fn checked_mul(left: Decimal, right: Decimal) -> Option<Decimal> {
    left.checked_mul(right)
}

fn is_multiple_of(value: Decimal, step: Decimal) -> bool {
    if step <= Decimal::ZERO {
        return false;
    }
    value
        .checked_div(step)
        .is_some_and(|ratio| ratio == ratio.trunc())
}

fn round_down_to_step(value: Decimal, step: Decimal) -> Option<Decimal> {
    if value <= Decimal::ZERO || step <= Decimal::ZERO {
        return None;
    }
    value
        .checked_div(step)
        .map(|steps| steps.trunc())
        .and_then(|steps| steps.checked_mul(step))
}

fn reject(reason: SizingRejectReason) -> InstrumentSizingOutcome {
    InstrumentSizingOutcome::Rejected { reason }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use crate::test_support::sample_request;

    fn context() -> InstrumentSizingContext {
        InstrumentSizingContext {
            loss_model: LossModel::Linear {
                contract_multiplier: Decimal::ONE,
                pnl_to_account_rate: Decimal::ONE,
            },
            inputs_fresh: true,
            notional_to_account_rate: Decimal::ONE,
            quantity_step: Decimal::new(1, 1),
            min_quantity: Decimal::new(1, 1),
            max_quantity: Some(Decimal::from(100)),
            min_notional_account_currency: None,
            estimated_cost_per_quantity: Decimal::ZERO,
            slippage_gap_buffer_per_quantity: Decimal::ZERO,
            pending_order_exposure: Decimal::ZERO,
            available_margin: None,
            pending_order_margin: Decimal::ZERO,
            margin_per_quantity: None,
        }
    }

    #[test]
    fn linear_model_resizes_to_risk_budget_and_rounds_down() {
        let mut request = sample_request();
        request.account_state.equity = Decimal::from(10_000);
        request.policy.max_risk_per_trade_pct = Decimal::ONE;
        request.policy.max_position_size_pct = Decimal::from(100);
        request.policy.max_total_exposure_pct = Decimal::from(100);
        request.policy.max_leverage = Decimal::from(10);
        request.entry_price = Some(Decimal::from(100));
        request.stop_loss = Some(Decimal::from(90));
        request.requested_quantity = Some(Decimal::from(50));
        let result = size_order_instrument_aware(&request, &context());
        assert_eq!(
            result,
            InstrumentSizingOutcome::Resized {
                quantity: Decimal::from(10),
                risk_budget: Decimal::from(100),
                modeled_loss: Decimal::from(100),
                reason: SizingLimitReason::RiskBudget,
            }
        );
    }

    #[test]
    fn tick_value_model_applies_conversion_once() {
        let mut request = sample_request();
        request.account_state.equity = Decimal::from(10_000);
        request.policy.max_risk_per_trade_pct = Decimal::ONE;
        request.policy.max_position_size_pct = Decimal::from(100);
        request.policy.max_total_exposure_pct = Decimal::from(100);
        request.policy.max_leverage = Decimal::from(10);
        request.entry_price = Some(Decimal::from(100));
        request.stop_loss = Some(Decimal::from(98));
        request.requested_quantity = Some(Decimal::from(20));
        let mut sizing = context();
        sizing.loss_model = LossModel::TickValue {
            tick_size: Decimal::new(1, 1),
            tick_value_per_quantity: Decimal::ONE,
            tick_value_to_account_rate: Decimal::from(2),
        };
        let result = size_order_instrument_aware(&request, &sizing);
        assert_eq!(result.quantity(), Some(Decimal::new(25, 1)));
        assert!(matches!(
            result,
            InstrumentSizingOutcome::Resized {
                risk_budget,
                modeled_loss,
                ..
            } if risk_budget == Decimal::from(100) && modeled_loss == Decimal::from(100)
        ));
    }

    #[test]
    fn buy_stop_at_or_above_entry_rejects() {
        let mut request = sample_request();
        request.entry_price = Some(Decimal::from(100));
        request.stop_loss = Some(Decimal::from(100));
        assert_eq!(
            size_order_instrument_aware(&request, &context()),
            InstrumentSizingOutcome::Rejected {
                reason: SizingRejectReason::InvalidStopGeometry,
            }
        );
    }

    #[test]
    fn sell_stop_at_or_below_entry_rejects() {
        let mut request = sample_request();
        request.side = OrderSide::Sell;
        request.entry_price = Some(Decimal::from(100));
        request.stop_loss = Some(Decimal::from(100));
        assert_eq!(
            size_order_instrument_aware(&request, &context()),
            InstrumentSizingOutcome::Rejected {
                reason: SizingRejectReason::InvalidStopGeometry,
            }
        );
    }

    #[test]
    fn stale_inputs_fail_closed() {
        let request = sample_request();
        let mut sizing = context();
        sizing.inputs_fresh = false;
        assert_eq!(
            size_order_instrument_aware(&request, &sizing),
            InstrumentSizingOutcome::Rejected {
                reason: SizingRejectReason::StaleInputs,
            }
        );
    }

    #[test]
    fn below_minimum_and_invalid_tick_distance_reject() {
        let mut request = sample_request();
        request.account_state.equity = Decimal::from(100);
        request.policy.max_risk_per_trade_pct = Decimal::ONE;
        request.entry_price = Some(Decimal::from(100));
        request.stop_loss = Some(Decimal::from(99));
        request.requested_quantity = Some(Decimal::new(1, 2));
        let mut sizing = context();
        sizing.loss_model = LossModel::TickValue {
            tick_size: Decimal::from(3),
            tick_value_per_quantity: Decimal::ONE,
            tick_value_to_account_rate: Decimal::ONE,
        };
        assert_eq!(
            size_order_instrument_aware(&request, &sizing),
            InstrumentSizingOutcome::Rejected {
                reason: SizingRejectReason::StopDistanceNotOnTick,
            }
        );

        sizing.loss_model = LossModel::Linear {
            contract_multiplier: Decimal::ONE,
            pnl_to_account_rate: Decimal::ONE,
        };
        assert_eq!(
            size_order_instrument_aware(&request, &sizing),
            InstrumentSizingOutcome::Rejected {
                reason: SizingRejectReason::BelowMinimumQuantity,
            }
        );
    }

    #[test]
    fn modeled_loss_includes_cost_and_buffer() {
        let mut request = sample_request();
        request.account_state.equity = Decimal::from(10_000);
        request.policy.max_risk_per_trade_pct = Decimal::ONE;
        request.policy.max_position_size_pct = Decimal::from(100);
        request.policy.max_total_exposure_pct = Decimal::from(100);
        request.policy.max_leverage = Decimal::from(10);
        request.entry_price = Some(Decimal::from(100));
        request.stop_loss = Some(Decimal::from(90));
        request.requested_quantity = Some(Decimal::from(5));
        let mut sizing = context();
        sizing.estimated_cost_per_quantity = Decimal::ONE;
        sizing.slippage_gap_buffer_per_quantity = Decimal::ONE;
        let result = size_order_instrument_aware(&request, &sizing);
        assert_eq!(
            result,
            InstrumentSizingOutcome::Approved {
                quantity: Decimal::from(5),
                risk_budget: Decimal::from(100),
                modeled_loss: Decimal::from(60),
            }
        );
    }
    #[test]
    fn pending_order_exposure_reduces_available_exposure_cap() {
        let mut request = sample_request();
        request.account_state.equity = Decimal::from(10_000);
        request.policy.max_risk_per_trade_pct = Decimal::from(10);
        request.policy.max_position_size_pct = Decimal::from(100);
        request.policy.max_total_exposure_pct = Decimal::ONE;
        request.policy.max_leverage = Decimal::from(10);
        request.entry_price = Some(Decimal::from(100));
        request.stop_loss = Some(Decimal::from(99));
        request.requested_quantity = Some(Decimal::from(10));
        let mut sizing = context();
        sizing.pending_order_exposure = Decimal::from(50);

        let result = size_order_instrument_aware(&request, &sizing);
        assert_eq!(result.quantity(), Some(Decimal::new(5, 1)));
        assert!(matches!(
            result,
            InstrumentSizingOutcome::Resized {
                reason: SizingLimitReason::ExposureLimit,
                ..
            }
        ));
    }

    #[test]
    fn available_margin_caps_quantity_independently() {
        let mut request = sample_request();
        request.account_state.equity = Decimal::from(10_000);
        request.policy.max_risk_per_trade_pct = Decimal::from(10);
        request.policy.max_position_size_pct = Decimal::from(100);
        request.policy.max_total_exposure_pct = Decimal::from(100);
        request.policy.max_leverage = Decimal::from(100);
        request.entry_price = Some(Decimal::from(100));
        request.stop_loss = Some(Decimal::from(99));
        request.requested_quantity = Some(Decimal::from(10));
        let mut sizing = context();
        sizing.available_margin = Some(Decimal::from(25));
        sizing.margin_per_quantity = Some(Decimal::from(10));

        let result = size_order_instrument_aware(&request, &sizing);
        assert_eq!(result.quantity(), Some(Decimal::new(25, 1)));
        assert!(matches!(
            result,
            InstrumentSizingOutcome::Resized {
                reason: SizingLimitReason::MarginLimit,
                ..
            }
        ));
    }

    #[test]
    fn mismatched_margin_inputs_fail_closed() {
        let request = sample_request();
        let mut sizing = context();
        sizing.available_margin = Some(Decimal::from(100));

        assert_eq!(
            size_order_instrument_aware(&request, &sizing),
            InstrumentSizingOutcome::Rejected {
                reason: SizingRejectReason::InvalidInput,
            }
        );
    }

    #[test]
    fn pending_margin_reserve_is_subtracted_before_margin_sizing() {
        let mut request = sample_request();
        request.account_state.equity = Decimal::from(10_000);
        request.policy.max_risk_per_trade_pct = Decimal::from(10);
        request.policy.max_position_size_pct = Decimal::from(100);
        request.policy.max_total_exposure_pct = Decimal::from(100);
        request.policy.max_leverage = Decimal::from(100);
        request.entry_price = Some(Decimal::from(100));
        request.stop_loss = Some(Decimal::from(99));
        request.requested_quantity = Some(Decimal::from(10));
        let mut sizing = context();
        sizing.available_margin = Some(Decimal::from(25));
        sizing.pending_order_margin = Decimal::from(5);
        sizing.margin_per_quantity = Some(Decimal::from(10));

        let result = size_order_instrument_aware(&request, &sizing);
        assert_eq!(result.quantity(), Some(Decimal::from(2)));
        assert!(matches!(
            result,
            InstrumentSizingOutcome::Resized {
                reason: SizingLimitReason::MarginLimit,
                ..
            }
        ));
    }

    #[test]
    fn deterministic_quantity_invariants_hold_across_risk_budgets() {
        for budget_pct in 1..=20 {
            let mut request = sample_request();
            request.account_state.equity = Decimal::from(10_000);
            request.policy.max_risk_per_trade_pct = Decimal::from(budget_pct);
            request.policy.max_position_size_pct = Decimal::from(100);
            request.policy.max_total_exposure_pct = Decimal::from(100);
            request.policy.max_leverage = Decimal::from(100);
            request.entry_price = Some(Decimal::from(100));
            request.stop_loss = Some(Decimal::from(99));
            request.requested_quantity = Some(Decimal::from(1_000));
            let sizing = context();

            let first = size_order_instrument_aware(&request, &sizing);
            let second = size_order_instrument_aware(&request, &sizing);
            assert_eq!(first, second);
            if let InstrumentSizingOutcome::Approved {
                quantity,
                modeled_loss,
                risk_budget,
            }
            | InstrumentSizingOutcome::Resized {
                quantity,
                modeled_loss,
                risk_budget,
                ..
            } = first
            {
                assert_eq!(
                    (quantity / sizing.quantity_step).trunc(),
                    quantity / sizing.quantity_step
                );
                assert!(modeled_loss <= risk_budget);
            }
        }
    }
}
