-- QOS-021A-E3A-R3: broker identity is distinct from Quant venues/accounts.
-- No historical account, venue, owner string, or MT5 snapshot is backfilled.

CREATE TABLE brokers (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9][a-z0-9._-]{0,127}$'),
    canonical_name TEXT NOT NULL UNIQUE CHECK (canonical_name ~ '^[a-z0-9][a-z0-9._-]{0,127}$'),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 160),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','DISABLED')),
    normalization_version SMALLINT NOT NULL CHECK (normalization_version > 0),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX brokers_status_idx ON brokers(status,id);

CREATE TABLE broker_servers (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) BETWEEN 1 AND 128),
    broker_id TEXT NOT NULL REFERENCES brokers(id) ON DELETE RESTRICT,
    canonical_identifier TEXT NOT NULL CHECK (canonical_identifier ~ '^[a-z0-9][a-z0-9._-]{0,127}$'),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 160),
    provider_type TEXT NOT NULL CHECK (provider_type IN ('MT5_PYTHON','OTHER')),
    environment TEXT NOT NULL CHECK (environment IN ('DEMO','LIVE','UNKNOWN')),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','DISABLED','UNVERIFIED')),
    normalization_version SMALLINT NOT NULL CHECK (normalization_version > 0),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    source_reference TEXT NOT NULL CHECK (length(trim(source_reference)) BETWEEN 1 AND 256),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (broker_id,canonical_identifier,normalization_version),
    UNIQUE (id,broker_id)
);
CREATE INDEX broker_servers_broker_status_idx ON broker_servers(broker_id,status);

CREATE TABLE external_broker_accounts (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) BETWEEN 1 AND 128),
    broker_id TEXT NOT NULL REFERENCES brokers(id) ON DELETE RESTRICT,
    broker_server_id TEXT NOT NULL,
    identifier_normalization_version SMALLINT NOT NULL CHECK (identifier_normalization_version > 0),
    identifier_key_version TEXT NOT NULL CHECK (length(trim(identifier_key_version)) BETWEEN 1 AND 64),
    identifier_lookup_hmac BYTEA NOT NULL CHECK (octet_length(identifier_lookup_hmac) = 32),
    account_category TEXT CHECK (account_category IS NULL OR length(account_category) <= 80),
    base_currency CHAR(3) CHECK (base_currency IS NULL OR base_currency ~ '^[A-Z]{3}$'),
    status TEXT NOT NULL DEFAULT 'PENDING' CHECK (status IN ('PENDING','ACTIVE','SUSPENDED','REVOKED')),
    verification_status TEXT NOT NULL DEFAULT 'PENDING_EVIDENCE' CHECK (verification_status IN ('PENDING_EVIDENCE','PENDING_APPROVAL','VERIFIED','REJECTED','SUSPENDED','REVOKED')),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (broker_server_id,broker_id) REFERENCES broker_servers(id,broker_id) ON DELETE RESTRICT,
    UNIQUE (broker_server_id,identifier_key_version,identifier_normalization_version,identifier_lookup_hmac)
);
CREATE INDEX external_broker_accounts_broker_idx ON external_broker_accounts(broker_id,broker_server_id,status);

