//! Deterministic regression tests for the public Journal analytics domain contract.
//!
//! These fixtures deliberately cover known records and incomplete imports. They
//! prevent analytics changes from silently imputing unavailable risk/excursion data
//! or changing the definitions of realized R, holding time and drawdown.

use chrono::{Duration, TimeZone, Utc};
use domain::{summarize_net_pnl, TradeAnalyticsFact, TradePsychologyFact};
use rust_decimal::Decimal;

fn fact() -> TradeAnalyticsFact {
    let opened_at = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();
    TradeAnalyticsFact {
        trade_id: "regression-001".into(),
        symbol: "EURUSD".into(),
        direction: "long".into(),
        setup: "breakout".into(),
        session: Some("London".into()),
        opened_at,
        closed_at: opened_at + Duration::seconds(5_400),
        net_pnl: Decimal::from(150),
        entry_price: Decimal::new(1_1000, 4),
        exit_price: Some(Decimal::new(1_1030, 4)),
        stop_loss: Some(Decimal::new(1_0950, 4)),
        take_profit: Some(Decimal::new(1_1100, 4)),
        risk_amount: Some(Decimal::from(50)),
        stored_r_multiple: None,
        mfe_r: Some(Decimal::from(3)),
        mae_r: Some(Decimal::new(-5, 1)),
        mfe_source: Some("candles".into()),
        mae_source: Some("candles".into()),
        mfe_precision: Some("M1".into()),
        mae_precision: Some("M1".into()),
        setup_valid: Some(true),
        rules_followed: Some(true),
        emotional_trade: Some(false),
        mistake: Some(false),
        discipline_score: Some(85),
        psychology: TradePsychologyFact {
            confidence: Some(8),
            fear: Some(2),
            fomo: Some(0),
            frustration: Some(1),
            revenge_intensity: Some(0),
            boredom: Some(0),
            ..TradePsychologyFact::default()
        },
    }
}

#[test]
fn known_trade_record_has_expected_execution_metrics() {
    let trade = fact();
    assert!(trade.validate().is_ok());
    assert_eq!(trade.realized_r(), Some(Decimal::from(3)));
    assert_eq!(trade.holding_seconds(), 5_400);
    assert_eq!(trade.mfe_capture_percent(), Some(Decimal::from(100)));
    assert_eq!(trade.profit_surrendered_r(), None);
}

#[test]
fn stored_realized_r_takes_precedence_over_pnl_over_risk_fallback() {
    let mut trade = fact();
    trade.stored_r_multiple = Some(Decimal::new(25, 1));
    assert_eq!(trade.realized_r(), Some(Decimal::new(25, 1)));
}

#[test]
fn missing_risk_and_excursions_remain_unknown() {
    let mut trade = fact();
    trade.risk_amount = None;
    trade.stored_r_multiple = None;
    trade.mfe_r = None;
    trade.mae_r = None;
    assert!(trade.validate().is_ok());
    assert_eq!(trade.realized_r(), None);
    assert_eq!(trade.mfe_capture_percent(), None);
    assert_eq!(trade.profit_surrendered_r(), None);
}

#[test]
fn negative_realized_r_is_not_clipped_in_mfe_capture() {
    let mut trade = fact();
    trade.net_pnl = Decimal::from(-25);
    assert_eq!(trade.realized_r(), Some(Decimal::new(-5, 1)));
    assert_eq!(
        trade.mfe_capture_percent(),
        Some(Decimal::new(-5, 1) / Decimal::from(3) * Decimal::from(100))
    );
}

#[test]
fn zero_or_negative_risk_does_not_create_fallback_r() {
    let mut trade = fact();
    trade.risk_amount = Some(Decimal::ZERO);
    trade.stored_r_multiple = None;
    assert_eq!(trade.realized_r(), None);
    trade.risk_amount = Some(Decimal::from(-1));
    assert!(trade.validate().is_err());
}

#[test]
fn invalid_excursion_signs_and_rating_ranges_are_rejected() {
    let mut trade = fact();
    trade.mfe_r = Some(Decimal::new(-1, 1));
    assert!(trade.validate().is_err());
    let mut trade = fact();
    trade.mae_r = Some(Decimal::new(1, 1));
    assert!(trade.validate().is_err());
    let mut trade = fact();
    trade.psychology.fear = Some(11);
    assert!(trade.validate().is_err());
    let mut trade = fact();
    trade.discipline_score = Some(101);
    assert!(trade.validate().is_err());
}

#[test]
fn zero_duration_is_valid_and_negative_duration_is_rejected() {
    let mut trade = fact();
    trade.closed_at = trade.opened_at;
    assert!(trade.validate().is_ok());
    assert_eq!(trade.holding_seconds(), 0);
    trade.closed_at = trade.opened_at - Duration::seconds(1);
    assert!(trade.validate().is_err());
}

#[test]
fn known_pnl_series_has_deterministic_summary_and_drawdown() {
    // Equity path: 0 -> 100 -> 60 -> 50 -> 130 -> 110.
    let summary = summarize_net_pnl(&[
        Decimal::from(100),
        Decimal::from(-40),
        Decimal::from(-10),
        Decimal::from(80),
        Decimal::from(-20),
    ])
    .unwrap();
    assert_eq!(summary.trade_count, 5);
    assert_eq!(summary.wins, 2);
    assert_eq!(summary.losses, 3);
    assert_eq!(summary.breakeven, 0);
    assert_eq!(summary.gross_profit, Decimal::from(180));
    assert_eq!(summary.gross_loss, Decimal::from(70));
    assert_eq!(summary.net_profit, Decimal::from(110));
    assert_eq!(summary.max_drawdown, Decimal::from(50));
    assert_eq!(summary.expectancy, Decimal::from(22));
    assert_eq!(summary.win_rate_percent, Decimal::from(40));
}

#[test]
fn all_win_series_keeps_infinite_profit_factor_explicitly_unavailable() {
    let summary = summarize_net_pnl(&[Decimal::from(10), Decimal::from(20)]).unwrap();
    assert_eq!(summary.profit_factor, None);
    assert_eq!(summary.max_drawdown, Decimal::ZERO);
}

#[test]
fn breakeven_is_classified_separately_from_wins_and_losses() {
    // The API derives streaks in close-time order. This sequence pins the
    // summary population's breakeven classification independently.
    let summary =
        summarize_net_pnl(&[Decimal::from(10), Decimal::ZERO, Decimal::from(-5)]).unwrap();
    assert_eq!(summary.wins, 1);
    assert_eq!(summary.losses, 1);
    assert_eq!(summary.breakeven, 1);
}
