//! Registry-backed admission for one managed MT5 terminal slot.
//!
//! This is intentionally separate from broker ownership admission: a valid broker
//! binding does not prove that a particular Windows terminal/worker is the assigned
//! publisher. Every telemetry frame must satisfy both checks.

use sqlx::PgPool;

use crate::error::PersistenceResult;

/// Return true only when the exact slot, managed gateway, worker assignment and
/// current READY worker session all agree with the authenticated daemon identity.
#[allow(clippy::too_many_arguments)]
pub async fn validate_ready_mt5_terminal_slot(
    pool: &PgPool,
    terminal_slot_id: &str,
    organization_id: &str,
    quant_account_id: &str,
    binding_id: &str,
    external_account_id: &str,
    broker_id: &str,
    broker_server_id: &str,
    terminal_id: &str,
    worker_ref: &str,
) -> PersistenceResult<bool> {
    let ready: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
              FROM mt5_terminal_slots slot
              JOIN mt5_gateway_hosts host
                ON host.id = slot.gateway_host_id
              JOIN broker_worker_assignments assignment
                ON assignment.terminal_slot_id = slot.id
              JOIN LATERAL (
                    SELECT worker_ref, status, ready_until
                      FROM broker_worker_sessions
                     WHERE assignment_id = assignment.id
                     ORDER BY created_at DESC, id DESC
                     LIMIT 1
              ) session ON TRUE
             WHERE slot.id = $1
               AND slot.status = 'READY'
               AND host.status = 'READY'
               AND slot.organization_id = $2
               AND slot.quant_account_id = $3
               AND slot.binding_id = $4
               AND slot.external_broker_account_id = $5
               AND slot.broker_id = $6
               AND slot.broker_server_id = $7
               AND slot.terminal_id = $8
               AND slot.worker_ref = $9
               AND assignment.worker_ref = $9
               AND assignment.organization_id = $2
               AND assignment.quant_account_id = $3
               AND assignment.binding_id = $4
               AND assignment.external_broker_account_id = $5
               AND assignment.broker_id = $6
               AND assignment.broker_server_id = $7
               AND assignment.status = 'ACTIVE'
               AND assignment.valid_from <= NOW()
               AND assignment.valid_until > NOW()
               AND session.worker_ref = $9
               AND session.status = 'READY'
               AND session.ready_until > NOW()
        )
        "#,
    )
    .bind(terminal_slot_id)
    .bind(organization_id)
    .bind(quant_account_id)
    .bind(binding_id)
    .bind(external_account_id)
    .bind(broker_id)
    .bind(broker_server_id)
    .bind(terminal_id)
    .bind(worker_ref)
    .fetch_one(pool)
    .await?;
    Ok(ready)
}
