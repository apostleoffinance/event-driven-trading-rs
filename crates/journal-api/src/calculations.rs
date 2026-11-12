//! Deterministic journal calculations. These calculations never affect execution.
use chrono::{DateTime, Timelike, Utc};
use chrono_tz::{America::New_York, Europe::London};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedTradeMetrics {
    pub gross_pnl: Option<Decimal>,
    pub net_pnl: Option<Decimal>,
    pub r_multiple: Option<Decimal>,
    pub risk_amount: Option<Decimal>,
    pub session: String,
}

/// Classify the UTC entry timestamp into commonly used FX sessions.
pub fn classify_fx_session(at: DateTime<Utc>) -> String {
    let london = at.with_timezone(&London);
    let new_york = at.with_timezone(&New_York);
    let london_open = (8..17).contains(&london.hour());
    let new_york_open = (8..17).contains(&new_york.hour());
    match (london_open, new_york_open) {
        (true, true) => "London/New York overlap".into(),
        (true, false) => "London".into(),
        (false, true) => "New York".into(),
        (false, false) if (7..16).contains(&at.hour()) => "Europe".into(),
        _ => "Asia/Pacific".into(),
    }
}

/// Calculate standard-lot FX P&L only when quote currency matches P&L currency.
/// Non-FX instruments and currency conversions remain unset rather than guessed.
pub fn calculate_fx_metrics(
    symbol: &str, direction_long: bool, quantity_lots: Decimal,
    entry: Decimal, exit: Option<Decimal>, stop_loss: Option<Decimal>, commission: Decimal,
    swap: Decimal, other_fees: Decimal, pnl_currency: &str,
    risk_amount: Option<Decimal>, opened_at: DateTime<Utc>,
) -> DerivedTradeMetrics {
    let pair = symbol.trim().to_ascii_uppercase();
    let quote = pair.get(3..6).filter(|_| pair.len() == 6);
    let gross = match (quote, exit) {
        (Some(quote), Some(exit)) if quote == pnl_currency => {
            let movement = if direction_long { exit - entry } else { entry - exit };
            Some(movement * quantity_lots * Decimal::from(100_000u32))
        }
        _ => None,
    };
    let derived_risk = match (quote, stop_loss) {
        (Some(quote), Some(stop)) if quote == pnl_currency => Some((entry - stop).abs() * quantity_lots * Decimal::from(100_000u32)),
        _ => None,
    };
    let effective_risk = risk_amount.or(derived_risk);
    // Commission and other fees are costs whether users enter them signed or unsigned;
    // swap remains signed because positive swap is a credit.
    let net = gross.map(|value| value - commission.abs() + swap - other_fees.abs());
    let r_multiple = match (net, effective_risk.filter(|r| *r > Decimal::ZERO)) {
        (Some(value), Some(risk)) => Some(value / risk),
        _ => None,
    };
    DerivedTradeMetrics { gross_pnl: gross, net_pnl: net, r_multiple, risk_amount: derived_risk, session: classify_fx_session(opened_at) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    fn at(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 12, hour, 0, 0).unwrap()
    }
    #[test]
    fn calculates_long_short_pnl_and_fees() {
        let long = calculate_fx_metrics("EURUSD", true, Decimal::ONE,
            Decimal::new(10000, 4), Some(Decimal::new(10100, 4)),
            Decimal::new(-2, 0), Decimal::new(-1, 0), Decimal::ZERO,
            "USD", Some(Decimal::new(100, 0)), at(9));
        assert_eq!(long.gross_pnl, Some(Decimal::from(100)));
        assert_eq!(long.net_pnl, Some(Decimal::from(97)));
        assert_eq!(long.risk_amount, Some(Decimal::from(50)));
        assert_eq!(long.r_multiple, Some(Decimal::new(97, 2)));
        let short = calculate_fx_metrics("EURUSD", false, Decimal::ONE,
            Decimal::new(10100, 4), Some(Decimal::new(10000, 4)),
            Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, "USD", None, at(13));
        assert_eq!(short.gross_pnl, Some(Decimal::from(100)));
    }
    #[test]
    fn does_not_guess_currency_conversion_or_open_trade_pnl() {
        let result = calculate_fx_metrics("EURGBP", true, Decimal::ONE,
            Decimal::ONE, Some(Decimal::new(101, 2)), None, Decimal::ZERO,
            Decimal::ZERO, Decimal::ZERO, "USD", None, at(9));
        assert_eq!(result.gross_pnl, None);
        assert_eq!(result.net_pnl, None);
    }
    #[test]
    fn classifies_sessions() {
        assert_eq!(classify_fx_session(at(3)), "Asia");
        assert_eq!(classify_fx_session(at(9)), "London");
        assert_eq!(classify_fx_session(at(13)), "London/New York overlap");
        assert_eq!(classify_fx_session(at(18)), "New York");
    }
}
