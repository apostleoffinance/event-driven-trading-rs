//! Deterministic journal calculations. These calculations never affect execution.

use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc, Weekday};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedTradeMetrics {
    pub gross_pnl: Option<Decimal>,
    pub net_pnl: Option<Decimal>,
    pub r_multiple: Option<Decimal>,
    pub risk_amount: Option<Decimal>,
    pub session: String,
}

fn nth_sunday(year: i32, month: u32, nth: u32) -> u32 {
    let first_weekday = NaiveDate::from_ymd_opt(year, month, 1)
        .map(|date| date.weekday().num_days_from_sunday())
        .unwrap_or(0);
    let first_sunday_offset = (7 - first_weekday) % 7;
    1 + first_sunday_offset + (nth - 1) * 7
}

fn last_sunday(year: i32, month: u32) -> u32 {
    (1..=31)
        .rev()
        .find(|day| {
            NaiveDate::from_ymd_opt(year, month, *day)
                .is_some_and(|date| date.weekday() == Weekday::Sun)
        })
        .unwrap_or(1)
}

fn london_dst(at: &DateTime<Utc>) -> bool {
    let month = at.month();
    let day = at.day();
    let hour = at.hour();
    match month {
        4..=9 => true,
        3 => day > last_sunday(at.year(), 3) || (day == last_sunday(at.year(), 3) && hour >= 1),
        10 => day < last_sunday(at.year(), 10) || (day == last_sunday(at.year(), 10) && hour < 1),
        _ => false,
    }
}

fn new_york_dst(at: &DateTime<Utc>) -> bool {
    let month = at.month();
    let day = at.day();
    let hour = at.hour();
    match month {
        4..=10 => true,
        3 => day > nth_sunday(at.year(), 3, 2) || (day == nth_sunday(at.year(), 3, 2) && hour >= 7),
        11 => {
            day < nth_sunday(at.year(), 11, 1) || (day == nth_sunday(at.year(), 11, 1) && hour < 6)
        }
        _ => false,
    }
}

/// Classify UTC timestamps against London and New York local session hours,
/// including the distinct US/UK daylight-saving transition dates.
pub fn classify_fx_session(at: &DateTime<Utc>) -> String {
    let london_hour = (at.hour() as i32 + if london_dst(at) { 1 } else { 0 }) % 24;
    let new_york_offset = if new_york_dst(at) { 4 } else { 5 };
    let new_york_hour = (at.hour() as i32 - new_york_offset + 24) % 24;
    let london_open = (8..17).contains(&london_hour);
    let new_york_open = (8..17).contains(&new_york_hour);

    match (london_open, new_york_open) {
        (true, true) => "London/New York overlap".into(),
        (true, false) => "London".into(),
        (false, true) => "New York".into(),
        _ => "Asia/Pacific".into(),
    }
}