-- Each retained key version has its own non-reversible lookup token. During
-- rotation the API must check all retained versions and write every configured
-- token, preventing duplicate identities across an overlapping key rotation.
CREATE TABLE external_broker_account_identifier_tokens (
    external_broker_account_id TEXT NOT NULL REFERENCES external_broker_accounts(id) ON DELETE RESTRICT,
    broker_server_id TEXT NOT NULL REFERENCES broker_servers(id) ON DELETE RESTRICT,
    key_version TEXT NOT NULL CHECK (length(trim(key_version)) BETWEEN 1 AND 64),
    normalization_version SMALLINT NOT NULL CHECK (normalization_version > 0),
    lookup_hmac BYTEA NOT NULL CHECK (octet_length(lookup_hmac)=32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (external_broker_account_id,key_version,normalization_version),
    UNIQUE (broker_server_id,key_version,normalization_version,lookup_hmac)
);
CREATE INDEX external_broker_account_tokens_lookup_idx
    ON external_broker_account_identifier_tokens(broker_server_id,key_version,normalization_version,lookup_hmac);

-- R2 entitlement remains the tenant authority. The redundant unique key
-- permits a composite FK and prevents a binding from crossing organizations.
-- The organization/account pair is already unique through the primary key
-- on account_id; this explicit composite key is retained for workspace FKs.
ALTER TABLE organization_account_entitlements
    ADD CONSTRAINT organization_entitlements_org_account_unique UNIQUE (organization_id,account_id);

CREATE TABLE trading_account_broker_bindings (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) BETWEEN 1 AND 128),
    organization_id TEXT NOT NULL,
    quant_account_id TEXT NOT NULL,
    external_broker_account_id TEXT NOT NULL REFERENCES external_broker_accounts(id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'PENDING_EVIDENCE' CHECK (status IN ('PENDING_EVIDENCE','PENDING_APPROVAL','VERIFIED','SUSPENDED','REVOKED')),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    reassignment_reason TEXT NOT NULL CHECK (length(trim(reassignment_reason)) BETWEEN 1 AND 1000),
    approved_by TEXT REFERENCES identity_users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (organization_id,quant_account_id)
        REFERENCES organization_account_entitlements(organization_id,account_id) ON DELETE RESTRICT
);
-- A pending/suspended binding reserves both identities; only explicit revoke
-- releases them. This prevents parallel pending claims and silent reassignment.
CREATE UNIQUE INDEX broker_binding_external_reserved_uq
    ON trading_account_broker_bindings(external_broker_account_id)
    WHERE status IN ('PENDING_EVIDENCE','PENDING_APPROVAL','VERIFIED','SUSPENDED');
CREATE UNIQUE INDEX broker_binding_quant_reserved_uq
    ON trading_account_broker_bindings(quant_account_id)
    WHERE status IN ('PENDING_EVIDENCE','PENDING_APPROVAL','VERIFIED','SUSPENDED');
CREATE INDEX broker_binding_org_status_idx
    ON trading_account_broker_bindings(organization_id,status,created_at DESC);

CREATE TABLE broker_account_evidence (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) BETWEEN 1 AND 128),
    external_broker_account_id TEXT NOT NULL REFERENCES external_broker_accounts(id) ON DELETE RESTRICT,
    provider_type TEXT NOT NULL CHECK (provider_type IN ('MT5_PYTHON','OTHER')),
    evidence_type TEXT NOT NULL CHECK (length(trim(evidence_type)) BETWEEN 1 AND 80),
    observed_broker_id TEXT NOT NULL REFERENCES brokers(id) ON DELETE RESTRICT,
    observed_broker_server_id TEXT NOT NULL,
    observed_identifier_key_version TEXT NOT NULL,
    observed_identifier_normalization_version SMALLINT NOT NULL CHECK (observed_identifier_normalization_version > 0),
    observed_identifier_hmac BYTEA NOT NULL CHECK (octet_length(observed_identifier_hmac) = 32),
    attestation_key_id TEXT NOT NULL CHECK (length(trim(attestation_key_id)) BETWEEN 1 AND 128),
    attestation_digest BYTEA NOT NULL CHECK (octet_length(attestation_digest) = 32),
    observed_at TIMESTAMPTZ NOT NULL,
    evidence_status TEXT NOT NULL CHECK (evidence_status IN ('RECEIVED','AUTHENTICATED','REJECTED')),
    source_principal TEXT NOT NULL CHECK (length(trim(source_principal)) BETWEEN 1 AND 256),
    recorded_by TEXT REFERENCES identity_users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (observed_broker_server_id,observed_broker_id)
        REFERENCES broker_servers(id,broker_id) ON DELETE RESTRICT
);
CREATE INDEX broker_account_evidence_account_time_idx
    ON broker_account_evidence(external_broker_account_id,created_at DESC);

CREATE TABLE broker_account_verifications (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) BETWEEN 1 AND 128),
    external_broker_account_id TEXT NOT NULL REFERENCES external_broker_accounts(id) ON DELETE RESTRICT,
    evidence_id TEXT REFERENCES broker_account_evidence(id) ON DELETE RESTRICT,
    decision TEXT NOT NULL CHECK (decision IN ('PENDING_EVIDENCE','PENDING_APPROVAL','VERIFIED','REJECTED','SUSPENDED','REVOKED')),
    revision BIGINT NOT NULL CHECK (revision > 0),
    previous_revision BIGINT,
    approval_actor_id TEXT REFERENCES identity_users(id) ON DELETE RESTRICT,
    approval_authority TEXT,
    approval_reference TEXT,
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    decided_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK ((decision <> 'VERIFIED') OR
        (evidence_id IS NOT NULL AND approval_actor_id IS NOT NULL AND
         length(trim(COALESCE(approval_authority,''))) > 0 AND
         length(trim(COALESCE(approval_reference,''))) > 0)),
    UNIQUE (external_broker_account_id,revision)
);
CREATE INDEX broker_account_verifications_history_idx
    ON broker_account_verifications(external_broker_account_id,revision DESC);

