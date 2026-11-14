//! Read-only, database-backed R5-02 broker admission.
//!
//! The caller must authenticate `actor_ref` and `worker_ref` before calling.
//! This module only loads a repeatable-read snapshot, maps persisted facts into
//! the pure domain evaluator, and returns its decision. It never writes rows,
//! performs provider calls, or treats a broker terminal session as ownership
//! proof. Missing authority, evidence expiry, or worker assignment facts fail
//! closed through the domain evaluator.

use chrono::{DateTime, Utc};
use domain::broker_registry::{
    evaluate_broker_admission as evaluate_facts, AccountControlState,
    AccountIdentityResolution, AdmissionEnvironment, AdmissionEvidenceAuthentication,
    AdmissionEvidenceValidity, AdmissionLifecycleStatus, AdmissionRevision,
    AdmissionSessionState, BrokerAdmissionDecision, BrokerAdmissionFacts,
    BrokerAdmissionRequest, BrokerCatalogAdmissionFacts, BrokerBindingStatus,
    ExternalAccountAdmissionFacts, EvidenceAdmissionFacts, OrganizationAdmissionFacts,
    RegistryAvailability, VerificationAuthorityState, WorkerAdmissionFacts,
    WorkerAssignmentResolution, BrokerRegistryStatus,
};
use sqlx::{FromRow, PgPool};

use crate::error::PersistenceResult;

fn revision(value: i64) -> AdmissionRevision {
    match u64::try_from(value) {
        Ok(value) if value > 0 => AdmissionRevision {
            expected: Some(value),
            observed: Some(value),
        },
        _ => AdmissionRevision::default(),
    }
}

fn registry_status(value: &str) -> Option<BrokerRegistryStatus> {
    match value {
        "ACTIVE" => Some(BrokerRegistryStatus::Active),
        "DISABLED" => Some(BrokerRegistryStatus::Disabled),
        _ => None,
    }
}

fn lifecycle_status(value: Option<&str>) -> Option<AdmissionLifecycleStatus> {
    match value {
        Some("ACTIVE") => Some(AdmissionLifecycleStatus::Active),
        Some("SUSPENDED" | "CLOSED" | "REVOKED" | "INACTIVE") => {
            Some(AdmissionLifecycleStatus::Inactive)
        }
        Some(_) => Some(AdmissionLifecycleStatus::Unavailable),
        None => None,
    }
}

#[derive(FromRow)]
struct CatalogRow {
    broker_status: String,
    server_status: String,
    environment: String,
    broker_revision: i64,
    server_revision: i64,
}

#[derive(FromRow)]
struct BindingRow {
    organization_id: String,
    quant_account_id: String,
    external_account_id: String,
    binding_status: String,
    binding_revision: i64,
    broker_id: String,
    broker_server_id: String,
    external_status: String,
    verification_status: String,
    external_revision: i64,
}

#[derive(FromRow)]
struct OrganizationRow {
    organization_status: String,
    organization_revision: i64,
    membership_status: Option<String>,
    membership_revision: Option<i64>,
    entitlement_status: Option<String>,
    entitlement_revision: Option<i64>,
}

#[derive(FromRow)]
struct EvidenceRow {
    evidence_id: Option<String>,
    decision: String,
    verification_revision: i64,
    approval_actor_id: Option<String>,
    authority_grant_id: Option<String>,
    authority_grant_revision: Option<i64>,
    observed_external_account_id: Option<String>,
    observed_broker_id: Option<String>,
    observed_server_id: Option<String>,
    evidence_status: Option<String>,
    evidence_expires_at: Option<DateTime<Utc>>,
    evidence_revoked_at: Option<DateTime<Utc>>,
    grant_user_id: Option<String>,
    grant_permission: Option<String>,
    grant_status: Option<String>,
    grant_revision: Option<i64>,
    grant_valid_from: Option<DateTime<Utc>>,
    grant_valid_until: Option<DateTime<Utc>>,
    user_active: Option<bool>,
}

