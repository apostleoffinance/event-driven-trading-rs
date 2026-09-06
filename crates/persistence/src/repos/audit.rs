use domain::{AccountId, TradeIntentId};
use execution_engine::{AuditEvent, AuditKind};
use sqlx::PgPool;

use crate::error::PersistenceResult;

fn kind_str(kind: AuditKind) -> &'static str {
    match kind {
        AuditKind::OrderCreated => "order_created",
        AuditKind::OrderIdempotentReplay => "order_idempotent_replay",
        AuditKind::PendingRisk => "pending_risk",
        AuditKind::RiskApproved => "risk_approved",
        AuditKind::RiskRejected => "risk_rejected",
        AuditKind::Submitted => "submitted",
        AuditKind::Accepted => "accepted",
        AuditKind::Rejected => "rejected",
        AuditKind::PartiallyFilled => "partially_filled",
        AuditKind::Filled => "filled",
        AuditKind::CancelPending => "cancel_pending",
        AuditKind::Cancelled => "cancelled",
        AuditKind::Failed => "failed",
    }
}

pub async fn insert(
    pool: &PgPool,
    event: &AuditEvent,
    account_id: Option<&AccountId>,
    trade_intent_id: Option<&TradeIntentId>,
) -> PersistenceResult<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_events (
            order_id, client_order_id, account_id, trade_intent_id, kind, detail, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7)
        "#,
    )
    .bind(event.order_id.as_str())
    .bind(event.client_order_id.as_str())
    .bind(account_id.map(|id| id.as_str()))
    .bind(trade_intent_id.map(|id| id.as_str()))
    .bind(kind_str(event.kind))
    .bind(&event.detail)
    .bind(event.at)
    .execute(pool)
    .await?;
    Ok(())
}
