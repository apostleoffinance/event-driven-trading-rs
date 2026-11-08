//! Authenticated read-only analytics over the caller's Journal facts.
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use domain::summarize_net_pnl;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::FromRow;

use crate::{auth::ApiAuthError, AppState};

#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    pub journal_account_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct TradeFact {
    id: String,
    symbol: String,
    closed_at: DateTime<Utc>,
    net_pnl: Decimal,
    r_multiple: Option<Decimal>,
    session: Option<String>,
    setup_name: String,
    mfe_r: Option<Decimal>,
    mae_r: Option<Decimal>,
    setup_valid: Option<bool>,
    rules_followed: Option<bool>,
    emotional_trade: Option<bool>,
    mistake: Option<bool>,
    discipline_score: Option<i16>,
}

fn auth_error(error: ApiAuthError) -> Response {
    error.into_response()
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
        "#,
    )
    .bind(&claims.sub)
    .bind(query.journal_account_id.as_deref())
    .bind(query.from)
    .bind(query.to)
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
        SELECT t.id, t.symbol, t.closed_at,
          COALESCE(t.net_pnl, t.gross_pnl + t.commission + t.swap + t.other_fees) AS net_pnl,
          t.r_multiple, t.session, COALESCE(s.name, 'Unassigned') AS setup_name, t.mfe_r, t.mae_r, t.setup_valid, t.rules_followed, t.emotional_trade, t.mistake, t.discipline_score
        FROM journal_trades t
        LEFT JOIN journal_setups s ON s.owner_id=t.owner_id AND s.id=t.setup_id
        WHERE t.owner_id=$1 AND t.status='closed' AND t.closed_at IS NOT NULL
          AND (t.net_pnl IS NOT NULL OR t.gross_pnl IS NOT NULL)
          AND ($2::TEXT IS NULL OR t.journal_account_id=$2)
          AND ($3::TIMESTAMPTZ IS NULL OR t.closed_at >= $3)
          AND ($4::TIMESTAMPTZ IS NULL OR t.closed_at <= $4)
        ORDER BY t.closed_at ASC, t.id ASC
    "#,
    )
    .bind(&claims.sub)
    .bind(query.journal_account_id.as_deref())
    .bind(query.from)
    .bind(query.to)
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
            "trade_count":0,"wins":0,"losses":0,"breakeven":0,
            "win_rate_percent":"0","gross_profit":"0","gross_loss":"0",
            "net_profit":"0","profit_factor":null,"expectancy":"0",
            "average_win":null,"average_loss":null,"max_drawdown":"0",
            "average_r":null,"equity_curve":[],"by_symbol":[],"by_session":[],"by_setup":[],"by_weekday":[],"by_hour_utc":[],"by_day":[],"by_month":[],
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
        if let Some(r) = fact.r_multiple {
            total_r += r;
            r_count += 1;
        }
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
        "trade_count":stats.trade_count,"wins":stats.wins,"losses":stats.losses,"breakeven":stats.breakeven,
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
        "equity_curve":equity_curve,"by_symbol":group_stats(symbols),"by_session":group_stats(sessions),"by_setup":group_stats(setups),"by_weekday":group_stats(weekdays),"by_hour_utc":group_stats(hours_utc),"by_day":group_stats(days),"by_month":group_stats(months),
        "notice":"Descriptive historical analytics; not a prediction or trade recommendation."
    }})).into_response()
}
