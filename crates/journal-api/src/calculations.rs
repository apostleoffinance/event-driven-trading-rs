//! Deterministic journal calculations. These calculations never affect execution.
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedTradeMetrics {
    pub gross_pnl: Option<Decimal>,
    pub net_pnl: Option<Decimal>,
    pub r_multiple: Option<Decimal>,
    pub session: String,
}

/// Classify the UTC entry timestamp into commonly used FX sessions.
pub fn classify_fx_session(at: DateTime<Utc>) -> String {
    match at.hour() {
        0..=6 => "Asia".into(),
        7..=11 => "London".into(),
        12..=15 => "London/New York overlap".into(),
        16..=20 => "New York".into(),
        _ => "Outside major sessions".into(),
    }
}

/// Calculate standard-lot FX P&L only when quote currency matches P&L currency.
/// Non-FX instruments and currency conversions remain unset rather than guessed.
pub fn calculate_fx_metrics(
    symbol: &str, direction_long: bool, quantity_lots: Decimal,
    entry: Decimal, exit: Option<Decimal>, commission: Decimal,
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
    let net = gross.map(|value| value + commission + swap + other_fees);
    let r_multiple = match (net, risk_amount.filter(|r| *r > Decimal::ZERO)) {
        (Some(value), Some(risk)) => Some(value / risk),
        _ => None,
    };
    DerivedTradeMetrics { gross_pnl: gross, net_pnl: net, r_multiple, session: classify_fx_session(opened_at) }
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
        assert_eq!(long.r_multiple, Some(Decimal::new(97, 2)));
        let short = calculate_fx_metrics("EURUSD", false, Decimal::ONE,
            Decimal::new(10100, 4), Some(Decimal::new(10000, 4)),
            Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, "USD", None, at(13));
        assert_eq!(short.gross_pnl, Some(Decimal::from(100)));
    }
    #[test]
    fn does_not_guess_currency_conversion_or_open_trade_pnl() {
        let result = calculate_fx_metrics("EURGBP", true, Decimal::ONE,
            Decimal::ONE, Some(Decimal::new(101, 2)), Decimal::ZERO,
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
