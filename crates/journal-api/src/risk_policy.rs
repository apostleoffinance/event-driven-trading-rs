//! Owner-scoped account risk policy configuration and deterministic calculator assessment.
//! This is a planning/risk-analysis API; it never authorizes or submits orders.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{auth::{ApiAuthError, Claims}, AppState};

fn principal(state: &AppState, headers: &HeaderMap) -> Result<Claims, ApiAuthError> {
    state.auth.authenticate(headers)
}
fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(serde_json::json!({"error":{"code":code,"message":message}}))).into_response()
}
fn internal() -> Response {
    error(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "Internal server error")
}
fn not_found() -> Response {
    error(StatusCode::NOT_FOUND, "not_found", "Account not found")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRiskPolicy {
    pub profile_name: String,
    pub firm_name: Option<String>,
    pub program_name: Option<String>,
    pub account_type: String,
    pub risk_appetite: String,
    /// Max risk per trade as a percentage of current account equity.
    pub risk_per_trade_pct: Decimal,
    /// Optional absolute per-trade cap in account currency.
    pub max_risk_per_trade_amount: Option<Decimal>,
    pub max_daily_loss_pct: Option<Decimal>,
    /// initial_balance | start_of_day_balance | start_of_day_equity
    pub daily_loss_basis: String,
    pub max_total_drawdown_pct: Option<Decimal>,
    /// static_initial | trailing_high_water
    pub drawdown_basis: String,
    pub max_trades_per_day: Option<u32>,
    pub max_open_positions: Option<u32>,
    pub max_total_open_risk_pct: Option<Decimal>,
    pub minimum_rr: Option<Decimal>,
    pub max_lot_size: Option<Decimal>,
    pub max_leverage: Option<Decimal>,
    /// block | warn. Firm-rule limits default to blocking.
    pub firm_limits_enforcement: String,
    /// block | warn. Personal preference limits default to warnings.
    pub personal_limits_enforcement: String,
    pub daily_reset_timezone: String,
    pub notes: Option<String>,
}

impl Default for AccountRiskPolicy {
    fn default() -> Self {
        Self {
            profile_name: "My risk policy".into(),
            firm_name: None,
            program_name: None,
            account_type: "personal".into(),
            risk_appetite: "balanced".into(),
            risk_per_trade_pct: Decimal::new(5, 1), // 0.5%
            max_risk_per_trade_amount: None,
            max_daily_loss_pct: None,
            daily_loss_basis: "start_of_day_equity".into(),
            max_total_drawdown_pct: None,
            drawdown_basis: "trailing_high_water".into(),
            max_trades_per_day: None,
            max_open_positions: None,
            max_total_open_risk_pct: None,
            minimum_rr: Some(Decimal::ONE),
            max_lot_size: None,
            max_leverage: None,
            firm_limits_enforcement: "block".into(),
            personal_limits_enforcement: "warn".into(),
            daily_reset_timezone: "UTC".into(),
            notes: None,
        }
    }
}

fn valid_pct(v: Option<Decimal>) -> bool {
    v.is_none_or(|x| x > Decimal::ZERO && x <= Decimal::from(100))
}
fn validate(p: &AccountRiskPolicy) -> Result<(), &'static str> {
    if p.profile_name.trim().is_empty() || p.profile_name.len() > 120 {
        return Err("Policy name must contain 1–120 characters.");
    }
    if p.firm_name.as_ref().is_some_and(|v| v.len() > 120)
        || p.program_name.as_ref().is_some_and(|v| v.len() > 120)
        || p.notes.as_ref().is_some_and(|v| v.len() > 2000)
    {
        return Err("Firm, program, or notes field exceeds its maximum length.");
    }
    if !["personal", "prop", "challenge", "funded", "demo"].contains(&p.account_type.as_str()) {
        return Err("Account type must be personal, prop, challenge, funded, or demo.");
    }
    if !["conservative", "balanced", "aggressive", "custom"].contains(&p.risk_appetite.as_str()) {
        return Err("Risk appetite must be conservative, balanced, aggressive, or custom.");
    }
    if p.risk_per_trade_pct <= Decimal::ZERO || p.risk_per_trade_pct > Decimal::from(100) {
        return Err("Risk per trade must be greater than 0 and at most 100%.");
    }
    if p.max_risk_per_trade_amount.is_some_and(|v| v <= Decimal::ZERO) {
        return Err("Absolute risk cap must be greater than zero.");
    }
    if !valid_pct(p.max_daily_loss_pct)
        || !valid_pct(p.max_total_drawdown_pct)
        || !valid_pct(p.max_total_open_risk_pct)
    {
        return Err("Percentage limits must be greater than 0 and at most 100% when set.");
    }
    if p.minimum_rr.is_some_and(|v| v <= Decimal::ZERO || v > Decimal::from(100)) {
        return Err("Minimum R:R must be greater than 0 and at most 100.");
    }
    if p.max_lot_size.is_some_and(|v| v <= Decimal::ZERO)
        || p.max_leverage.is_some_and(|v| v <= Decimal::ZERO)
        || p.max_trades_per_day == Some(0)
        || p.max_open_positions == Some(0)
    {
        return Err("Lot, leverage, trade-count, and position limits must be positive.");
    }
    if !["initial_balance", "start_of_day_balance", "start_of_day_equity"].contains(&p.daily_loss_basis.as_str()) {
        return Err("Invalid daily loss basis.");
    }
    if !["static_initial", "trailing_high_water"].contains(&p.drawdown_basis.as_str()) {
        return Err("Invalid drawdown basis.");
    }
    if !["block", "warn"].contains(&p.firm_limits_enforcement.as_str())
        || !["block", "warn"].contains(&p.personal_limits_enforcement.as_str())
    {
        return Err("Enforcement must be block or warn.");
    }
    if p.daily_reset_timezone.trim().is_empty() || p.daily_reset_timezone.len() > 64 {
        return Err("Daily reset timezone must contain 1–64 characters.");
    }
    Ok(())
}