#[derive(FromRow)]
struct WorkerRow {
    assignment_id: String,
    assigned_worker_ref: String,
    assignment_status: String,
    assignment_revision: i64,
    organization_id: String,
    quant_account_id: String,
    binding_id: String,
    external_account_id: String,
    broker_id: String,
    broker_server_id: String,
    session_worker_ref: Option<String>,
    session_status: Option<String>,
    session_ready_until: Option<DateTime<Utc>>,
}

/// Evaluate broker admission from persisted facts in one repeatable-read,
/// read-only transaction.
///
/// `actor_ref` and `worker_ref` are authenticated opaque principal references
/// supplied by a trusted caller. This function does not authenticate either
/// principal. A successful permit reflects the exact revisions loaded in the
/// transaction; callers must not cache it as a durable authorization.
pub async fn evaluate_persisted_broker_admission(
    pool: &PgPool,
    request: &BrokerAdmissionRequest,
    actor_ref: &str,
    worker_ref: &str,
) -> PersistenceResult<BrokerAdmissionDecision> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;

    let mut facts = BrokerAdmissionFacts {
        registry_availability: Some(RegistryAvailability::Available),
        ..BrokerAdmissionFacts::default()
    };

    if let Some(row) = sqlx::query_as::<_, CatalogRow>(
        r#"SELECT b.status AS broker_status, s.status AS server_status,
                  s.environment, b.revision AS broker_revision,
                  s.revision AS server_revision
             FROM brokers b
             JOIN broker_servers s ON s.broker_id = b.id
            WHERE b.id = $1 AND s.id = $2"#,
    )
    .bind(&request.broker_ref)
    .bind(&request.server_ref)
    .fetch_optional(&mut *tx)
    .await?
    {
        facts.broker = BrokerCatalogAdmissionFacts {
            broker_ref: Some(request.broker_ref.clone()),
            server_ref: Some(request.server_ref.clone()),
            broker_status: registry_status(&row.broker_status),
            server_status: registry_status(&row.server_status),
            environment: Some(match row.environment.as_str() {
                "DEMO" => AdmissionEnvironment::Demo,
                "LIVE" => AdmissionEnvironment::Live,
                _ => AdmissionEnvironment::Unknown,
            }),
            broker_revision: revision(row.broker_revision),
            server_revision: revision(row.server_revision),
        };
    }

    let binding = sqlx::query_as::<_, BindingRow>(
        r#"SELECT x.organization_id, x.quant_account_id,
                  x.external_broker_account_id AS external_account_id,
                  x.status AS binding_status, x.revision AS binding_revision,
                  a.broker_id, a.broker_server_id, a.status AS external_status,
                  a.verification_status, a.revision AS external_revision
             FROM trading_account_broker_bindings x
             JOIN external_broker_accounts a ON a.id = x.external_broker_account_id
            WHERE x.id = $1
              AND x.organization_id = $2
              AND x.external_broker_account_id = $3"#,
    )
    .bind(&request.binding_ref)
    .bind(&request.organization_ref)
    .bind(&request.account_ref)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(row) = binding.as_ref() {
        facts.external_account = ExternalAccountAdmissionFacts {
            resolution: Some(AccountIdentityResolution::Unique),
            account_ref: Some(row.external_account_id.clone()),
            control: Some(if row.external_status == "ACTIVE"
                && row.verification_status == "VERIFIED"
            {
                AccountControlState::Verified
            } else {
                AccountControlState::Unverified
            }),
            binding_ref: Some(request.binding_ref.clone()),
            binding_status: match row.binding_status.as_str() {
                "PENDING_EVIDENCE" => Some(BrokerBindingStatus::PendingEvidence),
                "PENDING_APPROVAL" => Some(BrokerBindingStatus::PendingApproval),
                "VERIFIED" => Some(BrokerBindingStatus::Verified),
                "SUSPENDED" => Some(BrokerBindingStatus::Suspended),
                "REVOKED" => Some(BrokerBindingStatus::Revoked),
                _ => None,
            },
            account_revision: revision(row.external_revision),
            binding_revision: revision(row.binding_revision),
        };
    }

    let organization = sqlx::query_as::<_, OrganizationRow>(
        r#"SELECT o.status AS organization_status, o.revision AS organization_revision,
                  m.status AS membership_status, m.revision AS membership_revision,
                  e.status AS entitlement_status, e.revision AS entitlement_revision
             FROM organizations o
             LEFT JOIN organization_memberships m
               ON m.organization_id = o.id AND m.user_id = $2
             LEFT JOIN organization_account_entitlements e
               ON e.organization_id = o.id AND e.account_id = $3
            WHERE o.id = $1"#,
    )
    .bind(&request.organization_ref)
    .bind(actor_ref)
    .bind(&request.account_ref)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(row) = organization {
        facts.organization = OrganizationAdmissionFacts {
            organization_ref: Some(request.organization_ref.clone()),
            organization_status: lifecycle_status(Some(&row.organization_status)),
            membership_ref: row.membership_status.as_ref().map(|_| actor_ref.to_owned()),
            membership_status: lifecycle_status(row.membership_status.as_deref()),
            entitlement_ref: row.entitlement_status.as_ref().map(|_| request.account_ref.clone()),
            entitlement_status: lifecycle_status(row.entitlement_status.as_deref()),
            organization_revision: revision(row.organization_revision),
            membership_revision: row.membership_revision.map(revision).unwrap_or_default(),
            entitlement_revision: row.entitlement_revision.map(revision).unwrap_or_default(),
        };
    }

    if binding.is_some() {
        let evidence = sqlx::query_as::<_, EvidenceRow>(
            r#"SELECT v.evidence_id, v.decision,
                      v.revision AS verification_revision, v.approval_actor_id,
                      v.authority_grant_id, v.authority_grant_revision,
                      e.external_broker_account_id AS observed_external_account_id,
                      e.observed_broker_id, e.observed_broker_server_id AS observed_server_id,
                      e.evidence_status, e.expires_at AS evidence_expires_at,
                      e.revoked_at AS evidence_revoked_at,
                      g.user_id AS grant_user_id, g.permission AS grant_permission,
                      g.status AS grant_status, g.revision AS grant_revision,
                      g.valid_from AS grant_valid_from, g.valid_until AS grant_valid_until,
                      u.is_active AS user_active
                 FROM broker_account_verifications v
                 LEFT JOIN broker_account_evidence e ON e.id = v.evidence_id
                 LEFT JOIN broker_platform_authority_grants g ON g.id = v.authority_grant_id
                 LEFT JOIN identity_users u ON u.id = g.user_id
                WHERE v.external_broker_account_id = $1
                ORDER BY v.revision DESC
                LIMIT 1"#,
        )
        .bind(&request.account_ref)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = evidence {
            let now = Utc::now();
            let evidence_present = row.evidence_id.is_some();
            let identity_matches = row.observed_external_account_id.as_deref()
                == Some(request.account_ref.as_str())
                && row.observed_broker_id.as_deref() == Some(request.broker_ref.as_str())
                && row.observed_server_id.as_deref() == Some(request.server_ref.as_str());

            let validity = if row.evidence_revoked_at.is_some()
                || matches!(row.decision.as_str(), "REVOKED" | "SUSPENDED" | "REJECTED")
            {
                AdmissionEvidenceValidity::RevokedOrSuperseded
            } else if row.evidence_expires_at.map_or(true, |expiry| expiry <= now) {
                AdmissionEvidenceValidity::Expired
            } else if !identity_matches {
                AdmissionEvidenceValidity::Conflict
            } else if row.decision != "VERIFIED"
                || row.evidence_status.as_deref() != Some("AUTHENTICATED")
            {
                AdmissionEvidenceValidity::Invalid
            } else {
                AdmissionEvidenceValidity::Valid
            };

            let authority_valid = row.decision == "VERIFIED"
                && row.authority_grant_id.is_some()
                && row.authority_grant_revision == row.grant_revision
                && row.grant_user_id == row.approval_actor_id
                && row.grant_permission.as_deref() == Some("BROKER_ACCOUNT_VERIFY")
                && row.grant_status.as_deref() == Some("ACTIVE")
                && row.user_active == Some(true)
                && row.grant_valid_from.map_or(false, |start| start <= now)
                && row.grant_valid_until.map_or(true, |expiry| expiry > now);

            facts.evidence = EvidenceAdmissionFacts {
                evidence_ref: row.evidence_id,
                external_account_ref: row.observed_external_account_id,
                broker_ref: row.observed_broker_id,
                server_ref: row.observed_server_id,
                authentication: Some(if row.evidence_status.as_deref() == Some("AUTHENTICATED") {
                    AdmissionEvidenceAuthentication::Authenticated
                } else {
                    AdmissionEvidenceAuthentication::Unauthenticated
                }),
                validity: Some(validity),
                verification_authority: Some(if authority_valid {
                    VerificationAuthorityState::Authorized
                } else {
                    VerificationAuthorityState::Unauthorized
                }),
                revision: revision(row.verification_revision),
            };

            if !evidence_present {
                facts.evidence.evidence_ref = None;
            }
        }
    }

    let active_assignment_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
             FROM broker_worker_assignments
            WHERE binding_id = $1 AND status = 'ACTIVE'
              AND valid_from <= NOW() AND valid_until > NOW()"#,
    )
    .bind(&request.binding_ref)
    .fetch_one(&mut *tx)
    .await?;

    if active_assignment_count == 0 {
        facts.worker.assignment_resolution = Some(WorkerAssignmentResolution::Missing);
    } else if active_assignment_count != 1 {
        facts.worker.assignment_resolution = Some(WorkerAssignmentResolution::Ambiguous);
    } else {
        let worker = sqlx::query_as::<_, WorkerRow>(
            r#"SELECT a.id AS assignment_id, a.worker_ref AS assigned_worker_ref,
                      a.status AS assignment_status, a.revision AS assignment_revision,
                      a.organization_id, a.quant_account_id, a.binding_id,
                      a.external_broker_account_id AS external_account_id,
                      a.broker_id, a.broker_server_id,
                      s.worker_ref AS session_worker_ref, s.status AS session_status,
                      s.ready_until AS session_ready_until
                 FROM broker_worker_assignments a
                 LEFT JOIN LATERAL (
                    SELECT worker_ref, status, ready_until
                      FROM broker_worker_sessions
                     WHERE assignment_id = a.id
                     ORDER BY created_at DESC, id DESC
                     LIMIT 1
                 ) s ON TRUE
                WHERE a.binding_id = $1 AND a.status = 'ACTIVE'
                  AND a.valid_from <= NOW() AND a.valid_until > NOW()
                LIMIT 1"#,
        )
        .bind(&request.binding_ref)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = worker {
            let worker_matches = row.assigned_worker_ref == worker_ref;
            let session_ready = worker_matches
                && row.session_worker_ref.as_deref() == Some(worker_ref)
                && row.session_status.as_deref() == Some("READY")
                && row.session_ready_until.map_or(false, |until| until > Utc::now());

            facts.worker = WorkerAdmissionFacts {
                worker_ref: worker_matches.then_some(row.assigned_worker_ref),
                assignment_ref: Some(row.assignment_id),
                assignment_resolution: Some(if worker_matches {
                    WorkerAssignmentResolution::Unique
                } else {
                    WorkerAssignmentResolution::Missing
                }),
                assigned_binding_ref: Some(row.binding_id),
                assigned_account_ref: Some(row.external_account_id),
                assigned_broker_ref: Some(row.broker_id),
                assigned_server_ref: Some(row.broker_server_id),
                session_state: Some(if session_ready {
                    AdmissionSessionState::Ready
                } else {
                    AdmissionSessionState::NotReady
                }),
                revision: revision(row.assignment_revision),
            };
        } else {
            facts.worker.assignment_resolution = Some(WorkerAssignmentResolution::Missing);
        }
    }

    let decision = evaluate_facts(request, &facts);
    // A commit failure invalidates the snapshot and must not return a permit.
    tx.commit().await?;
    Ok(decision)
}
