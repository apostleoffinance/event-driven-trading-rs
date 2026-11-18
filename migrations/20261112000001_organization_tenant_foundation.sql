-- QOS-021A-E3A-R2: explicit organization membership and Quant-account tenancy.
-- Existing users, dashboard grants and accounts are intentionally not backfilled.

CREATE TABLE organizations (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) BETWEEN 1 AND 128),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','SUSPENDED','CLOSED')),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE organization_events (
    id BIGSERIAL PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    actor_user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    revision BIGINT NOT NULL CHECK (revision > 0),
    previous_name TEXT,
    next_name TEXT NOT NULL,
    previous_status TEXT CHECK (previous_status IS NULL OR previous_status IN ('ACTIVE','SUSPENDED','CLOSED')),
    next_status TEXT NOT NULL CHECK (next_status IN ('ACTIVE','SUSPENDED','CLOSED')),
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (organization_id,revision)
);
CREATE INDEX organization_events_history_idx ON organization_events(organization_id,created_at DESC);

CREATE TABLE organization_memberships (
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    role TEXT NOT NULL CHECK (role IN ('OWNER','ADMIN','TRADER','RISK_MANAGER','VIEWER')),
    status TEXT NOT NULL CHECK (status IN ('ACTIVE','SUSPENDED','REVOKED')),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    updated_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (organization_id,user_id)
);
CREATE INDEX organization_memberships_user_active_idx
    ON organization_memberships(user_id,organization_id) WHERE status='ACTIVE';
CREATE INDEX organization_memberships_org_role_idx
    ON organization_memberships(organization_id,role,status);

CREATE TABLE organization_membership_events (
    id BIGSERIAL PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    actor_user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    revision BIGINT NOT NULL CHECK (revision > 0),
    previous_role TEXT CHECK (previous_role IS NULL OR previous_role IN ('OWNER','ADMIN','TRADER','RISK_MANAGER','VIEWER')),
    next_role TEXT NOT NULL CHECK (next_role IN ('OWNER','ADMIN','TRADER','RISK_MANAGER','VIEWER')),
    previous_status TEXT CHECK (previous_status IS NULL OR previous_status IN ('ACTIVE','SUSPENDED','REVOKED')),
    next_status TEXT NOT NULL CHECK (next_status IN ('ACTIVE','SUSPENDED','REVOKED')),
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (organization_id,user_id,revision)
);
CREATE INDEX organization_membership_events_history_idx
    ON organization_membership_events(organization_id,user_id,created_at DESC);

ALTER TABLE accounts
    ADD COLUMN account_revision BIGINT NOT NULL DEFAULT 1 CHECK (account_revision > 0);

CREATE FUNCTION increment_account_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF ROW(NEW.account_type,NEW.status,NEW.venue_id,NEW.starting_capital,NEW.label,
           NEW.environment,NEW.environment_verification,NEW.environment_verified_at,
           NEW.environment_verified_by,NEW.environment_verification_reason)
       IS DISTINCT FROM
       ROW(OLD.account_type,OLD.status,OLD.venue_id,OLD.starting_capital,OLD.label,
           OLD.environment,OLD.environment_verification,OLD.environment_verified_at,
           OLD.environment_verified_by,OLD.environment_verification_reason) THEN
        NEW.account_revision := OLD.account_revision + 1;
    ELSE
        NEW.account_revision := OLD.account_revision;
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER accounts_revision_monotonic
    BEFORE UPDATE ON accounts FOR EACH ROW EXECUTE FUNCTION increment_account_revision();

CREATE TABLE organization_account_entitlements (
    account_id TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE RESTRICT,
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    status TEXT NOT NULL CHECK (status IN ('ACTIVE','SUSPENDED','REVOKED')),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    assigned_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    updated_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX organization_account_entitlements_active_idx
    ON organization_account_entitlements(organization_id,account_id) WHERE status='ACTIVE';
CREATE INDEX organization_account_entitlements_history_idx
    ON organization_account_entitlements(account_id,revision DESC);

CREATE TABLE organization_account_entitlement_events (
    id BIGSERIAL PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    actor_user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    previous_organization_id TEXT REFERENCES organizations(id) ON DELETE RESTRICT,
    next_organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    previous_status TEXT CHECK (previous_status IS NULL OR previous_status IN ('ACTIVE','SUSPENDED','REVOKED')),
    next_status TEXT NOT NULL CHECK (next_status IN ('ACTIVE','SUSPENDED','REVOKED')),
    account_revision BIGINT NOT NULL CHECK (account_revision > 0),
    entitlement_revision BIGINT NOT NULL CHECK (entitlement_revision > 0),
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (account_id,entitlement_revision)
);
CREATE INDEX organization_account_entitlement_events_history_idx
    ON organization_account_entitlement_events(account_id,created_at DESC);

CREATE FUNCTION protect_last_organization_owner() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    remaining_owners BIGINT;
BEGIN
    IF OLD.role='OWNER' AND OLD.status='ACTIVE'
       AND (NEW.role <> 'OWNER' OR NEW.status <> 'ACTIVE') THEN
        -- Serialize every ownership-affecting membership update per organization.
        PERFORM 1 FROM organizations WHERE id=OLD.organization_id FOR UPDATE;
        SELECT count(*) INTO remaining_owners
        FROM organization_memberships
        WHERE organization_id=OLD.organization_id AND user_id<>OLD.user_id
          AND role='OWNER' AND status='ACTIVE';
        IF remaining_owners = 0 THEN
            RAISE EXCEPTION 'organization must retain at least one active OWNER'
                USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER organization_memberships_last_owner
    BEFORE UPDATE ON organization_memberships
    FOR EACH ROW EXECUTE FUNCTION protect_last_organization_owner();

CREATE FUNCTION prevent_tenant_history_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'organization history records are append-only'
        USING ERRCODE='23514';
END
$$;
CREATE TRIGGER organization_membership_events_no_delete
    BEFORE UPDATE OR DELETE ON organization_membership_events
    FOR EACH ROW EXECUTE FUNCTION prevent_tenant_history_change();
CREATE TRIGGER organization_account_entitlement_events_no_delete
    BEFORE UPDATE OR DELETE ON organization_account_entitlement_events
    FOR EACH ROW EXECUTE FUNCTION prevent_tenant_history_change();
CREATE TRIGGER organization_events_no_delete
    BEFORE UPDATE OR DELETE ON organization_events
    FOR EACH ROW EXECUTE FUNCTION prevent_tenant_history_change();
CREATE TRIGGER organization_memberships_no_delete
    BEFORE DELETE ON organization_memberships
    FOR EACH ROW EXECUTE FUNCTION prevent_tenant_history_change();
CREATE TRIGGER organization_account_entitlements_no_delete
    BEFORE DELETE ON organization_account_entitlements
    FOR EACH ROW EXECUTE FUNCTION prevent_tenant_history_change();
