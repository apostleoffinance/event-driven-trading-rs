//! Authenticated, read-only trading dashboard queries.
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::FromRow;

use crate::{auth::ApiAuthError, AppState};
use domain::{Account, AccountId, AccountState, AccountType, Venue, VenueId, VenueType};

fn auth(state: &AppState, headers: &HeaderMap) -> Result<String, ApiAuthError> {
    state.auth.authenticate(headers).map(|claims| claims.sub)
}

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"error":{"code":code,"message":message}})),
    )
        .into_response()
}
fn internal() -> Response {
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "Unable to read trading dashboard data",
    )
}
fn not_found() -> Response {
    error(
        StatusCode::NOT_FOUND,
        "not_found",
        "Trading dashboard resource not found",
    )
}
fn dec(v: Decimal) -> String {
    v.to_string()
}
fn status_case(v: &str) -> String {
    v.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<String>()
}

#[derive(Debug, FromRow)]
struct AccountRow {
    id: String,
    account_type: String,
    environment: String,
    status: String,
    venue_id: String,
    venue_name: String,
    venue_enabled: bool,
    starting_capital: Decimal,
    label: Option<String>,
    balance: Decimal,
    equity: Decimal,
    used_margin: Decimal,
    open_position_count: i32,
    daily_realized_pnl: Decimal,
    peak_equity: Decimal,
    kill_switch: bool,
    updated_at: DateTime<Utc>,
}

async fn account_rows(
    state: &AppState,
    owner: &str,
    account_id: Option<&str>,
) -> Result<Vec<AccountRow>, sqlx::Error> {
    sqlx::query_as::<_,AccountRow>(r#"
        SELECT a.id,a.account_type,a.environment,a.status,a.venue_id,v.name AS venue_name,v.enabled AS venue_enabled,
               a.starting_capital,a.label,s.balance,s.equity,s.used_margin,s.open_position_count,
               s.daily_realized_pnl,s.peak_equity,s.kill_switch,s.updated_at
        FROM dashboard_account_access g
        JOIN accounts a ON a.id=g.account_id
        JOIN venues v ON v.id=a.venue_id
        JOIN account_states s ON s.account_id=a.id
        WHERE g.owner_id=$1 AND ($2::TEXT IS NULL OR a.id=$2)
        ORDER BY a.created_at DESC
    "#).bind(owner).bind(account_id).fetch_all(state.store.pool()).await
}
fn account_json(a: AccountRow) -> serde_json::Value {
    let drawdown = if a.peak_equity > Decimal::ZERO {
        ((a.peak_equity - a.equity).max(Decimal::ZERO) / a.peak_equity * Decimal::from(100))
            .to_f64()
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let risk_status = if a.kill_switch { "BREACH" } else { "WARNING" };
    let conn_status = if a.venue_enabled {
        "DEGRADED"
    } else {
        "DISCONNECTED"
    };
    let environment = match a.account_type.as_str() {
        "simulated" => "PAPER",
        _ => "SHADOW",
    };
    serde_json::json!({
        "id":a.id,"name":a.label.unwrap_or_else(||a.id.clone()),
        "type":status_case(&a.account_type),"venueId":a.venue_id,"venueName":a.venue_name,
        "environment":a.environment,"balance":dec(a.balance),"equity":dec(a.equity),
        "availableCapital":dec((a.equity-a.used_margin).max(Decimal::ZERO)),
        "dailyPnl":dec(a.daily_realized_pnl),"unrealizedPnl":dec(a.equity-a.balance),
        "realizedPnl":a.realized_pnl.map(dec),"drawdownPct":drawdown,
        "openPositionCount":a.open_position_count,"status":status_case(&a.status),
        "riskStatus":risk_status,"connectionStatus":conn_status,
        "lastUpdatedAt":a.updated_at
    })
}
use rust_decimal::prelude::ToPrimitive;

#[derive(Debug, Deserialize)]
pub struct PaperAccountWrite {
    pub name: String,
    pub initial_capital: Decimal,
}

pub async fn create_paper_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PaperAccountWrite>,
) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let name = input.name.trim();
    if name.is_empty() || name.len() > 80 || input.initial_capital <= Decimal::ZERO {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_account",
            "Name must be 1–80 characters and initial_capital must be positive.",
        );
    }

    let now = Utc::now();
    let account_id = match AccountId::new(uuid::Uuid::new_v4().to_string()) {
        Ok(id) => id,
        Err(_) => return internal(),
    };
    let venue_id = match VenueId::new("simulated") {
        Ok(id) => id,
        Err(_) => return internal(),
    };
    let account = Account::new(
        account_id.clone(),
        AccountType::Simulated,
        venue_id.clone(),
        now,
    );
    let account_state = match AccountState::new(account_id.clone(), input.initial_capital, now) {
        Ok(state) => state,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_capital",
                "initial_capital must be positive.",
            )
        }
    };
    let venue = Venue::new(venue_id, VenueType::Simulated, "Paper Simulator", now);

    if state.store.upsert_venue(&venue).await.is_err()
        || state
            .store
            .upsert_account(&account, input.initial_capital, Some(name), &account_state)
            .await
            .is_err()
    {
        return internal();
    }
    if sqlx::query(
        "INSERT INTO dashboard_account_access(owner_id,account_id,access_role,granted_by) VALUES($1,$2,'owner',$1)",
    )
    .bind(&owner)
    .bind(account_id.as_str())
    .execute(state.store.pool())
    .await
    .is_err()
    {
        return internal();
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({"data":{
            "id":account_id.as_str(),"name":name,"type":"Simulated","venueId":"simulated",
            "venueName":"Paper Simulator","environment":"PAPER","balance":dec(input.initial_capital),
            "equity":dec(input.initial_capital),"availableCapital":dec(input.initial_capital),
            "dailyPnl":"0","unrealizedPnl":"0","realizedPnl":"0","drawdownPct":0.0,
            "openPositionCount":0,"status":"Active","riskStatus":"WARNING","connectionStatus":"DEGRADED"
        }})),
    ).into_response()
}

