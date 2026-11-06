//! Journal domain analytics and position sizing helpers.
//!
//! This is educational/risk-estimation tooling. It does not place orders,
//! modify execution account state, or override the execution Risk Engine.

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::error::{DomainError, DomainResult};
use crate::money::{require_non_negative, require_positive};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionSizingInput {
    pub account_balance: Decimal,
    /// Risk percent, e.g. 1.0 means one percent.
    pub risk_percent: Decimal,
    pub entry_price: Decimal,
    pub stop_loss: Decimal,
    /// Cash value per one price-unit movement for one lot/contract.
    pub value_per_price_unit_per_lot: Decimal,
    /// Broker minimum quantity increment, e.g. 0.01 lot.
    pub quantity_step: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionSizingResult {
    pub risk_budget: Decimal,
    pub stop_distance: Decimal,
    pub raw_quantity: Decimal,
    pub rounded_quantity: Decimal,
    pub estimated_risk: Decimal,
}

pub fn calculate_position_size(input: PositionSizingInput) -> DomainResult<PositionSizingResult> {
    require_positive(input.account_balance, "account balance")?;
    require_positive(input.risk_percent, "risk percent")?;
    if input.risk_percent > Decimal::from(100) {
        return Err(DomainError::Validation("risk percent cannot exceed 100".into()));
    }
    require_positive(input.entry_price, "entry price")?;
    require_positive(input.stop_loss, "stop loss")?;
    require_positive(input.value_per_price_unit_per_lot, "value per price unit per lot")?;
    require_positive(input.quantity_step, "quantity step")?;

    let distance = (input.entry_price - input.stop_loss).abs();
    require_positive(distance, "stop distance")?;
    let risk_budget = input.account_balance * input.risk_percent / Decimal::from(100);
    let raw_quantity = risk_budget / (distance * input.value_per_price_unit_per_lot);
    let steps = (raw_quantity / input.quantity_step).floor();
    let rounded_quantity = steps * input.quantity_step;
    let estimated_risk = rounded_quantity * distance * input.value_per_price_unit_per_lot;

    Ok(PositionSizingResult {
        risk_budget,
        stop_distance: distance,
        raw_quantity,
        rounded_quantity,
        estimated_risk,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalyticsSummary {
    pub trade_count: usize,
    pub wins: usize,
    pub losses: usize,
    pub breakeven: usize,
    pub win_rate_percent: Decimal,
    pub gross_profit: Decimal,
    pub gross_loss: Decimal,
    pub net_profit: Decimal,
    pub profit_factor: Option<Decimal>,
    pub expectancy: Decimal,
    pub average_win: Option<Decimal>,
    pub average_loss: Option<Decimal>,
    pub max_drawdown: Decimal,
}

pub fn summarize_net_pnl(pnls: &[Decimal]) -> DomainResult<AnalyticsSummary> {
    let trade_count = pnls.len();
    if trade_count == 0 {
        return Err(DomainError::Validation("analytics requires at least one closed trade".into()));
    }
    let mut wins = 0usize;
    let mut losses = 0usize;
    let mut breakeven = 0usize;
    let mut gross_profit = Decimal::ZERO;
    let mut gross_loss = Decimal::ZERO;
    let mut equity = Decimal::ZERO;
    let mut peak = Decimal::ZERO;
    let mut max_drawdown = Decimal::ZERO;

    for pnl in pnls {
        if *pnl > Decimal::ZERO {
            wins += 1;
            gross_profit += *pnl;
        } else if *pnl < Decimal::ZERO {
            losses += 1;
            gross_loss += pnl.abs();
        } else {
            breakeven += 1;
        }
        equity += *pnl;
        if equity > peak {
            peak = equity;
        }
        let drawdown = peak - equity;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }

    let count = Decimal::from(trade_count as u64);
    let win_rate_percent = Decimal::from(wins as u64) * Decimal::from(100) / count;
    let net_profit = pnls.iter().copied().sum::<Decimal>();
    let profit_factor = if gross_loss > Decimal::ZERO {
        Some(gross_profit / gross_loss)
    } else if gross_profit > Decimal::ZERO {
        None // Infinite; represent explicitly as unavailable instead of infinity.
    } else {
        Some(Decimal::ZERO)
    };
    let average_win = (wins > 0).then(|| gross_profit / Decimal::from(wins as u64));
    let average_loss = (losses > 0).then(|| gross_loss / Decimal::from(losses as u64));
    let expectancy = net_profit / count;

    Ok(AnalyticsSummary {
        trade_count, wins, losses, breakeven, win_rate_percent, gross_profit,
        gross_loss, net_profit, profit_factor, expectancy, average_win,
        average_loss, max_drawdown,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn sizing_respects_step_and_risk_cap() {
        let result = calculate_position_size(PositionSizingInput {
            account_balance: Decimal::from(10_000),
            risk_percent: Decimal::ONE,
            entry_price: Decimal::from(100),
            stop_loss: Decimal::from(95),
            value_per_price_unit_per_lot: Decimal::ONE,
            quantity_step: Decimal::new(1, 0),
        }).unwrap();
        assert_eq!(result.risk_budget, Decimal::from(100));
        assert_eq!(result.rounded_quantity, Decimal::from(20));
        assert_eq!(result.estimated_risk, Decimal::from(100));
    }

    #[test]
    fn rejects_zero_stop_distance() {
        let result = calculate_position_size(PositionSizingInput {
            account_balance: Decimal::from(1000),
            risk_percent: Decimal::ONE,
            entry_price: Decimal::from(10),
            stop_loss: Decimal::from(10),
            value_per_price_unit_per_lot: Decimal::ONE,
            quantity_step: Decimal::new(1, 2),
        });
        assert!(result.is_err());
    }

    #[test]
    fn pnl_summary_calculates_expectancy_and_drawdown() {
        let s = summarize_net_pnl(&[Decimal::from(100), Decimal::from(-40), Decimal::from(-10)]).unwrap();
        assert_eq!(s.net_profit, Decimal::from(50));
        assert_eq!(s.expectancy, Decimal::new(50, 0) / Decimal::from(3));
        assert_eq!(s.max_drawdown, Decimal::from(50));
        assert_eq!(s.win_rate_percent, Decimal::new(100, 0) / Decimal::from(3));
    }

    #[test]
    fn analytics_empty_series_is_invalid() {
        assert!(summarize_net_pnl(&[]).is_err());
    }
}