/// Calculate standard-lot FX P&L only when quote currency matches P&L currency.
/// Non-FX instruments and currency conversions remain unset rather than guessed.
#[allow(clippy::too_many_arguments)]
pub fn calculate_fx_metrics(
    symbol: &str,
    direction_long: bool,
    quantity_lots: Decimal,
    entry: Decimal,
    exit: Option<Decimal>,
    stop_loss: Option<Decimal>,
    commission: Decimal,
    swap: Decimal,
    other_fees: Decimal,
    pnl_currency: &str,
    risk_amount: Option<Decimal>,
    opened_at: &DateTime<Utc>,
) -> DerivedTradeMetrics {
    const CURRENCIES: [&str; 32] = [
        "USD", "EUR", "GBP", "JPY", "CHF", "CAD", "AUD", "NZD", "SEK", "NOK", "DKK", "PLN", "CZK",
        "HUF", "TRY", "SGD", "HKD", "MXN", "ZAR", "CNH", "CNY", "INR", "BRL", "RUB", "ILS", "THB",
        "KRW", "PHP", "IDR", "MYR", "AED", "SAR",
    ];
    let pair = symbol
        .chars()
        .filter(|character| character.is_ascii_alphabetic())
        .take(6)
        .collect::<String>()
        .to_ascii_uppercase();
    let quote = if pair.len() == 6 {
        let base = &pair[0..3];
        let quote = &pair[3..6];
        if CURRENCIES.contains(&base) && CURRENCIES.contains(&quote) {
            Some(quote)
        } else {
            None
        }
    } else {
        None
    };

    let gross = match (quote, exit) {
        (Some(quote), Some(exit)) if quote == pnl_currency => {
            let movement = if direction_long {
                exit - entry
            } else {
                entry - exit
            };
            Some(movement * quantity_lots * Decimal::from(100_000u32))
        }
        _ => None,
    };

    let derived_risk = match (quote, stop_loss) {
        (Some(quote), Some(stop)) if quote == pnl_currency => {
            Some((entry - stop).abs() * quantity_lots * Decimal::from(100_000u32))
        }
        _ => None,
    };
    let effective_risk = risk_amount.or(derived_risk);

    // Commission and other fees are costs whether entered signed or unsigned.
    // Swap remains signed because positive swap is a credit.
    let net = gross.map(|value| value - commission.abs() + swap - other_fees.abs());
    let r_multiple = match (net, effective_risk.filter(|risk| *risk > Decimal::ZERO)) {
        (Some(value), Some(risk)) => Some(value / risk),
        _ => None,
    };

    DerivedTradeMetrics {
        gross_pnl: gross,
        net_pnl: net,
        r_multiple,
        risk_amount: derived_risk,
        session: if quote.is_some() {
            classify_fx_session(opened_at)
        } else {
            "Not classified".into()
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use chrono::TimeZone;

    fn at(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 12, hour, 0, 0).unwrap()
    }

    #[test]
    fn calculates_long_short_pnl_risk_and_fees() {
        let long = calculate_fx_metrics(
            "EURUSD",
            true,
            Decimal::ONE,
            Decimal::new(10_000, 4),
            Some(Decimal::new(10_010, 4)),
            Some(Decimal::new(9_995, 4)),
            Decimal::from(2),
            Decimal::from(-1),
            Decimal::ZERO,
            "USD",
            None,
            &at(9),
        );
        assert_eq!(long.gross_pnl, Some(Decimal::from(100)));
        assert_eq!(long.net_pnl, Some(Decimal::from(97)));
        assert_eq!(long.risk_amount, Some(Decimal::from(50)));
        assert_eq!(long.r_multiple, Some(Decimal::new(194, 2)));

        let short = calculate_fx_metrics(
            "EURUSD",
            false,
            Decimal::ONE,
            Decimal::new(10_010, 4),
            Some(Decimal::new(10_000, 4)),
            None,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            "USD",
            None,
            &at(13),
        );
        assert_eq!(short.gross_pnl, Some(Decimal::from(100)));
    }

    #[test]
    fn does_not_guess_currency_conversion_or_non_fx_pnl() {
        let result = calculate_fx_metrics(
            "EURGBP",
            true,
            Decimal::ONE,
            Decimal::ONE,
            Some(Decimal::new(101, 2)),
            None,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            "USD",
            None,
            &at(9),
        );
        assert_eq!(result.gross_pnl, None);
        assert_eq!(result.net_pnl, None);

        let metal = calculate_fx_metrics(
            "XAUUSD",
            true,
            Decimal::ONE,
            Decimal::ONE,
            Some(Decimal::from(2)),
            None,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            "USD",
            None,
            &at(9),
        );
        assert_eq!(metal.gross_pnl, None);
        assert_eq!(metal.session, "Not classified");
    }

    #[test]
    fn classifies_dst_aware_sessions() {
        assert_eq!(classify_fx_session(&at(3)), "Asia/Pacific");
        assert_eq!(classify_fx_session(&at(9)), "London");
        assert_eq!(classify_fx_session(&at(13)), "London/New York overlap");
        assert_eq!(classify_fx_session(&at(18)), "New York");

        let before_us_dst = Utc.with_ymd_and_hms(2026, 3, 7, 12, 30, 0).unwrap();
        let after_us_dst = Utc.with_ymd_and_hms(2026, 3, 8, 12, 30, 0).unwrap();
        assert_eq!(classify_fx_session(&before_us_dst), "London");
        assert_eq!(
            classify_fx_session(&after_us_dst),
            "London/New York overlap"
        );

        let before_us_standard = Utc.with_ymd_and_hms(2026, 10, 31, 12, 30, 0).unwrap();
        let after_us_standard = Utc.with_ymd_and_hms(2026, 11, 1, 12, 30, 0).unwrap();
        assert_eq!(
            classify_fx_session(&before_us_standard),
            "London/New York overlap"
        );
        assert_eq!(classify_fx_session(&after_us_standard), "London");
    }
}
