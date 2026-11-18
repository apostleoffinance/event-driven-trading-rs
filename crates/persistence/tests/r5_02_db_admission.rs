#![allow(clippy::unwrap_used)]

use chrono::Utc;
use domain::broker_registry::{
    AdmissionOperation, BrokerAdmissionDecision, BrokerAdmissionDenialReason,
    BrokerAdmissionRequest,
};
use domain::{EntitlementStatus, Organization, OrganizationId};
use persistence::{evaluate_persisted_broker_admission, migrate, TradingStore};
use sqlx::PgPool;

async fn insert_user(pool: &PgPool, id: &str) {
    sqlx::query(
        "INSERT INTO identity_users(id,email,display_name,password_hash) VALUES($1,$2,$1,'test-only')",
    )
    .bind(id)
    .bind(format!("{id}@example.test"))
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_tenant_and_account(pool: &PgPool) {
    insert_user(pool, "r502-owner").await;
    insert_user(pool, "r502-verifier").await;
    TradingStore::new(pool.clone())
        .create_organization(
            &Organization::new(
                OrganizationId::new("r502-org").unwrap(),
                "R5-02 integration test",
                Utc::now(),
            )
            .unwrap(),
            "r502-owner",
        )
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO venues(id,venue_type,name,created_at) VALUES('simulated','simulated','Paper',NOW()) ON CONFLICT(id) DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES('r502-quant-account','simulated','active','simulated',100,NOW(),'PAPER')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO account_states(account_id,balance,equity,used_margin,open_position_count,daily_realized_pnl,peak_equity,kill_switch,updated_at) VALUES('r502-quant-account',100,100,0,0,0,100,FALSE,NOW())",
    )
    .execute(pool)
    .await
    .unwrap();
    TradingStore::new(pool.clone())
        .assign_account_entitlement(
            "r502-org",
            "r502-quant-account",
            "r502-owner",
            1,
            0,
            "R5-02 integration fixture",
        )
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO brokers(id,canonical_name,display_name,normalization_version) VALUES('r502-broker','r502-broker','R5-02 Broker',1)",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO broker_servers(id,broker_id,canonical_identifier,display_name,provider_type,environment,normalization_version,source_reference) VALUES('r502-server','r502-broker','r502.demo','R5-02 Demo','MT5_PYTHON','DEMO',1,'controlled-test-seed')",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_pending_binding(pool: &PgPool) {
    TradingStore::new(pool.clone())
        .register_pending_broker_binding(
            "r502-org",
            "r502-owner",
            "r502-quant-account",
            "r502-broker",
            "r502-server",
            "r502-external-account",
            "r502-binding",
            1,
            "test-key-v1",
            &[("test-key-v1".to_owned(), [0x5a; 32])],
            "R5-02 integration fixture",
        )
        .await
        .unwrap();
}

fn request() -> BrokerAdmissionRequest {
    BrokerAdmissionRequest {
        request_ref: "r502-request".into(),
        operation: AdmissionOperation::ReadOnlyHistorical,
        organization_ref: "r502-org".into(),
        account_ref: "r502-external-account".into(),
        binding_ref: "r502-binding".into(),
        broker_ref: "r502-broker".into(),
        server_ref: "r502-server".into(),
    }
}

#[sqlx::test]
async fn persisted_pending_binding_fails_closed(pool: PgPool) {
    migrate(&pool).await.unwrap();
    seed_tenant_and_account(&pool).await;
    seed_pending_binding(&pool).await;

    let decision =
        evaluate_persisted_broker_admission(&pool, &request(), "r502-owner", "r502-worker")
            .await
            .unwrap();
    assert!(matches!(
        decision,
        BrokerAdmissionDecision::Denied(ref reasons)
            if reasons.contains(&BrokerAdmissionDenialReason::AccountControlUnverified)
                && reasons.contains(&BrokerAdmissionDenialReason::EvidenceMissing)
                && reasons.contains(&BrokerAdmissionDenialReason::WorkerAssignmentMissingOrAmbiguous)
    ));
}

#[sqlx::test]
async fn verified_persisted_snapshot_admits_only_while_authority_and_session_are_current(
    pool: PgPool,
) {
    migrate(&pool).await.unwrap();
    seed_tenant_and_account(&pool).await;
    seed_pending_binding(&pool).await;

    sqlx::query(
        "UPDATE external_broker_accounts SET status='ACTIVE',verification_status='PENDING_APPROVAL',revision=revision+1,updated_at=NOW() WHERE id='r502-external-account'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let lookup_hmac = [0x5a_u8; 32];
    let attestation_digest = [0x11_u8; 32];
    sqlx::query(
        "INSERT INTO broker_account_evidence(id,external_broker_account_id,provider_type,evidence_type,observed_broker_id,observed_broker_server_id,observed_identifier_key_version,observed_identifier_normalization_version,observed_identifier_hmac,attestation_key_id,attestation_digest,observed_at,evidence_status,source_principal,expires_at) VALUES('r502-evidence','r502-external-account','MT5_PYTHON','ACCOUNT_IDENTITY','r502-broker','r502-server','test-key-v1',1,$1,'test-attestation-key',$2,NOW(),'AUTHENTICATED','test-worker',NOW()+INTERVAL '15 minutes')",
    )
    .bind(lookup_hmac.as_slice())
    .bind(attestation_digest.as_slice())
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO broker_platform_authority_grants(id,user_id,permission,status,revision,valid_from,valid_until,granted_by,reason) VALUES('r502-verifier-grant','r502-verifier','BROKER_ACCOUNT_VERIFY','ACTIVE',1,NOW()-INTERVAL '1 minute',NOW()+INTERVAL '1 hour','r502-owner','controlled integration test grant')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO broker_account_verifications(id,external_broker_account_id,evidence_id,decision,revision,previous_revision,approval_actor_id,approval_authority,approval_reference,reason,authority_grant_id,authority_grant_revision) VALUES('r502-verification','r502-external-account','r502-evidence','VERIFIED',2,1,'r502-verifier','PLATFORM_VERIFIER','R5-02-TEST','controlled integration test approval','r502-verifier-grant',1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "UPDATE trading_account_broker_bindings SET status='VERIFIED',revision=revision+1,updated_at=NOW() WHERE id='r502-binding'",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO broker_worker_assignments(id,worker_ref,organization_id,quant_account_id,binding_id,external_broker_account_id,broker_id,broker_server_id,status,revision,valid_from,valid_until) VALUES('r502-assignment','r502-worker','r502-org','r502-quant-account','r502-binding','r502-external-account','r502-broker','r502-server','ACTIVE',1,NOW()-INTERVAL '1 minute',NOW()+INTERVAL '1 hour')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO broker_worker_sessions(id,assignment_id,worker_ref,status,revision,ready_until,started_at) VALUES('r502-session-1','r502-assignment','r502-worker','READY',1,NOW()+INTERVAL '1 minute',NOW())",
    )
    .execute(&pool)
    .await
    .unwrap();

    let decision =
        evaluate_persisted_broker_admission(&pool, &request(), "r502-owner", "r502-worker")
            .await
            .unwrap();
    assert!(matches!(
        decision,
        BrokerAdmissionDecision::Admitted(ref permit)
            if permit.binding_ref == "r502-binding"
                && permit.account_ref == "r502-external-account"
                && permit.quant_account_ref == "r502-quant-account"
                && permit.revisions.worker_assignment == 1
    ));

    // Evidence is append-only: revocation is represented by a new immutable
    // verification decision, never by mutating the evidence row.
    sqlx::query(
        "INSERT INTO broker_account_verifications(id,external_broker_account_id,evidence_id,decision,revision,previous_revision,approval_actor_id,approval_authority,approval_reference,reason) VALUES('r502-revocation','r502-external-account','r502-evidence','REVOKED',3,2,'r502-verifier','PLATFORM_VERIFIER','R5-02-REVOKE','test evidence revocation')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE external_broker_accounts SET verification_status='REVOKED',revision=revision+1,updated_at=NOW() WHERE id='r502-external-account'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let evidence_revoked =
        evaluate_persisted_broker_admission(&pool, &request(), "r502-owner", "r502-worker")
            .await
            .unwrap();
    assert!(matches!(
        evidence_revoked,
        BrokerAdmissionDecision::Denied(ref reasons)
            if reasons.contains(&BrokerAdmissionDenialReason::EvidenceRevokedOrSuperseded)
    ));

    // Return the external account to the explicit evidence-review state;
    // the verified-decision trigger only permits approval from PENDING_APPROVAL.
    sqlx::query(
        "UPDATE external_broker_accounts SET verification_status='PENDING_APPROVAL',revision=revision+1,updated_at=NOW() WHERE id='r502-external-account'",
    )
    .execute(&pool)
    .await
    .unwrap();

    // A new immutable evidence record with an already-expired validity window
    // proves expiry is evaluated against the database snapshot clock.
    sqlx::query(
        "INSERT INTO broker_account_evidence(id,external_broker_account_id,provider_type,evidence_type,observed_broker_id,observed_broker_server_id,observed_identifier_key_version,observed_identifier_normalization_version,observed_identifier_hmac,attestation_key_id,attestation_digest,observed_at,evidence_status,source_principal,expires_at) VALUES('r502-evidence-expired','r502-external-account','MT5_PYTHON','ACCOUNT_IDENTITY','r502-broker','r502-server','test-key-v1',1,$1,'test-attestation-key',$2,NOW()-INTERVAL '2 minutes','AUTHENTICATED','test-worker',NOW()-INTERVAL '1 second')",
    )
    .bind([0x5a_u8; 32].as_slice())
    .bind([0x22_u8; 32].as_slice())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO broker_account_verifications(id,external_broker_account_id,evidence_id,decision,revision,previous_revision,approval_actor_id,approval_authority,approval_reference,reason,authority_grant_id,authority_grant_revision) VALUES('r502-expired-verification','r502-external-account','r502-evidence-expired','VERIFIED',4,3,'r502-verifier','PLATFORM_VERIFIER','R5-02-EXPIRED','expired evidence test','r502-verifier-grant',1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let evidence_expired =
        evaluate_persisted_broker_admission(&pool, &request(), "r502-owner", "r502-worker")
            .await
            .unwrap();
    assert!(matches!(
        evidence_expired,
        BrokerAdmissionDecision::Denied(ref reasons)
            if reasons.contains(&BrokerAdmissionDenialReason::EvidenceExpired)
    ));

    // Entitlement suspension must remove admission without changing the
    // broker evidence or worker session.
    let store = TradingStore::new(pool.clone());
    let canonical_server = store
        .broker_server_canonical_identifier("r502-broker", "r502-server")
        .await
        .unwrap();
    assert_eq!(canonical_server.as_deref(), Some("r502.demo"));

    store
        .set_account_entitlement_status(
            "r502-org",
            "r502-quant-account",
            "r502-owner",
            1,
            1,
            EntitlementStatus::Suspended,
            "R5-02 revocation test",
        )
        .await
        .unwrap();
    let entitlement_suspended =
        evaluate_persisted_broker_admission(&pool, &request(), "r502-owner", "r502-worker")
            .await
            .unwrap();
    assert!(matches!(
        entitlement_suspended,
        BrokerAdmissionDecision::Denied(ref reasons)
            if reasons.contains(&BrokerAdmissionDenialReason::EntitlementMissingOrInactive)
    ));
    store
        .set_account_entitlement_status(
            "r502-org",
            "r502-quant-account",
            "r502-owner",
            1,
            2,
            EntitlementStatus::Active,
            "R5-02 restore test",
        )
        .await
        .unwrap();

    sqlx::query(
        "UPDATE broker_worker_sessions SET status='NOT_READY',revision=revision+1 WHERE id='r502-session-1'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let session_denied =
        evaluate_persisted_broker_admission(&pool, &request(), "r502-owner", "r502-worker")
            .await
            .unwrap();
    assert!(matches!(
        session_denied,
        BrokerAdmissionDecision::Denied(ref reasons)
            if reasons.contains(&BrokerAdmissionDenialReason::SessionNotReady)
    ));

    sqlx::query(
        "UPDATE broker_platform_authority_grants SET status='REVOKED',revision=revision+1,revoked_by='r502-owner' WHERE id='r502-verifier-grant'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let authority_denied =
        evaluate_persisted_broker_admission(&pool, &request(), "r502-owner", "r502-worker")
            .await
            .unwrap();
    assert!(matches!(
        authority_denied,
        BrokerAdmissionDecision::Denied(ref reasons)
            if reasons.contains(&BrokerAdmissionDenialReason::VerificationAuthorityMissingOrUnauthorized)
    ));

    // A closed pool represents an unavailable admission registry. The caller
    // must receive an error, never an implicit allow decision.
    pool.close().await;
    assert!(
        evaluate_persisted_broker_admission(&pool, &request(), "r502-owner", "r502-worker")
            .await
            .is_err()
    );
}