async fn owned_account(pool: &sqlx::PgPool, owner: &str, account: &str) -> Result<(String, Decimal), sqlx::Error> {
    sqlx::query_as::<_, (String, Decimal)>(
        "SELECT currency, initial_balance FROM journal_accounts WHERE owner_id=$1 AND id=$2 AND archived=FALSE"
    ).bind(owner).bind(account).fetch_optional(pool).await?.ok_or(sqlx::Error::RowNotFound)
}

pub async fn get(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let claims = match principal(&state, &headers) { Ok(c) => c, Err(e) => return e.into_response() };
    if owned_account(state.store.pool(), &claims.sub, &id).await.is_err() { return not_found(); }
    match sqlx::query_as::<_, (Value, i32, chrono::DateTime<chrono::Utc>)>(
        "SELECT policy_json, version, updated_at FROM journal_account_risk_policies WHERE owner_id=$1 AND account_id=$2"
    ).bind(&claims.sub).bind(&id).fetch_optional(state.store.pool()).await {
        Ok(Some((policy, version, updated_at))) => Json(serde_json::json!({"policy":policy,"version":version,"updated_at":updated_at})).into_response(),
        Ok(None) => Json(serde_json::json!({"policy":AccountRiskPolicy::default(),"version":0,"updated_at":null})).into_response(),
        Err(_) => internal(),
    }
}

