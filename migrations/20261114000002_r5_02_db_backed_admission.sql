-- R5-02-A2: persisted, read-only broker admission facts.
-- Existing registrations/evidence are not promoted. Historical evidence without
-- explicit expiry and authority binding remains ineligible for admission.

ALTER TABLE broker_account_evidence
    ADD COLUMN expires_at TIMESTAMPTZ,
    ADD COLUMN revoked_at TIMESTAMPTZ;

ALTER TABLE broker_account_verifications
    ADD COLUMN authority_grant_id TEXT,
    ADD COLUMN authority_grant_revision BIGINT;

CREATE TABLE broker_platform_authority_grants (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) BETWEEN 1 AND 128),
    user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    permission TEXT NOT NULL CHECK (permission IN ('BROKER_ACCOUNT_VERIFY')),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','REVOKED','EXPIRED')),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    valid_from TIMESTAMPTZ NOT NULL,
    valid_until TIMESTAMPTZ,
    granted_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    revoked_by TEXT REFERENCES identity_users(id) ON DELETE RESTRICT,
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (valid_until IS NULL OR valid_until > valid_from),
    CHECK ((status = 'REVOKED') = (revoked_by IS NOT NULL))
);
CREATE UNIQUE INDEX broker_platform_authority_active_user_permission_uq
    ON broker_platform_authority_grants(user_id,permission)
    WHERE status = 'ACTIVE';
CREATE INDEX broker_platform_authority_validity_idx
    ON broker_platform_authority_grants(user_id,status,valid_from,valid_until);

CREATE TABLE broker_worker_assignments (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) BETWEEN 1 AND 128),
    worker_ref TEXT NOT NULL CHECK (length(trim(worker_ref)) BETWEEN 1 AND 128),
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    quant_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    binding_id TEXT NOT NULL REFERENCES trading_account_broker_bindings(id) ON DELETE RESTRICT,
    external_broker_account_id TEXT NOT NULL REFERENCES external_broker_accounts(id) ON DELETE RESTRICT,
    broker_id TEXT NOT NULL REFERENCES brokers(id) ON DELETE RESTRICT,
    broker_server_id TEXT NOT NULL REFERENCES broker_servers(id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','SUSPENDED','REVOKED')),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    valid_from TIMESTAMPTZ NOT NULL,
    valid_until TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (valid_until > valid_from)
);
CREATE UNIQUE INDEX broker_worker_assignment_active_binding_uq
    ON broker_worker_assignments(binding_id)
    WHERE status = 'ACTIVE';
CREATE INDEX broker_worker_assignment_scope_idx
    ON broker_worker_assignments(organization_id,quant_account_id,worker_ref,status);

CREATE TABLE broker_worker_sessions (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) BETWEEN 1 AND 128),
    assignment_id TEXT NOT NULL REFERENCES broker_worker_assignments(id) ON DELETE RESTRICT,
    worker_ref TEXT NOT NULL CHECK (length(trim(worker_ref)) BETWEEN 1 AND 128),
    status TEXT NOT NULL CHECK (status IN ('READY','NOT_READY','REVOKED')),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    ready_until TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX broker_worker_session_one_ready_uq
    ON broker_worker_sessions(assignment_id)
    WHERE status = 'READY';
CREATE INDEX broker_worker_session_current_idx
    ON broker_worker_sessions(assignment_id,worker_ref,status,ready_until DESC);

CREATE FUNCTION reject_r5_02_admission_history_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'R5-02 admission authority and worker session history is append-only';
END
$$;

-- Grant changes are represented by new grant rows (revoke then issue a new ID).
CREATE TRIGGER broker_platform_authority_grants_no_update_delete
    BEFORE UPDATE OR DELETE ON broker_platform_authority_grants
    FOR EACH ROW EXECUTE FUNCTION reject_r5_02_admission_history_mutation();
CREATE TRIGGER broker_worker_sessions_no_update_delete
    BEFORE UPDATE OR DELETE ON broker_worker_sessions
    FOR EACH ROW EXECUTE FUNCTION reject_r5_02_admission_history_mutation();

CREATE FUNCTION validate_r5_02_verified_decision_authority() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    authority_valid BOOLEAN;
BEGIN
    IF NEW.decision = 'VERIFIED' THEN
        SELECT (
            g.user_id = NEW.approval_actor_id
            AND g.permission = 'BROKER_ACCOUNT_VERIFY'
            AND g.status = 'ACTIVE'
            AND g.revision = NEW.authority_grant_revision
            AND g.valid_from <= NOW()
            AND (g.valid_until IS NULL OR g.valid_until > NOW())
            AND u.is_active
        ) INTO authority_valid
        FROM broker_platform_authority_grants g
        JOIN identity_users u ON u.id = g.user_id
        WHERE g.id = NEW.authority_grant_id;

        IF authority_valid IS DISTINCT FROM TRUE THEN
            RAISE EXCEPTION 'VERIFIED broker decision requires a current platform authority grant';
        END IF;
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER broker_verification_requires_platform_authority
    BEFORE INSERT ON broker_account_verifications
    FOR EACH ROW EXECUTE FUNCTION validate_r5_02_verified_decision_authority();

COMMENT ON TABLE broker_platform_authority_grants IS
    'Explicit platform verifier authority. Tenant OWNER/ADMIN membership is not platform verification authority.';
COMMENT ON TABLE broker_worker_assignments IS
    'Trusted, scoped assignment facts provisioned by an authorized control plane; worker_ref must be authenticated by the caller.';
COMMENT ON TABLE broker_worker_sessions IS
    'Append-only worker readiness assertions. A current READY row must have ready_until in the future.';
COMMENT ON COLUMN broker_account_evidence.expires_at IS
    'Required for R5-02 admission. NULL legacy evidence fails closed.';
COMMENT ON COLUMN broker_account_verifications.authority_grant_id IS
    'Platform authority grant used for the verification decision; NULL legacy decisions fail closed.';