CREATE TABLE trading_account_broker_binding_events (
    id BIGSERIAL PRIMARY KEY,
    binding_id TEXT NOT NULL REFERENCES trading_account_broker_bindings(id) ON DELETE RESTRICT,
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    quant_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    external_broker_account_id TEXT NOT NULL REFERENCES external_broker_accounts(id) ON DELETE RESTRICT,
    actor_user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    revision BIGINT NOT NULL CHECK (revision > 0),
    previous_status TEXT,
    next_status TEXT NOT NULL CHECK (next_status IN ('PENDING_EVIDENCE','PENDING_APPROVAL','VERIFIED','SUSPENDED','REVOKED')),
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (binding_id,revision)
);
CREATE INDEX broker_binding_events_history_idx
    ON trading_account_broker_binding_events(binding_id,created_at DESC);

CREATE FUNCTION protect_broker_registry_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_TABLE_NAME = 'brokers' AND
       (to_jsonb(NEW) - ARRAY['display_name','status','revision','created_at','updated_at'])
       IS DISTINCT FROM (to_jsonb(OLD) - ARRAY['display_name','status','revision','created_at','updated_at']) THEN
        RAISE EXCEPTION 'broker canonical identity is immutable';
    ELSIF TG_TABLE_NAME = 'broker_servers' AND
       (to_jsonb(NEW) - ARRAY['display_name','environment','status','revision','source_reference','created_at','updated_at'])
       IS DISTINCT FROM (to_jsonb(OLD) - ARRAY['display_name','environment','status','revision','source_reference','created_at','updated_at']) THEN
        RAISE EXCEPTION 'broker server canonical identity is immutable';
    ELSIF TG_TABLE_NAME = 'external_broker_accounts' AND
       (to_jsonb(NEW) - ARRAY['account_category','base_currency','status','verification_status','revision','created_at','updated_at'])
       IS DISTINCT FROM (to_jsonb(OLD) - ARRAY['account_category','base_currency','status','verification_status','revision','created_at','updated_at']) THEN
        RAISE EXCEPTION 'external broker account identity is immutable';
    ELSIF TG_TABLE_NAME = 'trading_account_broker_bindings' AND
       (to_jsonb(NEW) - ARRAY['status','revision','reassignment_reason','approved_by','created_at','updated_at'])
       IS DISTINCT FROM (to_jsonb(OLD) - ARRAY['status','revision','reassignment_reason','approved_by','created_at','updated_at']) THEN
        RAISE EXCEPTION 'broker binding identity is immutable; create a new audited binding';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER broker_identity_immutable
    BEFORE UPDATE ON brokers FOR EACH ROW EXECUTE FUNCTION protect_broker_registry_identity();
CREATE TRIGGER broker_server_identity_immutable
    BEFORE UPDATE ON broker_servers FOR EACH ROW EXECUTE FUNCTION protect_broker_registry_identity();
CREATE TRIGGER external_broker_account_identity_immutable
    BEFORE UPDATE ON external_broker_accounts FOR EACH ROW EXECUTE FUNCTION protect_broker_registry_identity();
CREATE TRIGGER broker_binding_identity_immutable
    BEFORE UPDATE ON trading_account_broker_bindings FOR EACH ROW EXECUTE FUNCTION protect_broker_registry_identity();

CREATE FUNCTION enforce_broker_binding_entitlement() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    entitlement_status TEXT;
BEGIN
    IF NEW.status IN ('PENDING_EVIDENCE','PENDING_APPROVAL','VERIFIED') THEN
        SELECT status INTO entitlement_status
        FROM organization_account_entitlements
        WHERE organization_id=NEW.organization_id AND account_id=NEW.quant_account_id
        FOR UPDATE;
        IF entitlement_status IS DISTINCT FROM 'ACTIVE' THEN
            RAISE EXCEPTION 'broker binding requires an active organization account entitlement';
        END IF;
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER broker_binding_requires_active_entitlement
    BEFORE INSERT OR UPDATE OF organization_id,quant_account_id,status
    ON trading_account_broker_bindings
    FOR EACH ROW EXECUTE FUNCTION enforce_broker_binding_entitlement();