pub async fn put(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(policy): Json<AccountRiskPolicy>) -> Response {
    let claims = match principal(&state, &headers) { Ok(c) => c, Err(e) => return e.into_response() };
    let (currency, _initial_balance) = match owned_account(state.store.pool(), &claims.sub, &id).await {
        Ok(account) => account,
        Err(_) => return not_found(),
    };
    if let Err(message) = validate(&policy) { return error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_policy", message); }
    let json = match serde_json::to_value(&policy) { Ok(v) => v, Err(_) => return internal() };
    match sqlx::query(
        "INSERT INTO journal_account_risk_policies(owner_id,account_id,policy_json,version,updated_at) VALUES($1,$2,$3,1,NOW()) ON CONFLICT(owner_id,account_id) DO UPDATE SET policy_json=EXCLUDED.policy_json,version=journal_account_risk_policies.version+1,updated_at=NOW() RETURNING version,updated_at"
    ).bind(&claims.sub).bind(&id).bind(json).fetch_one(state.store.pool()).await {
        Ok(row) => {
            use sqlx::Row;
            Json(serde_json::json!({"policy":policy,"version":row.get::<i32,_>("version"),"updated_at":row.get::<chrono::DateTime<Utc>,_>("updated_at"),"account_currency":currency})).into_response()
        },
        Err(_) => internal(),
    }
}

#[derive(Debug, Deserialize)]
pub struct PolicyEvaluationInput {
    pub risk_amount: Decimal,
    pub potential_reward: Option<Decimal>,
    pub equity: Decimal,
    pub initial_balance: Option<Decimal>,
    pub start_of_day_balance: Option<Decimal>,
    pub start_of_day_equity: Option<Decimal>,
    pub peak_equity: Option<Decimal>,
    #[serde(default)]
    pub daily_pnl: Decimal,
    #[serde(default)]
    pub daily_risk_used: Decimal,
    #[serde(default)]
    pub trades_today: u32,
    #[serde(default)]
    pub open_positions: u32,
    pub current_open_risk: Option<Decimal>,
    pub lot_size: Option<Decimal>,
}

pub async fn evaluate(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(input): Json<PolicyEvaluationInput>) -> Response {
    let claims = match principal(&state, &headers) { Ok(c) => c, Err(e) => return e.into_response() };
    let (currency, initial_balance) = match owned_account(state.store.pool(), &claims.sub, &id).await {
        Ok(account) => account,
        Err(_) => return not_found(),
    };
    if input.risk_amount <= Decimal::ZERO || input.equity <= Decimal::ZERO
        || input.daily_pnl > input.equity || input.daily_risk_used < Decimal::ZERO
        || input.current_open_risk.is_some_and(|v| v < Decimal::ZERO)
        || input.lot_size.is_some_and(|v| v <= Decimal::ZERO)
    {
        return error(StatusCode::BAD_REQUEST, "invalid_evaluation", "Risk, equity, and lot inputs must be valid positive/non-negative values.");
    }
    let policy_json = match sqlx::query_scalar::<_, Value>("SELECT policy_json FROM journal_account_risk_policies WHERE owner_id=$1 AND account_id=$2")
        .bind(&claims.sub).bind(&id).fetch_optional(state.store.pool()).await {
        Ok(Some(v)) => v,
        Ok(None) => match serde_json::to_value(AccountRiskPolicy::default()) { Ok(v)=>v, Err(_)=>return internal() },
        Err(_) => return internal(),
    };
    let policy: AccountRiskPolicy = match serde_json::from_value(policy_json) { Ok(p)=>p, Err(_)=>return error(StatusCode::INTERNAL_SERVER_ERROR,"invalid_saved_policy","Saved risk policy could not be parsed. Please edit and save it again.") };
    if let Err(message) = validate(&policy) { return error(StatusCode::INTERNAL_SERVER_ERROR, "invalid_saved_policy", message); }

    let mut violations: Vec<Value> = Vec::new();
    let mut blocked = false;
    let mut warned = false;
    let mut push = |code: &str, message: String, severity: &str| {
        if severity == "block" { blocked = true; } else { warned = true; }
        violations.push(serde_json::json!({"code":code,"message":message,"severity":severity}));
    };
    let per_trade_cap = input.equity * policy.risk_per_trade_pct / Decimal::from(100);
    let effective_cap = policy.max_risk_per_trade_amount.map_or(per_trade_cap, |v| per_trade_cap.min(v));
    if input.risk_amount > effective_cap {
        push("trade_risk_cap", format!("Planned risk {} {} exceeds the effective per-trade cap {} {}.", input.risk_amount, currency, effective_cap, currency), &policy.personal_limits_enforcement);
    }
    if let (Some(min_rr),Some(reward))=(policy.minimum_rr,input.potential_reward) {
        if reward <= Decimal::ZERO || reward / input.risk_amount < min_rr {
            push("minimum_rr",format!("Planned reward/risk is below the configured minimum {}R.",min_rr),&policy.personal_limits_enforcement);
        }
    }
    if let Some(max_daily_pct)=policy.max_daily_loss_pct {
        let base=match policy.daily_loss_basis.as_str() {
            "initial_balance"=>input.initial_balance.unwrap_or(initial_balance),
            "start_of_day_balance"=>input.start_of_day_balance.unwrap_or(input.equity),
            _=>input.start_of_day_equity.unwrap_or(input.equity),
        };
        if base <= Decimal::ZERO { return error(StatusCode::BAD_REQUEST,"invalid_daily_loss_basis","Daily-loss calculation base must be positive."); }
        let max_daily=base*max_daily_pct/Decimal::from(100);
        let used_loss=(-input.daily_pnl).max(Decimal::ZERO);
        if used_loss >= max_daily || used_loss + input.risk_amount >= max_daily {
            push("daily_loss_limit",format!("Daily loss budget {} {} ({}%) is reached or would be breached by this planned risk.",max_daily,currency,max_daily_pct),&policy.firm_limits_enforcement);
        }
    }
    if let Some(max_dd)=policy.max_total_drawdown_pct {
        let peak=match policy.drawdown_basis.as_str() {
            "static_initial"=>input.initial_balance.unwrap_or(initial_balance),
            _=>input.peak_equity.unwrap_or(input.equity),
        };
        if peak <= Decimal::ZERO { return error(StatusCode::BAD_REQUEST,"invalid_drawdown_basis","Drawdown reference must be positive."); }
        let dd=(peak-input.equity).max(Decimal::ZERO)/peak*Decimal::from(100);
        let projected=(peak-input.equity+input.risk_amount).max(Decimal::ZERO)/peak*Decimal::from(100);
        if dd >= max_dd || projected >= max_dd {
            push("max_drawdown",format!("Current/projected drawdown {:.4}% / {:.4}% meets or exceeds the configured {}% limit.",dd,projected,max_dd),&policy.firm_limits_enforcement);
        }
    }
    if policy.max_trades_per_day.is_some_and(|n| input.trades_today >= n) {
        push("max_trades_per_day",format!("Daily trade limit of {} has been reached.",policy.max_trades_per_day.unwrap_or(0)),&policy.firm_limits_enforcement);
    }
    if policy.max_open_positions.is_some_and(|n| input.open_positions >= n) {
        push("max_open_positions",format!("Open-position limit of {} has been reached.",policy.max_open_positions.unwrap_or(0)),&policy.firm_limits_enforcement);
    }
    if let Some(max_open_risk_pct)=policy.max_total_open_risk_pct {
        let used=input.current_open_risk.unwrap_or(input.daily_risk_used);
        let total_cap=input.equity*max_open_risk_pct/Decimal::from(100);
        if used+input.risk_amount > total_cap {
            push("max_open_risk",format!("Current open risk plus this trade exceeds the {}% account-equity open-risk limit.",max_open_risk_pct),&policy.firm_limits_enforcement);
        }
    }
    if policy.max_lot_size.is_some_and(|max| input.lot_size.is_some_and(|lot| lot > max)) {
        push("max_lot_size",format!("Requested lot size exceeds configured maximum {}.",policy.max_lot_size.unwrap_or(Decimal::ZERO)),&policy.personal_limits_enforcement);
    }
    let status=if blocked {"blocked"} else if warned {"warning"} else {"within_policy"};
    Json(serde_json::json!({
        "status":status,"currency":currency,"policy_name":policy.profile_name,
        "risk_amount":input.risk_amount.to_string(),"risk_pct_of_equity":(input.risk_amount/input.equity*Decimal::from(100)).round_dp(4).to_string(),
        "effective_trade_risk_cap":effective_cap.to_string(),"violations":violations,
        "enforcement_note":"Risk assessment is advisory for planning. No order is created or sent."
    })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_conservative_and_valid() {
        let policy=AccountRiskPolicy::default();
        assert_eq!(policy.risk_per_trade_pct,Decimal::new(5,1));
        assert!(validate(&policy).is_ok());
    }

    #[test]
    fn invalid_policy_percent_is_rejected() {
        let mut policy=AccountRiskPolicy::default();
        policy.risk_per_trade_pct=Decimal::from(101);
        assert_eq!(validate(&policy),Err("Risk per trade must be greater than 0 and at most 100%."));
    }

    #[test]
    fn policy_requires_hard_firm_enforcement_option() {
        let mut policy=AccountRiskPolicy::default();
        policy.firm_limits_enforcement="ignore".into();
        assert!(validate(&policy).is_err());
    }
}
