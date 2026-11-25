-- M1: first-class organization workspaces and Quant-account placement.
-- Existing organizations receive one deterministic default workspace. Existing
-- organization entitlements are preserved and mirrored into a single workspace
-- assignment; user-private Journal data and global broker catalogues are untouched.

CREATE TABLE workspaces (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) BETWEEN 1 AND 128),
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
    slug TEXT NOT NULL CHECK (slug ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','ARCHIVED')),
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, organization_id),
    UNIQUE (organization_id, slug),
    CHECK (NOT is_default OR status = 'ACTIVE')
);

CREATE UNIQUE INDEX workspaces_one_active_default_per_org_uq
    ON workspaces(organization_id) WHERE is_default AND status='ACTIVE';
CREATE INDEX workspaces_org_status_idx
    ON workspaces(organization_id,status,created_at,id);

CREATE TABLE workspace_events (
    id BIGSERIAL PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    actor_user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    revision BIGINT NOT NULL CHECK (revision > 0),
    event_type TEXT NOT NULL CHECK (event_type IN ('CREATED','RENAMED','ARCHIVED','DEFAULT_CHANGED')),
    previous_name TEXT,
    next_name TEXT NOT NULL,
    previous_status TEXT CHECK (previous_status IS NULL OR previous_status IN ('ACTIVE','ARCHIVED')),
    next_status TEXT NOT NULL CHECK (next_status IN ('ACTIVE','ARCHIVED')),
    previous_is_default BOOLEAN,
    next_is_default BOOLEAN NOT NULL,
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id,revision),
    FOREIGN KEY (workspace_id,organization_id)
        REFERENCES workspaces(id,organization_id) ON DELETE RESTRICT
);
CREATE INDEX workspace_events_history_idx
    ON workspace_events(workspace_id,created_at DESC);

-- Organization entitlement remains the legal/commercial account authority.
-- This child assignment gives each Quant account one operational workspace.
CREATE TABLE workspace_account_assignments (
    account_id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    assigned_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (organization_id,account_id),
    FOREIGN KEY (organization_id,account_id)
        REFERENCES organization_account_entitlements(organization_id,account_id) ON DELETE RESTRICT,
    FOREIGN KEY (workspace_id,organization_id)
        REFERENCES workspaces(id,organization_id) ON DELETE RESTRICT
);
CREATE INDEX workspace_account_assignments_workspace_idx
    ON workspace_account_assignments(organization_id,workspace_id,account_id);

-- Deterministic identifiers make repair/retry safe before SQLx records success.
INSERT INTO workspaces (
    id, organization_id, name, slug, status, is_default, revision,
    created_by, created_at, updated_at
)
SELECT
    'ws-default-' || md5(o.id), o.id, 'Default', 'default', 'ACTIVE', TRUE,
    1, o.created_by, o.created_at, o.updated_at
FROM organizations o
WHERE NOT EXISTS (
    SELECT 1 FROM workspaces w
    WHERE w.organization_id=o.id AND w.is_default AND w.status='ACTIVE'
)
ON CONFLICT (organization_id,slug) DO NOTHING;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM organizations o
        LEFT JOIN workspaces w
          ON w.organization_id=o.id AND w.is_default AND w.status='ACTIVE'
        GROUP BY o.id
        HAVING count(w.id) <> 1
    ) THEN
        RAISE EXCEPTION 'workspace backfill failed: every organization must have exactly one active default workspace';
    END IF;
END
$$;

INSERT INTO workspace_events (
    workspace_id, organization_id, actor_user_id, revision, event_type,
    previous_name, next_name, previous_status, next_status,
    previous_is_default, next_is_default, reason
)
SELECT
    w.id, w.organization_id, w.created_by, w.revision, 'CREATED',
    NULL, w.name, NULL, w.status, NULL, w.is_default,
    'M1 backfill: provisioned the organization default workspace'
FROM workspaces w
WHERE w.slug='default' AND w.is_default=TRUE
  AND NOT EXISTS (
      SELECT 1 FROM workspace_events e
      WHERE e.workspace_id=w.id AND e.revision=w.revision
  );

-- Preserve every entitlement, including suspended/revoked rows. Effective
-- trading access continues to depend on the existing entitlement status.
INSERT INTO workspace_account_assignments (
    account_id, organization_id, workspace_id, revision, assigned_by, reason,
    created_at, updated_at
)
SELECT
    e.account_id, e.organization_id, w.id, 1, e.assigned_by,
    'M1 backfill: placed existing organization-entitled Quant account in the organization default workspace',
    e.created_at, e.updated_at
FROM organization_account_entitlements e
JOIN workspaces w
  ON w.organization_id=e.organization_id AND w.is_default AND w.status='ACTIVE'
ON CONFLICT (account_id) DO NOTHING;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM organization_account_entitlements e
        LEFT JOIN workspace_account_assignments a
          ON a.organization_id=e.organization_id AND a.account_id=e.account_id
        WHERE a.account_id IS NULL
    ) THEN
        RAISE EXCEPTION 'workspace account backfill failed: an organization entitlement has no workspace assignment';
    END IF;
END
$$;

