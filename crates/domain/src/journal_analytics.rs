//! Journal domain analytics and position sizing helpers.
//!
//! This is educational/risk-estimation tooling. It does not place orders,
//! modify execution account state, or override the execution Risk Engine.

use rust_decimal::Decimal;

use crate::error::{DomainError, DomainResult};
use crate::money::require_positive;

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
        return Err(DomainError::Validation(
            "risk percent cannot exceed 100".into(),
        ));
    }
    require_positive(input.entry_price, "entry price")?;
    require_positive(input.stop_loss, "stop loss")?;
    require_positive(
        input.value_per_price_unit_per_lot,
        "value per price unit per lot",
    )?;
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
        return Err(DomainError::Validation(
            "analytics requires at least one closed trade".into(),
        ));
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
        trade_count,
        wins,
        losses,
        breakeven,
        win_rate_percent,
        gross_profit,
        gross_loss,
        net_profit,
        profit_factor,
        expectancy,
        average_win,
        average_loss,
        max_drawdown,
    })
}


/// Canonical, validated trade-level fact consumed by Journal analytics.
/// Nullable fields stay nullable so incomplete source records are never imputed.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeAnalyticsFact {
    pub trade_id: String,
    pub symbol: String,
    pub direction: String,
    pub setup: String,
    pub session: Option<String>,
    pub opened_at: chrono::DateTime<chrono::Utc>,
    pub closed_at: chrono::DateTime<chrono::Utc>,
    pub net_pnl: Decimal,
    pub risk_amount: Option<Decimal>,
    pub stored_r_multiple: Option<Decimal>,
    pub mfe_r: Option<Decimal>,
    pub mae_r: Option<Decimal>,
    pub setup_valid: Option<bool>,
    pub rules_followed: Option<bool>,
    pub emotional_trade: Option<bool>,
    pub mistake: Option<bool>,
    pub discipline_score: Option<i16>,
    pub psychology: TradePsychologyFact,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TradePsychologyFact {
    pub emotion_before: Option<String>,
    pub emotion_during: Option<String>,
    pub emotion_after: Option<String>,
    pub confidence: Option<i16>,
    pub fear: Option<i16>,
    pub fomo: Option<i16>,
    pub frustration: Option<i16>,
    pub revenge_intensity: Option<i16>,
    pub boredom: Option<i16>,
}

impl TradeAnalyticsFact {
    pub fn validate(&self) -> DomainResult<()> {
        if self.trade_id.trim().is_empty() || self.symbol.trim().is_empty() {
            return Err(DomainError::Validation("analytics trade ID and symbol must be non-empty".into()));
        }
        if !matches!(self.direction.as_str(), "long" | "short") {
            return Err(DomainError::Validation("analytics trade direction must be long or short".into()));
        }
        if self.closed_at < self.opened_at {
            return Err(DomainError::Validation("analytics close timestamp cannot precede open timestamp".into()));
        }
        if self.risk_amount.is_some_and(|value| value < Decimal::ZERO) {
            return Err(DomainError::Validation("analytics risk amount cannot be negative".into()));
        }
        if self.mfe_r.is_some_and(|value| value < Decimal::ZERO)
            || self.mae_r.is_some_and(|value| value > Decimal::ZERO)
        {
            return Err(DomainError::Validation("MFE R must be non-negative and MAE R must be non-positive".into()));
        }
        if self.discipline_score.is_some_and(|value| !(0..=100).contains(&value)) {
            return Err(DomainError::Validation("discipline score must be between 0 and 100".into()));
        }
        let ratings = [self.psychology.confidence, self.psychology.fear, self.psychology.fomo,
            self.psychology.frustration, self.psychology.revenge_intensity, self.psychology.boredom];
        if ratings.iter().flatten().any(|value| !(0..=10).contains(value)) {
            return Err(DomainError::Validation("psychology ratings must be between 0 and 10".into()));
        }
        Ok(())
    }

    pub fn realized_r(&self) -> Option<Decimal> {
        self.stored_r_multiple.or_else(|| self.risk_amount
            .filter(|risk| *risk > Decimal::ZERO).map(|risk| self.net_pnl / risk))
    }

    pub fn holding_seconds(&self) -> i64 {
        self.closed_at.signed_duration_since(self.opened_at).num_seconds().max(0)
    }

    pub fn mfe_capture_percent(&self) -> Option<Decimal> {
        match (self.realized_r(), self.mfe_r) {
            (Some(realized), Some(mfe)) if mfe > Decimal::ZERO =>
                Some(realized / mfe * Decimal::from(100)),
            _ => None,
        }
    }

    pub fn profit_surrendered_r(&self) -> Option<Decimal> {
        match (self.mfe_r, self.realized_r()) {
            (Some(mfe), Some(realized)) if mfe > realized => Some(mfe - realized),
            _ => None,
        }
    }
}

#[cfg(test)]
mod trade_analytics_fact_tests {
    #![allow(clippy::unwrap_used)]
    use super::{TradeAnalyticsFact, TradePsychologyFact};
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;

    fn fixture() -> TradeAnalyticsFact {
        TradeAnalyticsFact {
            trade_id: "trade-1".into(), symbol: "EURUSD".into(), direction: "long".into(),
            setup: "breakout".into(), session: Some("London".into()),
            opened_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            closed_at: Utc.timestamp_opt(1_700_003_600, 0).unwrap(),
            net_pnl: Decimal::from(100), risk_amount: Some(Decimal::from(50)),
            stored_r_multiple: None, mfe_r: Some(Decimal::from(3)), mae_r: Some(Decimal::new(-5, 1)),
            setup_valid: Some(true), rules_followed: Some(true), emotional_trade: Some(false),
            mistake: Some(false), discipline_score: Some(90),
            psychology: TradePsychologyFact { confidence: Some(8), fear: Some(2), ..TradePsychologyFact::default() },
        }
    }

    #[test]
    fn validates_and_derives_trade_level_execution_metrics() {
        let fact = fixture();
        assert!(fact.validate().is_ok());
        assert_eq!(fact.realized_r(), Some(Decimal::from(2)));
        assert_eq!(fact.holding_seconds(), 3_600);
        assert_eq!(fact.mfe_capture_percent(), Some(Decimal::from(2) / Decimal::from(3) * Decimal::from(100)));
        assert_eq!(fact.profit_surrendered_r(), Some(Decimal::ONE));
    }

    #[test]
    fn preserves_missing_risk_and_excursion_as_missing() {
        let mut fact = fixture(); fact.risk_amount = None; fact.stored_r_multiple = None; fact.mfe_r = None;
        assert_eq!(fact.realized_r(), None);
        assert_eq!(fact.mfe_capture_percent(), None);
        assert_eq!(fact.profit_surrendered_r(), None);
    }

    #[test]
    fn rejects_invalid_timestamp_and_psychology_rating() {
        let mut fact = fixture(); fact.closed_at = fact.opened_at - chrono::Duration::seconds(1);
        assert!(fact.validate().is_err());
        let mut fact = fixture(); fact.psychology.fear = Some(11);
        assert!(fact.validate().is_err());
    }
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
        })
        .unwrap();
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
        let s = summarize_net_pnl(&[Decimal::from(100), Decimal::from(-40), Decimal::from(-10)])
            .unwrap();
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