pub async fn accounts(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match account_rows(&state, &owner, None).await {
        Ok(rows) => {
            Json(serde_json::json!({"data":rows.into_iter().map(account_json).collect::<Vec<_>>()}))
                .into_response()
        }
        Err(_) => internal(),
    }
}
pub async fn account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match account_rows(&state, &owner, Some(&id)).await {
        Ok(mut rows) => match rows.pop() {
            Some(row) => Json(serde_json::json!({"data":account_json(row)})).into_response(),
            None => not_found(),
        },
        Err(_) => internal(),
    }
}

pub async fn portfolio(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let totals=sqlx::query_as::<_,(Option<Decimal>,Option<Decimal>,Option<Decimal>,Option<Decimal>,Option<Decimal>,Option<Decimal>,Option<Decimal>,Option<Decimal>)>(r#"
      SELECT COALESCE(SUM(s.equity),0),COALESCE(SUM(s.balance),0),COALESCE(SUM(s.equity-s.used_margin),0),
             COALESCE(SUM(s.daily_realized_pnl),0),COALESCE(SUM(s.equity-s.balance),0),
             COALESCE(SUM(a.starting_capital),0),COALESCE(SUM(s.used_margin),0),
             COALESCE(MAX(CASE WHEN s.peak_equity>0 THEN (s.peak_equity-s.equity)/s.peak_equity*100 ELSE 0 END),0)
      FROM dashboard_account_access g JOIN accounts a ON a.id=g.account_id JOIN account_states s ON s.account_id=a.id
      WHERE g.owner_id=$1
    "#).bind(&owner).fetch_one(state.store.pool()).await;
    let (equity, balance, available, daily, unrealized, starting, margin, drawdown) = match totals {
        Ok((e, b, av, d, u, st, m, dd)) => (
            e.unwrap_or_default(),
            b.unwrap_or_default(),
            av.unwrap_or_default(),
            d.unwrap_or_default(),
            u.unwrap_or_default(),
            st.unwrap_or_default(),
            m.unwrap_or_default(),
            dd.unwrap_or_default(),
        ),
        Err(_) => return internal(),
    };
    let realized=sqlx::query_scalar::<_,Option<Decimal>>(r#"
      SELECT SUM(ps.realized_pnl) FROM performance_snapshots ps
      JOIN dashboard_account_access g ON g.account_id=ps.account_id
      WHERE g.owner_id=$1 AND ps.id=(SELECT MAX(p2.id) FROM performance_snapshots p2 WHERE p2.account_id=ps.account_id)
    "#).bind(&owner).fetch_one(state.store.pool()).await.unwrap_or(None).unwrap_or(daily);
    let series = sqlx::query_as::<_, (DateTime<Utc>, Decimal, Decimal, Decimal)>(
        r#"
      SELECT ps.captured_at,ps.equity,ps.realized_pnl,ps.drawdown_pct
      FROM performance_snapshots ps JOIN dashboard_account_access g ON g.account_id=ps.account_id
      WHERE g.owner_id=$1 ORDER BY ps.captured_at DESC LIMIT 100
    "#,
    )
    .bind(&owner)
    .fetch_all(state.store.pool())
    .await
    .unwrap_or_default();
    let equity_series=series.into_iter().rev().map(|(t,e,r,d)|serde_json::json!({"time":t,"equity":dec(e),"realizedPnl":dec(r),"drawdownPct":d.to_f64().unwrap_or(0.0)})).collect::<Vec<_>>();
    let gross=sqlx::query_scalar::<_,Option<Decimal>>(r#"
      SELECT SUM(ABS(p.quantity*p.mark_price)) FROM positions p JOIN dashboard_account_access g ON g.account_id=p.account_id WHERE g.owner_id=$1
    "#).bind(&owner).fetch_one(state.store.pool()).await.unwrap_or(None).unwrap_or_default();
    let cumret = if starting > Decimal::ZERO {
        ((equity - starting) / starting * Decimal::from(100))
            .to_f64()
            .unwrap_or(0.0)
    } else {
        0.0
    };
    Json(serde_json::json!({"data":{
      "totalEquity":dec(equity),"balance":dec(balance),"availableCapital":dec(available),
      "dailyPnl":dec(daily),"unrealizedPnl":dec(unrealized),"realizedPnl":dec(realized),
      "cumulativeReturnPct":cumret,"drawdownPct":drawdown.to_f64().unwrap_or(0.0),
      "maxDrawdownPct":drawdown.to_f64().unwrap_or(0.0),"grossExposure":dec(gross),
      "netExposure":dec(gross),"exposurePct":if equity>Decimal::ZERO{(gross/equity*Decimal::from(100)).to_f64().unwrap_or(0.0)}else{0.0},
      "equitySeries":equity_series,"pnlByStrategy":[],"pnlByInstrument":[]
    }})).into_response()
}

pub async fn positions(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let rows=sqlx::query_as::<_,(String,String,String,String,Decimal,Decimal,Decimal,Decimal,DateTime<Utc>)>(r#"
      SELECT p.id,p.account_id,i.symbol,p.side,p.quantity,p.avg_entry_price,p.mark_price,p.realized_pnl,p.opened_at
      FROM positions p JOIN instruments i ON i.id=p.instrument_id
      JOIN dashboard_account_access g ON g.account_id=p.account_id
      WHERE g.owner_id=$1 AND p.quantity>0 ORDER BY p.opened_at DESC
    "#).bind(&owner).fetch_all(state.store.pool()).await;
    match rows { Ok(rows)=>Json(serde_json::json!({"data":rows.into_iter().map(|(id,account,symbol,side,qty,entry,mark,realized,opened)|{
      let unrealized=if side=="long"{(mark-entry)*qty}else{(entry-mark)*qty};
      serde_json::json!({"id":id,"accountId":account,"strategyId":null,"instrumentId":symbol,"side":if side=="long"{"Long"}else{"Short"},"quantity":dec(qty),"entryPrice":dec(entry),"markPrice":dec(mark),"unrealizedPnl":dec(unrealized),"realizedPnl":dec(realized),"exposure":dec((mark*qty).abs()),"riskPct":null,"openedAt":opened})
    }).collect::<Vec<_>>()})).into_response(),Err(_)=>internal()}
}

#[derive(Debug, Deserialize)]
pub struct ListLimit {
    pub limit: Option<i64>,
}
pub async fn orders(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListLimit>,
) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let rows=sqlx::query_as::<_,(String,String,String,String,String,String,String,String,String,String,Decimal,Option<Decimal>,Decimal,String,DateTime<Utc>,Option<DateTime<Utc>>,DateTime<Utc>)>(r#"
      SELECT o.id,o.client_order_id,o.account_id,o.venue_id,o.strategy_id,COALESCE(o.trade_intent_id,''),i.symbol,o.side,o.order_type,o.time_in_force,o.quantity,o.price,o.filled_quantity,o.status,o.created_at,o.submitted_at,o.updated_at
      FROM orders o JOIN instruments i ON i.id=o.instrument_id JOIN dashboard_account_access g ON g.account_id=o.account_id
      WHERE g.owner_id=$1 ORDER BY o.created_at DESC LIMIT $2
    "#).bind(&owner).bind(limit).fetch_all(state.store.pool()).await;
    match rows {Ok(rows)=>Json(serde_json::json!({"data":rows.into_iter().map(|(id,client,account,venue,strategy,intent,symbol,side,kind,tif,qty,price,filled,status,created,submitted,updated)|serde_json::json!({
      "id":id,"clientOrderId":client,"accountId":account,"venueId":venue,"strategyId":strategy,"tradeIntentId":intent,"instrumentId":symbol,
      "side":status_case(&side),"orderType":status_case(&kind),"timeInForce":status_case(&tif),"quantity":dec(qty),"price":price.map(dec),"filledQuantity":dec(filled),
      "status":status_case(&status),"createdAt":created,"submittedAt":submitted,"updatedAt":updated
    })).collect::<Vec<_>>()})).into_response(),Err(_)=>internal()}
}

pub async fn strategies(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let rows=sqlx::query_as::<_,(String,String,String,String,bool,DateTime<Utc>,String)>(r#"
      SELECT d.id,s.name,d.strategy_version,d.account_id,d.enabled,d.created_at,a.account_type
      FROM strategy_deployments d JOIN strategies s ON s.id=d.strategy_id JOIN accounts a ON a.id=d.account_id
      JOIN dashboard_account_access g ON g.account_id=d.account_id
      WHERE g.owner_id=$1 ORDER BY d.created_at DESC
    "#).bind(&owner).fetch_all(state.store.pool()).await;
    match rows {Ok(rows)=>Json(serde_json::json!({"data":rows.into_iter().map(|(id,name,version,account,enabled,created,account_type)|serde_json::json!({
      "id":id,"name":name,"version":version,"status":if enabled{"PAPER"}else{"STOPPED"},"environment":if account_type=="simulated"{"PAPER"}else{"SHADOW"},
      "instrumentId":null,"signals":null,"trades":null,"returnPct":null,"sharpe":null,"maxDrawdownPct":null,"winRatePct":null,"profitFactor":null,
      "description":format!("Deployment for account {account}; strategy performance metrics are not yet persisted."),"configuration":[],"recentIntentIds":[],"createdAt":created
    })).collect::<Vec<_>>()})).into_response(),Err(_)=>internal()}
}

pub async fn risk(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let row=sqlx::query_as::<_,(Option<Decimal>,Option<Decimal>,Option<Decimal>,Option<bool>,i64,Option<Decimal>,Option<Decimal>)>(r#"
      WITH owned AS (
        SELECT g.account_id,s.daily_realized_pnl,s.balance,s.equity,s.peak_equity,s.kill_switch
        FROM dashboard_account_access g JOIN account_states s ON s.account_id=g.account_id WHERE g.owner_id=$1
      ), limits AS (
        SELECT MAX(NULLIF(rp.policy_json->>'max_daily_loss_pct','')::NUMERIC) AS daily_limit,
               MAX(NULLIF(rp.policy_json->>'max_drawdown_pct','')::NUMERIC) AS dd_limit
        FROM dashboard_account_access g
        JOIN strategy_deployments d ON d.account_id=g.account_id
        JOIN risk_profiles rp ON rp.id=d.risk_profile_id WHERE g.owner_id=$1
      )
      SELECT COALESCE(SUM(o.daily_realized_pnl),0),COALESCE(SUM(o.balance),0),
        COALESCE(MAX(CASE WHEN o.peak_equity>0 THEN (o.peak_equity-o.equity)/o.peak_equity*100 ELSE 0 END),0),
        BOOL_OR(o.kill_switch),COUNT(*),limits.daily_limit,limits.dd_limit
      FROM owned o CROSS JOIN limits GROUP BY limits.daily_limit,limits.dd_limit
    "#).bind(&owner).fetch_optional(state.store.pool()).await;
    let (daily, balance, dd, kill, count, daily_limit, dd_limit) = match row {
        Ok(Some(r)) => r,
        Ok(None) => (
            Some(Decimal::ZERO),
            Some(Decimal::ZERO),
            Some(Decimal::ZERO),
            Some(false),
            0,
            None,
            None,
        ),
        Err(_) => return internal(),
    };
    let daily = daily.unwrap_or_default();
    let balance = balance.unwrap_or_default();
    let dd = dd.unwrap_or_default();
    let kill = kill.unwrap_or(false);
    let daily_loss = if balance > Decimal::ZERO {
        (-daily).max(Decimal::ZERO) / balance * Decimal::from(100)
    } else {
        Decimal::ZERO
    };
    let engine_status = if count == 0 {
        "DISCONNECTED"
    } else {
        "DEGRADED"
    };
    Json(serde_json::json!({"data":{
      "engineStatus":engine_status,
      "dailyLossPct":daily_loss.to_f64().unwrap_or(0.0),
      "dailyLossLimitPct":daily_limit.and_then(|v|v.to_f64()),
      "maxDrawdownPct":dd.to_f64().unwrap_or(0.0),
      "maxDrawdownLimitPct":dd_limit.and_then(|v|v.to_f64()),
      "openRiskPct":null,"exposurePct":null,
      "killSwitch":kill,"tone":if kill{"BREACH"}else{"WARNING"},"hierarchy":[]
    }}))
    .into_response()
}

pub async fn risk_decisions(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let rows=sqlx::query_as::<_,(i64,DateTime<Utc>,String,String,String,String,Option<Decimal>,Option<Decimal>,String,Option<String>)>(r#"
      SELECT r.id,r.created_at,r.trade_intent_id,ti.strategy_id,ti.instrument_id,r.account_id,ti.target_quantity,r.quantity,r.decision_type,r.reason
      FROM risk_decisions r JOIN trade_intents ti ON ti.id=r.trade_intent_id
      JOIN dashboard_account_access g ON g.account_id=r.account_id WHERE g.owner_id=$1 ORDER BY r.created_at DESC LIMIT 100
    "#).bind(&owner).fetch_all(state.store.pool()).await;
    match rows{Ok(rows)=>Json(serde_json::json!({"data":rows.into_iter().map(|(id,time,intent,strategy,instrument,account,requested,approved,decision,reason)|serde_json::json!({
      "id":id.to_string(),"time":time,"tradeIntentId":intent,"strategyId":strategy,"instrumentId":instrument,"accountId":account,
      "requestedQuantity":requested.map(dec),"approvedQuantity":approved.map(dec),"decision":match decision.as_str(){"approved"=>"APPROVED","resized"=>"RESIZED","rejected"=>"REJECTED",_=>"HALTED"},"reason":reason.unwrap_or_default()
    })).collect::<Vec<_>>()})).into_response(),Err(_)=>internal()}
}

pub async fn events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListLimit>,
) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let rows=sqlx::query_as::<_,(i64,Option<String>,Option<String>,String,serde_json::Value,DateTime<Utc>)>(r#"
      SELECT e.id,e.order_id,e.venue_id,e.event_type,e.payload_json,e.created_at FROM execution_events e
      JOIN orders o ON o.id=e.order_id JOIN dashboard_account_access g ON g.account_id=o.account_id
      WHERE g.owner_id=$1 ORDER BY e.created_at DESC LIMIT $2
    "#).bind(&owner).bind(limit).fetch_all(state.store.pool()).await;
    match rows{Ok(rows)=>Json(serde_json::json!({"data":rows.into_iter().map(|(id,order,venue,kind,payload,time)|serde_json::json!({
      "id":id.to_string(),"timestamp":time,"type":status_case(&kind),"entity":order,"status":"RECORDED","detail":format!("Execution event for order {}",order.as_deref().unwrap_or("unknown")),"payload":payload,"correlationId":venue
    })).collect::<Vec<_>>()})).into_response(),Err(_)=>internal()}
}

pub async fn reconciliation(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let rows=sqlx::query_as::<_,(String,String,DateTime<Utc>,serde_json::Value)>(r#"
      SELECT r.account_id,r.terminal_id,r.captured_at,r.snapshot FROM mt5_bridge_reconciliation_snapshots r
      JOIN dashboard_account_access g ON g.account_id=r.account_id WHERE g.owner_id=$1
      ORDER BY r.created_at DESC LIMIT 1
    "#).bind(&owner).fetch_optional(state.store.pool()).await;
    match rows{Ok(Some((account,terminal,time,snapshot)))=>Json(serde_json::json!({"data":{"accountId":account,"venueId":terminal,"status":"ALERT","lines":[],"alerts":[{"title":"Reconciliation snapshot available","detail":"Snapshot data is available; live venue comparison is not exposed by this API.","action":"ALERT_ONLY"}],"lastRunAt":time,"snapshot":snapshot}})).into_response(),
      Ok(None)=>Json(serde_json::json!({"data":{"accountId":"","venueId":"","status":"ALERT","lines":[],"alerts":[{"title":"No reconciliation snapshot","detail":"No authorized reconciliation snapshot is available.","action":"ALERT_ONLY"}],"lastRunAt":null}})).into_response(),Err(_)=>internal()}
}

pub async fn system_components(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let _owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let db = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(state.store.pool())
        .await
        .is_ok();
    let now = Utc::now();
    let status = if db { "DEGRADED" } else { "ERROR" };
    let components = vec![
        ("api", "EdgeNest API"),
        ("database", "PostgreSQL"),
        ("risk", "Risk Engine"),
        ("oms", "Order Management"),
        ("market-data", "Market Data"),
        ("venue", "Venue Connectivity"),
        ("reconciliation", "Reconciliation"),
    ];
    Json(serde_json::json!({"data":components.into_iter().map(|(id,name)|serde_json::json!({"id":id,"name":name,"status":if id=="api"{"HEALTHY"}else if id=="database"&&db{"HEALTHY"}else{status},"uptime":"unknown","lastHeartbeat":null,"latencyMs":null,"errorCount":null})).collect::<Vec<_>>()})).into_response()
}
pub async fn system_pipeline(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let _owner = match auth(&state, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let now = Utc::now();
    let stages = [
        "Market Data",
        "Strategy",
        "Trade Intent",
        "Risk",
        "OMS",
        "Venue",
        "Reconciliation",
    ];
    Json(serde_json::json!({"data":stages.iter().enumerate().map(|(i,label)|serde_json::json!({"id":format!("stage-{i}"),"label":label,"status":"DEGRADED","lastActivity":"unknown","metric":"No live activity reported","href":"/system"})).collect::<Vec<_>>()})).into_response()
}
