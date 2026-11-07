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
    direction: String,
    closed_at: DateTime<Utc>,
    net_pnl: Decimal,
    risk_amount: Option<Decimal>,
    r_multiple: Option<Decimal>,
    session: Option<String>,
}

fn auth_error(error: ApiAuthError) -> Response { error.into_response() }

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

    let facts = sqlx::query_as::<_, TradeFact>(r#"
        SELECT id, symbol, direction, closed_at,
          COALESCE(net_pnl, gross_pnl + commission + swap + other_fees) AS net_pnl,
          risk_amount, r_multiple, session
        FROM journal_trades
        WHERE owner_id=$1 AND status='closed' AND closed_at IS NOT NULL
          AND (net_pnl IS NOT NULL OR gross_pnl IS NOT NULL)
          AND ($2::TEXT IS NULL OR journal_account_id=$2)
          AND ($3::TIMESTAMPTZ IS NULL OR closed_at >= $3)
          AND ($4::TIMESTAMPTZ IS NULL OR closed_at <= $4)
        ORDER BY closed_at ASC, id ASC
    "#)
    .bind(&claims.sub)
    .bind(query.journal_account_id.as_deref())
    .bind(query.from)
    .bind(query.to)
    .fetch_all(state.store.pool())
    .await;

    let facts = match facts {
        Ok(rows) => rows,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error":{"code":"internal_error","message":"Unable to calculate Journal analytics"}
        }))).into_response(),
    };
    if facts.is_empty() {
        return Json(serde_json::json!({"data":{
            "trade_count":0,"wins":0,"losses":0,"breakeven":0,
            "win_rate_percent":"0","gross_profit":"0","gross_loss":"0",
            "net_profit":"0","profit_factor":null,"expectancy":"0",
            "average_win":null,"average_loss":null,"max_drawdown":"0",
            "average_r":null,"equity_curve":[],"by_symbol":[],"by_session":[],
            "notice":"No closed trades with realized P&L match the selected filters."
        }})).into_response();
    }

    let pnls = facts.iter().map(|f| f.net_pnl).collect::<Vec<_>>();
    let stats = match summarize_net_pnl(&pnls) {
        Ok(value) => value,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error":{"code":"invalid_analytics_input","message":"No usable P&L values"}
        }))).into_response(),
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

    let mut symbols: std::collections::BTreeMap<String, Vec<Decimal>> = std::collections::BTreeMap::new();
    let mut sessions: std::collections::BTreeMap<String, Vec<Decimal>> = std::collections::BTreeMap::new();
    let mut total_r = Decimal::ZERO;
    let mut r_count = 0usize;
    for fact in &facts {
        symbols.entry(fact.symbol.clone()).or_default().push(fact.net_pnl);
        sessions.entry(fact.session.clone().unwrap_or_else(||"unspecified".into())).or_default().push(fact.net_pnl);
        if let Some(r) = fact.r_multiple { total_r += r; r_count += 1; }
    }
    let group_stats = |groups: std::collections::BTreeMap<String, Vec<Decimal>>| {
        groups.into_iter().map(|(key, values)| {
            let n=Decimal::from(values.len() as u64);
            let net=values.iter().copied().sum::<Decimal>();
            let wins=values.iter().filter(|v| **v>Decimal::ZERO).count();
            serde_json::json!({"key":key,"trades":values.len(),"net_pnl":net.to_string(),
                "win_rate_percent":(Decimal::from(wins as u64)*Decimal::from(100)/n).to_string(),
                "expectancy":(net/n).to_string()})
        }).collect::<Vec<_>>()
    };

    Json(serde_json::json!({"data":{
        "trade_count":stats.trade_count,"wins":stats.wins,"losses":stats.losses,"breakeven":stats.breakeven,
        "win_rate_percent":stats.win_rate_percent.to_string(),
        "gross_profit":stats.gross_profit.to_string(),"gross_loss":stats.gross_loss.to_string(),
        "net_profit":stats.net_profit.to_string(),"profit_factor":stats.profit_factor.map(|v|v.to_string()),
        "expectancy":stats.expectancy.to_string(),"average_win":stats.average_win.map(|v|v.to_string()),
        "average_loss":stats.average_loss.map(|v|v.to_string()),"max_drawdown":stats.max_drawdown.to_string(),
        "average_r":if r_count>0 {Some((total_r/Decimal::from(r_count as u64)).to_string())} else {None},
        "equity_curve":equity_curve,"by_symbol":group_stats(symbols),"by_session":group_stats(sessions),
        "notice":"Descriptive historical analytics; not a prediction or trade recommendation."
    }})).into_response()
}
