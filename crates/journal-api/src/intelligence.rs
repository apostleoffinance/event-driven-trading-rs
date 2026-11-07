//! Owner-scoped journal process intelligence and statistical sample warnings.
use axum::{extract::{Query, State}, http::{HeaderMap, StatusCode}, response::{IntoResponse, Response}, Json};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::FromRow;

use crate::{auth::ApiAuthError, AppState};

#[derive(Debug, Deserialize)]
pub struct IntelligenceQuery {
    pub journal_account_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct Fact {
    net_pnl: Decimal,
    rules_followed: Option<bool>,
    setup_valid: Option<bool>,
    emotional_trade: Option<bool>,
    mistake: Option<bool>,
    discipline_score: Option<i16>,
}

fn wilson_interval(wins: usize, n: usize) -> Option<(f64, f64)> {
    if n == 0 { return None; }
    let z = 1.959_963_984_540_054_f64;
    let p = wins as f64 / n as f64;
    let nf = n as f64;
    let denom = 1.0 + z * z / nf;
    let center = (p + z * z / (2.0 * nf)) / denom;
    let margin = z * ((p * (1.0 - p) / nf + z * z / (4.0 * nf * nf)).sqrt()) / denom;
    Some(((center - margin).max(0.0), (center + margin).min(1.0)))
}

pub async fn summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<IntelligenceQuery>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(c) => c,
        Err(e) => return ApiAuthError::into_response(e),
    };
    if query.from.zip(query.to).is_some_and(|(from, to)| from > to) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error":{"code":"invalid_date_range","message":"from must be earlier than or equal to to"}
        }))).into_response();
    }

    let rows = sqlx::query_as::<_, Fact>(r#"
        SELECT COALESCE(t.net_pnl, t.gross_pnl + t.commission + t.swap + t.other_fees) AS net_pnl,
          t.rules_followed, t.setup_valid, t.emotional_trade, t.mistake, t.discipline_score
        FROM journal_trades t
        WHERE t.owner_id=$1 AND t.status='closed' AND t.closed_at IS NOT NULL
          AND (t.net_pnl IS NOT NULL OR t.gross_pnl IS NOT NULL)
          AND ($2::TEXT IS NULL OR t.journal_account_id=$2)
          AND ($3::TIMESTAMPTZ IS NULL OR t.closed_at >= $3)
          AND ($4::TIMESTAMPTZ IS NULL OR t.closed_at <= $4)
        ORDER BY t.closed_at ASC, t.id ASC
    "#)
    .bind(&claims.sub).bind(query.journal_account_id.as_deref()).bind(query.from).bind(query.to)
    .fetch_all(state.store.pool()).await;

    let rows = match rows {
        Ok(rows) => rows,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error":{"code":"internal_error","message":"Unable to calculate Journal intelligence"}
        }))).into_response(),
    };

    let n = rows.len();
    let wins = rows.iter().filter(|r| r.net_pnl > Decimal::ZERO).count();
    let losses = rows.iter().filter(|r| r.net_pnl < Decimal::ZERO).count();
    let breakeven = n.saturating_sub(wins + losses);
    let known_rules = rows.iter().filter(|r| r.rules_followed.is_some()).count();
    let rules_ok = rows.iter().filter(|r| r.rules_followed == Some(true)).count();
    let known_valid = rows.iter().filter(|r| r.setup_valid.is_some()).count();
    let valid = rows.iter().filter(|r| r.setup_valid == Some(true)).count();
    let emotional = rows.iter().filter(|r| r.emotional_trade == Some(true)).count();
    let mistakes = rows.iter().filter(|r| r.mistake == Some(true)).count();
    let discipline: Vec<i16> = rows.iter().filter_map(|r| r.discipline_score).collect();

    let good_process_wins = rows.iter().filter(|r| r.rules_followed == Some(true) && r.net_pnl > Decimal::ZERO).count();
    let good_process_losses = rows.iter().filter(|r| r.rules_followed == Some(true) && r.net_pnl < Decimal::ZERO).count();
    let poor_process_wins = rows.iter().filter(|r| r.rules_followed == Some(false) && r.net_pnl > Decimal::ZERO).count();
    let poor_process_losses = rows.iter().filter(|r| r.rules_followed == Some(false) && r.net_pnl < Decimal::ZERO).count();

    let confidence = wilson_interval(wins, n);
    let sample_status = if n < 30 { "insufficient_sample" } else if n < 100 { "early_sample" } else { "more_stable_sample" };
    let mut insights: Vec<serde_json::Value> = Vec::new();
    if n < 30 {
        insights.push(serde_json::json!({"type":"sample_size","severity":"info","title":"Small sample",
            "message":format!("{} closed trades recorded. Treat win-rate and setup conclusions as provisional until at least 30 trades are available.",n)}));
    }
    if known_rules >= 5 && rules_ok * 100 < known_rules * 70 {
        insights.push(serde_json::json!({"type":"discipline","severity":"warning","title":"Rule adherence is below 70%",
            "message":"Review rule violations separately from P&L. Winning trades that violate rules remain poor-process outcomes."}));
    }
    if n >= 5 && emotional * 100 >= n * 30 {
        insights.push(serde_json::json!({"type":"behavior","severity":"warning","title":"Emotional trades are frequent",
            "message":"At least 30% of recorded trades were marked emotional. Review the notes and context; this is a journal pattern, not a diagnosis."}));
    }
    if n >= 5 && mistakes * 100 >= n * 25 {
        insights.push(serde_json::json!({"type":"behavior","severity":"warning","title":"Mistakes are recurring",
            "message":"At least 25% of trades were marked as mistakes. Categorize recurring errors and review the playbook."}));
    }
    if poor_process_wins > 0 {
        insights.push(serde_json::json!({"type":"process_outcome","severity":"warning","title":"Profitable trades with poor process",
            "message":format!("{} winning trades did not follow rules. Profit does not make these trades process-quality wins.",poor_process_wins)}));
    }
    if good_process_losses > 0 {
        insights.push(serde_json::json!({"type":"process_outcome","severity":"info","title":"Good process, losing outcome",
            "message":format!("{} losses followed the rules. Evaluate process separately from the outcome.",good_process_losses)}));
    }
    if insights.is_empty() {
        insights.push(serde_json::json!({"type":"neutral","severity":"info","title":"No strong pattern detected",
            "message":"Keep recording process and psychology data. Insights become more meaningful with consistent journal entries."}));
    }

    Json(serde_json::json!({"data":{
        "trade_count":n,"wins":wins,"losses":losses,"breakeven":breakeven,
        "win_rate_percent":if n>0 {Some((wins as f64*100.0/n as f64).to_string())} else {None},
        "win_rate_confidence_95":confidence.map(|(low,high)|serde_json::json!({"lower_percent":low*100.0,"upper_percent":high*100.0})),
        "sample_status":sample_status,"minimum_sample_recommendation":30,
        "rule_adherence_percent":if known_rules>0 {Some(rules_ok as f64*100.0/known_rules as f64)} else {None},
        "rule_adherence_known_count":known_rules,"valid_setup_percent":if known_valid>0 {Some(valid as f64*100.0/known_valid as f64)} else {None},
        "valid_setup_known_count":known_valid,"emotional_trade_count":emotional,"mistake_count":mistakes,
        "average_discipline_score":if !discipline.is_empty(){Some(discipline.iter().map(|v|i64::from(*v)).sum::<i64>() as f64/discipline.len() as f64)}else{None},
        "process_outcomes":{"good_process_win":good_process_wins,"good_process_loss":good_process_losses,"poor_process_win":poor_process_wins,"poor_process_loss":poor_process_losses},
        "insights":insights,
        "notice":"Historical descriptive heuristics only. No diagnoses, intent inference, forecasts, or trade recommendations."
    }})).into_response()
}

#[cfg(test)]
mod tests {
    use super::wilson_interval;

    #[test]
    fn wilson_interval_is_bounded_and_contains_observed_rate() {
        let (low, high) = wilson_interval(7, 10).expect("non-empty sample");
        assert!(low >= 0.0 && high <= 1.0 && low <= 0.7 && high >= 0.7);
    }

    #[test]
    fn wilson_interval_is_none_for_empty_sample() {
        assert!(wilson_interval(0, 0).is_none());
    }
}
