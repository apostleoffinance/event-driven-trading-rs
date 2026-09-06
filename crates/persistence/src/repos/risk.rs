use domain::{
    AccountId, RiskDecision, RiskProfile, RiskRejectReason, StrategyDeployment, StrategyId,
    StrategyVersion, TradeIntentId,
};
use sqlx::PgPool;

use crate::error::PersistenceResult;

pub async fn ensure_strategy_graph(
    pool: &PgPool,
    strategy_id: &StrategyId,
    strategy_version: &StrategyVersion,
    name: &str,
    deployment: &StrategyDeployment,
    risk_profile: &RiskProfile,
) -> PersistenceResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"INSERT INTO strategies (id, name) VALUES ($1, $2)
           ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name"#,
    )
    .bind(strategy_id.as_str())
    .bind(name)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"INSERT INTO strategy_versions (strategy_id, version) VALUES ($1, $2)
           ON CONFLICT DO NOTHING"#,
    )
    .bind(strategy_id.as_str())
    .bind(strategy_version.as_str())
    .execute(&mut *tx)
    .await?;

    let policy_json = serde_json::to_value(&risk_profile.policy)?;
    sqlx::query(
        r#"INSERT INTO risk_profiles (id, name, policy_json) VALUES ($1, $2, $3)
           ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, policy_json = EXCLUDED.policy_json"#,
    )
    .bind(risk_profile.id.as_str())
    .bind(&risk_profile.name)
    .bind(policy_json)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO strategy_deployments (
            id, strategy_id, strategy_version, account_id, risk_profile_id, enabled, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7)
        ON CONFLICT (id) DO UPDATE SET
            enabled = EXCLUDED.enabled,
            risk_profile_id = EXCLUDED.risk_profile_id
        "#,
    )
    .bind(deployment.id.as_str())
    .bind(deployment.strategy_id.as_str())
    .bind(deployment.strategy_version.as_str())
    .bind(deployment.account_id.as_str())
    .bind(deployment.risk_profile_id.as_str())
    .bind(deployment.enabled)
    .bind(deployment.created_at)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn insert_decision(
    pool: &PgPool,
    trade_intent_id: &TradeIntentId,
    account_id: &AccountId,
    decision: &RiskDecision,
) -> PersistenceResult<()> {
    let (decision_type, quantity, reason) = match decision {
        RiskDecision::Approved { quantity } => ("approved", Some(*quantity), None),
        RiskDecision::Resized { quantity, reason } => {
            ("resized", Some(*quantity), Some(reason.clone()))
        }
        RiskDecision::Rejected { reason } => (
            "rejected",
            None,
            Some(reject_reason_str(reason).to_string()),
        ),
        RiskDecision::Halted { reason } => ("halted", None, Some(reason.clone())),
    };
    let decision_json = serde_json::to_value(decision)?;
    sqlx::query(
        r#"
        INSERT INTO risk_decisions (
            trade_intent_id, account_id, decision_type, quantity, reason, decision_json, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6, NOW())
        "#,
    )
    .bind(trade_intent_id.as_str())
    .bind(account_id.as_str())
    .bind(decision_type)
    .bind(quantity)
    .bind(reason)
    .bind(decision_json)
    .execute(pool)
    .await?;
    Ok(())
}

fn reject_reason_str(reason: &RiskRejectReason) -> String {
    match reason {
        RiskRejectReason::GlobalHalt => "global_halt".into(),
        RiskRejectReason::AccountHalt => "account_halt".into(),
        RiskRejectReason::StrategyHalt => "strategy_halt".into(),
        RiskRejectReason::InstrumentRestricted => "instrument_restricted".into(),
        RiskRejectReason::SessionNotAllowed => "session_not_allowed".into(),
        RiskRejectReason::DrawdownBreached => "drawdown_breached".into(),
        RiskRejectReason::DailyLossBreached => "daily_loss_breached".into(),
        RiskRejectReason::ExposureLimit => "exposure_limit".into(),
        RiskRejectReason::PositionLimit => "position_limit".into(),
        RiskRejectReason::LeverageLimit => "leverage_limit".into(),
        RiskRejectReason::TradeRiskLimit => "trade_risk_limit".into(),
        RiskRejectReason::InvalidRequest => "invalid_request".into(),
        RiskRejectReason::Other(s) => s.clone(),
    }
}