CREATE TABLE workspace_account_assignment_events (
    id BIGSERIAL PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES workspace_account_assignments(account_id) ON DELETE RESTRICT,
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    workspace_id TEXT NOT NULL,
    actor_user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    revision BIGINT NOT NULL CHECK (revision > 0),
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (workspace_id,organization_id)
        REFERENCES workspaces(id,organization_id) ON DELETE RESTRICT,
    UNIQUE (account_id,revision)
);
CREATE INDEX workspace_account_assignment_events_history_idx
    ON workspace_account_assignment_events(account_id,created_at DESC);

INSERT INTO workspace_account_assignment_events (
    account_id,organization_id,workspace_id,actor_user_id,revision,reason,created_at
)
SELECT
    a.account_id,a.organization_id,a.workspace_id,a.assigned_by,a.revision,a.reason,a.created_at
FROM workspace_account_assignments a;

CREATE FUNCTION protect_workspace_account_assignment()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        RAISE EXCEPTION 'workspace account assignments cannot be deleted'
            USING ERRCODE='23514';
    END IF;
    IF NEW.account_id <> OLD.account_id OR NEW.organization_id <> OLD.organization_id THEN
        RAISE EXCEPTION 'workspace account assignment ownership is immutable'
            USING ERRCODE='23514';
    END IF;
    IF NEW.revision <> OLD.revision + 1 THEN
        RAISE EXCEPTION 'workspace account reassignment requires the next revision'
            USING ERRCODE='23514';
    END IF;
    NEW.updated_at := NOW();
    RETURN NEW;
END
$$;
CREATE TRIGGER workspace_account_assignment_revision_guard
    BEFORE UPDATE OR DELETE ON workspace_account_assignments
    FOR EACH ROW EXECUTE FUNCTION protect_workspace_account_assignment();

CREATE FUNCTION assert_organization_default_workspace(p_organization_id TEXT)
RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE
    defaults BIGINT;
BEGIN
    SELECT count(*) INTO defaults
      FROM workspaces
     WHERE organization_id=p_organization_id
       AND is_default=TRUE AND status='ACTIVE';
    IF defaults <> 1 THEN
        RAISE EXCEPTION 'organization % must have exactly one active default workspace', p_organization_id
            USING ERRCODE='23514';
    END IF;
END
$$;

CREATE FUNCTION check_workspace_default_invariant()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_TABLE_NAME='organizations' THEN
        PERFORM assert_organization_default_workspace(NEW.id);
    ELSIF TG_OP='DELETE' THEN
        PERFORM assert_organization_default_workspace(OLD.organization_id);
    ELSE
        PERFORM assert_organization_default_workspace(NEW.organization_id);
        IF TG_OP='UPDATE' AND OLD.organization_id <> NEW.organization_id THEN
            PERFORM assert_organization_default_workspace(OLD.organization_id);
        END IF;
    END IF;
    RETURN NULL;
END
$$;

-- Deferred checks permit M2 to insert organization + default workspace atomically.
CREATE CONSTRAINT TRIGGER organizations_require_default_workspace
    AFTER INSERT ON organizations
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION check_workspace_default_invariant();
CREATE CONSTRAINT TRIGGER workspaces_require_default_workspace
    AFTER INSERT OR UPDATE OR DELETE ON workspaces
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION check_workspace_default_invariant();

CREATE FUNCTION protect_workspace_identity_and_history()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        RAISE EXCEPTION 'workspaces cannot be deleted; archive them instead'
            USING ERRCODE='23514';
    END IF;
    IF NEW.id <> OLD.id OR NEW.organization_id <> OLD.organization_id
       OR NEW.created_by <> OLD.created_by OR NEW.created_at <> OLD.created_at THEN
        RAISE EXCEPTION 'workspace identity and ownership are immutable'
            USING ERRCODE='23514';
    END IF;
    IF NEW.revision <> OLD.revision + 1 THEN
        RAISE EXCEPTION 'workspace updates require the next revision'
            USING ERRCODE='23514';
    END IF;
    NEW.updated_at := NOW();
    RETURN NEW;
END
$$;
CREATE TRIGGER workspaces_identity_revision_guard
    BEFORE UPDATE OR DELETE ON workspaces
    FOR EACH ROW EXECUTE FUNCTION protect_workspace_identity_and_history();

CREATE FUNCTION prevent_workspace_history_change()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'workspace and workspace assignment history records are append-only'
        USING ERRCODE='23514';
END
$$;
CREATE TRIGGER workspace_events_no_update_delete
    BEFORE UPDATE OR DELETE ON workspace_events
    FOR EACH ROW EXECUTE FUNCTION prevent_workspace_history_change();
CREATE TRIGGER workspace_account_assignment_events_no_update_delete
    BEFORE UPDATE OR DELETE ON workspace_account_assignment_events
    FOR EACH ROW EXECUTE FUNCTION prevent_workspace_history_change();

COMMENT ON TABLE workspaces IS
    'Operational trading/research environments owned by an organization. Organization remains the membership and billing boundary.';
COMMENT ON TABLE workspace_account_assignments IS
    'One operational workspace assignment per Quant account; organization_account_entitlements remains the organization ownership authority.';
COMMENT ON TABLE workspace_account_assignment_events IS
    'Append-only, revisioned audit history for Quant-account workspace placement.';