CREATE FUNCTION reject_broker_registry_history_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'broker identity evidence and history are append-only';
END
$$;
CREATE TRIGGER broker_account_evidence_no_update_delete
    BEFORE UPDATE OR DELETE ON broker_account_evidence
    FOR EACH ROW EXECUTE FUNCTION reject_broker_registry_history_mutation();
CREATE TRIGGER external_broker_account_tokens_no_update_delete
    BEFORE UPDATE OR DELETE ON external_broker_account_identifier_tokens
    FOR EACH ROW EXECUTE FUNCTION reject_broker_registry_history_mutation();

CREATE FUNCTION validate_broker_evidence_identity() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    identity_matches BOOLEAN;
BEGIN
    SELECT (e.broker_id=NEW.observed_broker_id AND
            e.broker_server_id=NEW.observed_broker_server_id AND
            EXISTS (SELECT 1 FROM external_broker_account_identifier_tokens t
                     WHERE t.external_broker_account_id=e.id
                       AND t.broker_server_id=e.broker_server_id
                       AND t.key_version=NEW.observed_identifier_key_version
                       AND t.normalization_version=NEW.observed_identifier_normalization_version
                       AND t.lookup_hmac=NEW.observed_identifier_hmac) AND
            (SELECT status='ACTIVE' FROM brokers WHERE id=e.broker_id) AND
            (SELECT status='ACTIVE' FROM broker_servers WHERE id=e.broker_server_id))
      INTO identity_matches
      FROM external_broker_accounts e
     WHERE e.id=NEW.external_broker_account_id;
    IF identity_matches IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION 'provider evidence identity does not match registered external account';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER broker_evidence_identity_match
    BEFORE INSERT ON broker_account_evidence
    FOR EACH ROW EXECUTE FUNCTION validate_broker_evidence_identity();

CREATE FUNCTION validate_broker_account_token_server() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM external_broker_accounts e
        WHERE e.id=NEW.external_broker_account_id AND e.broker_server_id=NEW.broker_server_id
    ) THEN
        RAISE EXCEPTION 'broker account lookup token server does not match account identity';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER broker_account_token_server_match
    BEFORE INSERT ON external_broker_account_identifier_tokens
    FOR EACH ROW EXECUTE FUNCTION validate_broker_account_token_server();

CREATE FUNCTION validate_broker_verification_decision() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    evidence_matches BOOLEAN;
BEGIN
    IF NEW.decision='VERIFIED' THEN
        SELECT (e.evidence_status='AUTHENTICATED' AND
                e.external_broker_account_id=NEW.external_broker_account_id)
          INTO evidence_matches
          FROM broker_account_evidence e
         WHERE e.id=NEW.evidence_id;
        IF evidence_matches IS DISTINCT FROM TRUE THEN
            RAISE EXCEPTION 'verification requires authenticated provider evidence for this account';
        END IF;
        UPDATE external_broker_accounts
           SET status='ACTIVE',verification_status='VERIFIED',revision=revision+1,updated_at=NOW()
         WHERE id=NEW.external_broker_account_id
           AND verification_status='PENDING_APPROVAL';
        IF NOT FOUND THEN
            RAISE EXCEPTION 'external account is not pending approval';
        END IF;
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER broker_verification_requires_evidence
    BEFORE INSERT ON broker_account_verifications
    FOR EACH ROW EXECUTE FUNCTION validate_broker_verification_decision();
CREATE TRIGGER broker_account_verifications_no_update_delete
    BEFORE UPDATE OR DELETE ON broker_account_verifications
    FOR EACH ROW EXECUTE FUNCTION reject_broker_registry_history_mutation();
CREATE TRIGGER broker_binding_events_no_update_delete
    BEFORE UPDATE OR DELETE ON trading_account_broker_binding_events
    FOR EACH ROW EXECUTE FUNCTION reject_broker_registry_history_mutation();

COMMENT ON TABLE brokers IS 'Trusted broker catalogue; no user-facing writes until a platform administration authority exists.';
COMMENT ON TABLE broker_servers IS 'Broker-scoped normalized provider server identities; v1 normalization rules are versioned.';
COMMENT ON TABLE external_broker_accounts IS 'External identity lookup uses keyed HMAC only; raw account identifiers and credentials are never stored.';
COMMENT ON TABLE broker_account_evidence IS 'Append-only provider evidence; no terminal heartbeat alone constitutes ownership proof.';
COMMENT ON TABLE broker_account_verifications IS 'Separate approval decision history; VERIFIED requires both evidence and explicit authority references.';
