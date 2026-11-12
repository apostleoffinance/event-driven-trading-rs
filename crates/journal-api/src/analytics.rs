//! Authenticated read-only analytics over the caller's Journal facts.
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use domain::{summarize_net_pnl, TradeAnalyticsFact, TradePsychologyFact};
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::FromRow;

use crate::{auth::ApiAuthError, AppState};

#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    pub journal_account_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub symbol: Option<String>,
    pub setup_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct TradeFact {
    id: String,
    symbol: String,
    direction: String,
    opened_at: DateTime<Utc>,
    closed_at: DateTime<Utc>,
    net_pnl: Decimal,
    risk_amount: Option<Decimal>,
    entry_price: Decimal,
    exit_price: Option<Decimal>,
    stop_loss: Option<Decimal>,
    take_profit: Option<Decimal>,
    r_multiple: Option<Decimal>,
    session: Option<String>,
    setup_name: String,
    mfe_r: Option<Decimal>,
    mae_r: Option<Decimal>,
    mfe_source: Option<String>,
    mae_source: Option<String>,
    mfe_precision: Option<String>,
    mae_precision: Option<String>,
    setup_valid: Option<bool>,
    rules_followed: Option<bool>,
    emotional_trade: Option<bool>,
    mistake: Option<bool>,
    discipline_score: Option<i16>,
    emotion_before: Option<String>,
    emotion_during: Option<String>,
    emotion_after: Option<String>,
    confidence: Option<i16>,
    fear: Option<i16>,
    fomo: Option<i16>,
    frustration: Option<i16>,
    revenge_intensity: Option<i16>,
    boredom: Option<i16>,
}

fn auth_error(error: ApiAuthError) -> Response {
    error.into_response()
}

/// Legacy Journal imports stored maximum adverse excursion (MAE) as a
/// positive magnitude, while the analytics domain uses signed R values where
/// adverse movement is non-positive. Normalize at the read boundary so old
/// records remain intact and new excursion writes retain the canonical sign.
fn canonical_mae_r(value: Option<Decimal>) -> Option<Decimal> {
    value.map(|value| if value > Decimal::ZERO { -value } else { value })
}

pub async fn summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AnalyticsQuery>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(c) => c,
        Err(e) => return auth_error(e),
    };
    if query.from.zip(query.to).is_some_and(|(from, to)| from > to) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error":{"code":"invalid_date_range","message":"from must be earlier than or equal to to"}
        }))).into_response();
    }

    // Do not calculate monetary totals across unlike P&L currencies. Account
    // ownership remains enforced by the owner_id predicate in every query.
    let currency_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(DISTINCT COALESCE(t.pnl_currency, 'USD'))
        FROM journal_trades t
        WHERE t.owner_id=$1 AND t.status='closed' AND t.closed_at IS NOT NULL
          AND (t.net_pnl IS NOT NULL OR t.gross_pnl IS NOT NULL)
          AND ($2::TEXT IS NULL OR t.journal_account_id=$2)
          AND ($3::TIMESTAMPTZ IS NULL OR t.closed_at >= $3)
          AND ($4::TIMESTAMPTZ IS NULL OR t.closed_at <= $4)
          AND ($5::TEXT IS NULL OR t.symbol=$5)
          AND ($6::TEXT IS NULL OR t.setup_id=$6)
        "#,
    )
    .bind(&claims.sub)
    .bind(query.journal_account_id.as_deref())
    .bind(query.from)
    .bind(query.to)
    .bind(query.symbol.as_deref())
    .bind(query.setup_id.as_deref())
    .fetch_one(state.store.pool())
    .await;
    match currency_count {
        Ok(count) if count > 1 => return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "error":{"code":"mixed_currency_analytics","message":"Select a single account or currency before calculating monetary analytics."}
            })),
        ).into_response(),
        Err(_) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":{"code":"internal_error","message":"Unable to validate analytics currency"}})),
        ).into_response(),
        _ => {}
    }

    let facts = sqlx::query_as::<_, TradeFact>(
        r#"
        SELECT t.id, t.symbol, t.direction, t.opened_at, t.closed_at,
          COALESCE(t.net_pnl, t.gross_pnl + t.commission + t.swap + t.other_fees) AS net_pnl,
          t.risk_amount, t.entry_price, t.exit_price, t.stop_loss, t.take_profit,
          t.r_multiple, t.session, COALESCE(s.name, 'Unassigned') AS setup_name,
          t.mfe_r, t.mae_r, t.mfe_source, t.mae_source, t.mfe_precision, t.mae_precision,
          t.setup_valid, t.rules_followed, t.emotional_trade, t.mistake, t.discipline_score,
          p.emotion_before, p.emotion_during, p.emotion_after, p.confidence, p.fear, p.fomo,
          p.frustration, p.revenge_intensity, p.boredom
        FROM journal_trades t
        LEFT JOIN journal_setups s ON s.owner_id=t.owner_id AND s.id=t.setup_id
        LEFT JOIN journal_trade_psychology p ON p.owner_id=t.owner_id AND p.trade_id=t.id
        WHERE t.owner_id=$1 AND t.status='closed' AND t.closed_at IS NOT NULL
          AND (t.net_pnl IS NOT NULL OR t.gross_pnl IS NOT NULL)
          AND ($2::TEXT IS NULL OR t.journal_account_id=$2)
          AND ($3::TIMESTAMPTZ IS NULL OR t.closed_at >= $3)
          AND ($4::TIMESTAMPTZ IS NULL OR t.closed_at <= $4)
          AND ($5::TEXT IS NULL OR t.symbol=$5)
          AND ($6::TEXT IS NULL OR t.setup_id=$6)
        ORDER BY t.closed_at ASC, t.id ASC
    "#,
    )
    .bind(&claims.sub)
    .bind(query.journal_account_id.as_deref())
    .bind(query.from)
    .bind(query.to)
    .bind(query.symbol.as_deref())
    .bind(query.setup_id.as_deref())
    .fetch_all(state.store.pool())
    .await;

    let facts = match facts {
        Ok(rows) => rows,
        Err(_) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error":{"code":"internal_error","message":"Unable to calculate Journal analytics"}
            })),
        )
            .into_response(),
    };
    if facts.is_empty() {
        return Json(serde_json::json!({"data":{
            "generated_at":Utc::now(),
            "trade_count":0,"wins":0,"losses":0,"breakeven":0,
            "mfe_coverage_count":0,"mae_coverage_count":0,
            "win_rate_percent":"0","gross_profit":"0","gross_loss":"0",
            "net_profit":"0","profit_factor":null,"expectancy":"0",
            "average_win":null,"average_loss":null,"max_drawdown":"0",
            "average_r":null,"risk_coverage_count":0,"risk_coverage_percent":"0",
            "average_risk_amount":null,"maximum_risk_amount":null,
            "risk_distribution":{"minimum":null,"q1":null,"median":null,"q3":null,"maximum":null,"sample_count":0},
            "average_holding_seconds":null,"median_holding_seconds":null,
            "maximum_win_streak":0,"maximum_loss_streak":0,"recovery_factor":null,
            "average_mfe_capture_percent":null,"execution_coverage_count":0,"trade_metrics":[],
            "equity_curve":[],"by_symbol":[],"by_session":[],"by_setup":[],"by_weekday":[],"by_hour_utc":[],"by_day":[],"by_month":[],
            "notice":"No closed trades with realized P&L match the selected filters."
        }}))
        .into_response();
    }

    let pnls = facts.iter().map(|f| f.net_pnl).collect::<Vec<_>>();
    let stats = match summarize_net_pnl(&pnls) {
        Ok(value) => value,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error":{"code":"invalid_analytics_input","message":"No usable P&L values"}
                })),
            )
                .into_response()
        }
    };
    let mut equity = Decimal::ZERO;
    let mut peak = Decimal::ZERO;
    let mut equity_curve = Vec::with_capacity(facts.len());
    for fact in &facts {
        equity += fact.net_pnl;
        peak = peak.max(equity);
        equity_curve.push(serde_json::json!({
            "trade_id":fact.id,"closed_at":fact.closed_at,
            "net_pnl":fact.net_pnl.to_string(),"equity":equity.to_string(),
            "drawdown":(peak-equity).to_string()
        }));
    }

    let mut symbols: std::collections::BTreeMap<String, Vec<Decimal>> =
        std::collections::BTreeMap::new();
    let mut sessions: std::collections::BTreeMap<String, Vec<Decimal>> =
        std::collections::BTreeMap::new();
    let mut setups: std::collections::BTreeMap<String, Vec<Decimal>> =
        std::collections::BTreeMap::new();
    let mut weekdays: std::collections::BTreeMap<String, Vec<Decimal>> =
        std::collections::BTreeMap::new();
    let mut hours_utc: std::collections::BTreeMap<String, Vec<Decimal>> =
        std::collections::BTreeMap::new();
    let mut days: std::collections::BTreeMap<String, Vec<Decimal>> =
        std::collections::BTreeMap::new();
    let mut months: std::collections::BTreeMap<String, Vec<Decimal>> =
        std::collections::BTreeMap::new();
    let mut total_r = Decimal::ZERO;
    let mut r_count = 0usize;
    let mut risk_total = Decimal::ZERO;
    let mut risk_count = 0usize;
    let mut risk_max = Decimal::ZERO;
    let mut risk_values: Vec<Decimal> = Vec::new();
    let mut holding_seconds: Vec<i64> = Vec::with_capacity(facts.len());
    let mut current_win_streak = 0usize;
    let mut current_loss_streak = 0usize;
    let mut max_win_streak = 0usize;
    let mut max_loss_streak = 0usize;
    let mut trade_metrics = Vec::with_capacity(facts.len());
    let mut mfe_capture_values: Vec<Decimal> = Vec::new();
    let mut total_mfe_r = Decimal::ZERO;
    let mut mfe_count = 0usize;
    let mut total_mae_r = Decimal::ZERO;
    let mut mae_count = 0usize;
    let mut setup_valid_count = 0usize;
    let mut setup_valid_known = 0usize;
    let mut rules_followed_count = 0usize;
    let mut rules_followed_known = 0usize;
    let mut emotional_trade_count = 0usize;
    let mut mistake_count = 0usize;
    let mut total_discipline = 0i64;
    let mut discipline_count = 0usize;
    for fact in &facts {
        symbols
            .entry(fact.symbol.clone())
            .or_default()
            .push(fact.net_pnl);
        sessions
            .entry(fact.session.clone().unwrap_or_else(|| "unspecified".into()))
            .or_default()
            .push(fact.net_pnl);
        setups
            .entry(fact.setup_name.clone())
            .or_default()
            .push(fact.net_pnl);
        weekdays
            .entry(fact.closed_at.format("%A").to_string())
            .or_default()
            .push(fact.net_pnl);
        hours_utc
            .entry(fact.closed_at.format("%H:00 UTC").to_string())
            .or_default()
            .push(fact.net_pnl);
        days.entry(fact.closed_at.format("%Y-%m-%d").to_string())
            .or_default()
            .push(fact.net_pnl);
        months
            .entry(fact.closed_at.format("%Y-%m").to_string())
            .or_default()
            .push(fact.net_pnl);
        let analytics_fact = TradeAnalyticsFact {
            trade_id: fact.id.clone(),
            symbol: fact.symbol.clone(),
            direction: fact.direction.clone(),
            setup: fact.setup_name.clone(),
            session: fact.session.clone(),
            opened_at: fact.opened_at,
            closed_at: fact.closed_at,
            net_pnl: fact.net_pnl,
            entry_price: fact.entry_price,
            exit_price: fact.exit_price,
            stop_loss: fact.stop_loss,
            take_profit: fact.take_profit,
            risk_amount: fact.risk_amount,
            stored_r_multiple: fact.r_multiple,
            mfe_r: fact.mfe_r,
            mae_r: canonical_mae_r(fact.mae_r),
            mfe_source: fact.mfe_source.clone(),
            mae_source: fact.mae_source.clone(),
            mfe_precision: fact.mfe_precision.clone(),
            mae_precision: fact.mae_precision.clone(),
            setup_valid: fact.setup_valid,
            rules_followed: fact.rules_followed,
            emotional_trade: fact.emotional_trade,
            mistake: fact.mistake,
            discipline_score: fact.discipline_score,
            psychology: TradePsychologyFact {
                emotion_before: fact.emotion_before.clone(),
                emotion_during: fact.emotion_during.clone(),
                emotion_after: fact.emotion_after.clone(),
                confidence: fact.confidence,
                fear: fact.fear,
                fomo: fact.fomo,
                frustration: fact.frustration,
                revenge_intensity: fact.revenge_intensity,
                boredom: fact.boredom,
            },
        };
        if analytics_fact.validate().is_err() {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error":{"code":"invalid_trade_analytics_fact","message":"A stored Journal record failed analytics validation."}
                })),
            ).into_response();
        }
        let realized_r = analytics_fact.realized_r();
        if let Some(r) = realized_r {
            total_r += r;
            r_count += 1;
        }
        if let Some(risk) = fact.risk_amount.filter(|value| *value > Decimal::ZERO) {
            risk_total += risk;
            risk_count += 1;
            risk_max = risk_max.max(risk);
            risk_values.push(risk);
        }
        let held = analytics_fact.holding_seconds();
        holding_seconds.push(held);
        if fact.net_pnl > Decimal::ZERO {
            current_win_streak += 1;
            current_loss_streak = 0;
            max_win_streak = max_win_streak.max(current_win_streak);
        } else if fact.net_pnl < Decimal::ZERO {
            current_loss_streak += 1;
            current_win_streak = 0;
            max_loss_streak = max_loss_streak.max(current_loss_streak);
        } else {
            current_win_streak = 0;
            current_loss_streak = 0;
        }
        let capture = analytics_fact.mfe_capture_percent();
        if let Some(value) = capture {
            mfe_capture_values.push(value);
        }
        trade_metrics.push(serde_json::json!({
            "trade_id": fact.id.clone(),
            "symbol": fact.symbol.clone(),
            "session": fact.session.clone().unwrap_or_else(|| "unspecified".into()),
            "setup": fact.setup_name.clone(),
            "setup_valid": fact.setup_valid,
            "rules_followed": fact.rules_followed,
            "emotional_trade": fact.emotional_trade,
            "mistake": fact.mistake,
            "discipline_score": fact.discipline_score,
            "psychology": {
                "emotion_before": fact.emotion_before.clone(),
                "emotion_during": fact.emotion_during.clone(),
                "emotion_after": fact.emotion_after.clone(),
                "confidence": fact.confidence,
                "fear": fact.fear,
                "fomo": fact.fomo,
                "frustration": fact.frustration,
                "revenge_intensity": fact.revenge_intensity,
                "boredom": fact.boredom
            },
            "direction": fact.direction.clone(),
            "opened_at": fact.opened_at,
            "closed_at": fact.closed_at,
            "net_pnl": fact.net_pnl.to_string(),
            "risk_amount": fact.risk_amount.map(|v|v.to_string()),
            "realized_r": realized_r.map(|v|v.to_string()),
            "mfe_r": fact.mfe_r.map(|v|v.to_string()),
            "mae_r": fact.mae_r.map(|v|v.to_string()),
            "mfe_source": fact.mfe_source.clone(),
            "mae_source": fact.mae_source.clone(),
            "mfe_precision": fact.mfe_precision.clone(),
            "mae_precision": fact.mae_precision.clone(),
            "holding_seconds": held,
            "mfe_capture_percent": capture.map(|v|v.to_string()),
            "profit_surrendered_r": analytics_fact.profit_surrendered_r().map(|v|v.to_string()),
            "stop_loss_recorded": fact.stop_loss.is_some(),
            "take_profit_recorded": fact.take_profit.is_some(),
            "entry_price": fact.entry_price.to_string(),
            "exit_price": fact.exit_price.map(|v|v.to_string())
        }));
        if let Some(r) = fact.mfe_r {
            total_mfe_r += r;
            mfe_count += 1;
        }
        if let Some(r) = fact.mae_r {
            total_mae_r += r;
            mae_count += 1;
        }
        if let Some(valid) = fact.setup_valid {
            setup_valid_known += 1;
            if valid {
                setup_valid_count += 1;
            }
        }
        if let Some(followed) = fact.rules_followed {
            rules_followed_known += 1;
            if followed {
                rules_followed_count += 1;
            }
        }
        if fact.emotional_trade == Some(true) {
            emotional_trade_count += 1;
        }
        if fact.mistake == Some(true) {
            mistake_count += 1;
        }
        if let Some(score) = fact.discipline_score {
            total_discipline += i64::from(score);
            discipline_count += 1;
        }
    }
    holding_seconds.sort_unstable();
    let average_holding_seconds = if holding_seconds.is_empty() {
        None
    } else {
        Some(holding_seconds.iter().sum::<i64>() as f64 / holding_seconds.len() as f64)
    };
    let median_holding_seconds = if holding_seconds.is_empty() {
        None
    } else {
        let mid = holding_seconds.len() / 2;
        Some(if holding_seconds.len().is_multiple_of(2) {
            (holding_seconds[mid - 1] as f64 + holding_seconds[mid] as f64) / 2.0
        } else {
            holding_seconds[mid] as f64
        })
    };
    let average_mfe_capture_percent = if mfe_capture_values.is_empty() {
        None
    } else {
        Some(
            mfe_capture_values.iter().copied().sum::<Decimal>()
                / Decimal::from(mfe_capture_values.len() as u64),
        )
    };
    let risk_coverage_percent = Decimal::from(risk_count as u64) * Decimal::from(100)
        / Decimal::from(stats.trade_count as u64);
    risk_values.sort();
    let risk_quantile = |numerator: usize| -> Option<Decimal> {
        if risk_values.is_empty() {
            None
        } else {
            let index = (risk_values.len() - 1) * numerator / 4;
            risk_values.get(index).copied()
        }
    };
    let group_stats = |groups: std::collections::BTreeMap<String, Vec<Decimal>>| {
        groups
            .into_iter()
            .map(|(key, values)| {
                let n = Decimal::from(values.len() as u64);
                let net = values.iter().copied().sum::<Decimal>();
                let wins = values.iter().filter(|v| **v > Decimal::ZERO).count();
                serde_json::json!({"key":key,"trades":values.len(),"net_pnl":net.to_string(),
                "win_rate_percent":(Decimal::from(wins as u64)*Decimal::from(100)/n).to_string(),
                "expectancy":(net/n).to_string()})
            })
            .collect::<Vec<_>>()
    };

    Json(serde_json::json!({"data":{
        "generated_at":Utc::now(),
        "trade_count":stats.trade_count,"wins":stats.wins,"losses":stats.losses,"breakeven":stats.breakeven,
        "mfe_coverage_count":mfe_count,"mae_coverage_count":mae_count,
        "win_rate_percent":stats.win_rate_percent.to_string(),
        "gross_profit":stats.gross_profit.to_string(),"gross_loss":stats.gross_loss.to_string(),
        "net_profit":stats.net_profit.to_string(),"profit_factor":stats.profit_factor.map(|v|v.to_string()),
        "expectancy":stats.expectancy.to_string(),"average_win":stats.average_win.map(|v|v.to_string()),
        "average_loss":stats.average_loss.map(|v|v.to_string()),"max_drawdown":stats.max_drawdown.to_string(),
        "average_r":if r_count>0 {Some((total_r/Decimal::from(r_count as u64)).to_string())} else {None},
        "average_mfe_r":if mfe_count>0 {Some((total_mfe_r/Decimal::from(mfe_count as u64)).to_string())} else {None},
        "average_mae_r":if mae_count>0 {Some((total_mae_r/Decimal::from(mae_count as u64)).to_string())} else {None},
        "setup_valid_count":setup_valid_count,"rules_followed_count":rules_followed_count,"emotional_trade_count":emotional_trade_count,"mistake_count":mistake_count,
        "setup_valid_percent":if setup_valid_known>0 {Some((Decimal::from(setup_valid_count as u64)*Decimal::from(100)/Decimal::from(setup_valid_known as u64)).to_string())} else {None},
        "rules_followed_percent":if rules_followed_known>0 {Some((Decimal::from(rules_followed_count as u64)*Decimal::from(100)/Decimal::from(rules_followed_known as u64)).to_string())} else {None},
        "average_discipline_score":if discipline_count>0 {Some((Decimal::from(total_discipline)/Decimal::from(discipline_count as u64)).to_string())} else {None},
        "risk_coverage_count":risk_count,
        "risk_coverage_percent":risk_coverage_percent.to_string(),
        "average_risk_amount":if risk_count>0 {Some((risk_total/Decimal::from(risk_count as u64)).to_string())} else {None},
        "maximum_risk_amount":if risk_count>0 {Some(risk_max.to_string())} else {None},
        "risk_distribution": {
            "minimum":risk_values.first().map(|v|v.to_string()),
            "q1":risk_quantile(1).map(|v|v.to_string()),
            "median":risk_quantile(2).map(|v|v.to_string()),
            "q3":risk_quantile(3).map(|v|v.to_string()),
            "maximum":risk_values.last().map(|v|v.to_string()),
            "sample_count":risk_values.len()
        },
        "average_holding_seconds":average_holding_seconds,
        "median_holding_seconds":median_holding_seconds,
        "maximum_win_streak":max_win_streak,
        "maximum_loss_streak":max_loss_streak,
        "recovery_factor":if stats.max_drawdown > Decimal::ZERO {Some((stats.net_profit/stats.max_drawdown).to_string())} else {None},
        "average_mfe_capture_percent":average_mfe_capture_percent.map(|v|v.to_string()),
        "execution_coverage_count":mfe_capture_values.len(),
        "trade_metrics":trade_metrics,
        "equity_curve":equity_curve,"by_symbol":group_stats(symbols),"by_session":group_stats(sessions),"by_setup":group_stats(setups),"by_weekday":group_stats(weekdays),"by_hour_utc":group_stats(hours_utc),"by_day":group_stats(days),"by_month":group_stats(months),
        "notice":"Descriptive historical analytics; not a prediction or trade recommendation."
    }})).into_response()
}

#[cfg(test)]
mod mae_normalization_tests {
    use super::canonical_mae_r;
    use rust_decimal::Decimal;

    #[test]
    fn normalizes_legacy_positive_mae_magnitudes() {
        assert_eq!(
            canonical_mae_r(Some(Decimal::new(213, 2))),
            Some(Decimal::new(-213, 2))
        );
    }

    #[test]
    fn preserves_canonical_negative_zero_and_missing_mae() {
        assert_eq!(
            canonical_mae_r(Some(Decimal::new(-5, 1))),
            Some(Decimal::new(-5, 1))
        );
        assert_eq!(canonical_mae_r(Some(Decimal::ZERO)), Some(Decimal::ZERO));
        assert_eq!(canonical_mae_r(None), None);
    }
}
