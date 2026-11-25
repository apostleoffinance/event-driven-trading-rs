-- M3.1: explicit workspace access grants with organization-authority intersection.
-- OWNER/ADMIN are implicit workspace administrators and do not receive grant rows.
-- Every other member needs an active grant. Membership suspension/revocation or
-- promotion to OWNER/ADMIN revokes existing grants in the same transaction.

CREATE TABLE workspace_access_grants (
    organization_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('WORKSPACE_ADMIN','TRADER','RISK_MANAGER','VIEWER')),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','REVOKED')),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    granted_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    updated_by TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id,user_id),
    FOREIGN KEY (workspace_id,organization_id)
        REFERENCES workspaces(id,organization_id) ON DELETE RESTRICT,
    FOREIGN KEY (organization_id,user_id)
        REFERENCES organization_memberships(organization_id,user_id) ON DELETE RESTRICT
);
CREATE INDEX workspace_access_grants_user_active_idx
    ON workspace_access_grants(organization_id,user_id,workspace_id)
    WHERE status='ACTIVE';
CREATE INDEX workspace_access_grants_workspace_active_idx
    ON workspace_access_grants(organization_id,workspace_id,user_id)
    WHERE status='ACTIVE';

CREATE TABLE workspace_access_grant_events (
    id BIGSERIAL PRIMARY KEY,
    organization_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    actor_user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    revision BIGINT NOT NULL CHECK (revision > 0),
    event_type TEXT NOT NULL CHECK (event_type IN ('GRANTED','ROLE_CHANGED','REVOKED','REACTIVATED')),
    previous_role TEXT CHECK (previous_role IS NULL OR previous_role IN ('WORKSPACE_ADMIN','TRADER','RISK_MANAGER','VIEWER')),
    next_role TEXT NOT NULL CHECK (next_role IN ('WORKSPACE_ADMIN','TRADER','RISK_MANAGER','VIEWER')),
    previous_status TEXT CHECK (previous_status IS NULL OR previous_status IN ('ACTIVE','REVOKED')),
    next_status TEXT NOT NULL CHECK (next_status IN ('ACTIVE','REVOKED')),
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id,user_id,revision),
    FOREIGN KEY (workspace_id,organization_id)
        REFERENCES workspaces(id,organization_id) ON DELETE RESTRICT,
    FOREIGN KEY (organization_id,user_id)
        REFERENCES organization_memberships(organization_id,user_id) ON DELETE RESTRICT
);
CREATE INDEX workspace_access_grant_events_history_idx
    ON workspace_access_grant_events(organization_id,workspace_id,user_id,created_at DESC);

CREATE FUNCTION enforce_workspace_access_grant()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    org_status TEXT;
    workspace_status TEXT;
    member_role TEXT;
    member_status TEXT;
    actor_role TEXT;
    actor_status TEXT;
BEGIN
    IF TG_OP='UPDATE' THEN
        IF NEW.organization_id <> OLD.organization_id
           OR NEW.workspace_id <> OLD.workspace_id
           OR NEW.user_id <> OLD.user_id
           OR NEW.granted_by <> OLD.granted_by
           OR NEW.created_at <> OLD.created_at THEN
            RAISE EXCEPTION 'workspace grant identity and grantor are immutable'
                USING ERRCODE='23514';
        END IF;
        IF NEW.revision <> OLD.revision + 1 THEN
            RAISE EXCEPTION 'workspace grant updates require the next revision'
                USING ERRCODE='23514';
        END IF;
        IF NEW.role = OLD.role AND NEW.status = OLD.status THEN
            RAISE EXCEPTION 'workspace grant updates must change role or status'
                USING ERRCODE='23514';
        END IF;
        IF NEW.status='REVOKED' AND OLD.status='REVOKED' THEN
            RAISE EXCEPTION 'workspace grant is already revoked'
                USING ERRCODE='23514';
        END IF;
    ELSIF NEW.revision <> 1 OR NEW.status <> 'ACTIVE' THEN
        RAISE EXCEPTION 'new workspace grants must begin active at revision one'
            USING ERRCODE='23514';
    END IF;

    SELECT o.status,w.status,m.role,m.status
      INTO org_status,workspace_status,member_role,member_status
      FROM organizations o
      JOIN workspaces w ON w.organization_id=o.id
      JOIN organization_memberships m ON m.organization_id=o.id
     WHERE o.id=NEW.organization_id
       AND w.id=NEW.workspace_id
       AND m.user_id=NEW.user_id
     FOR SHARE OF o,w,m;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'workspace grant target is not a member of the workspace organization'
            USING ERRCODE='23503';
    END IF;

    SELECT role,status INTO actor_role,actor_status
      FROM organization_memberships
     WHERE organization_id=NEW.organization_id AND user_id=NEW.updated_by
     FOR SHARE;

    IF NOT FOUND OR actor_status <> 'ACTIVE' OR actor_role NOT IN ('OWNER','ADMIN') THEN
        RAISE EXCEPTION 'workspace grants require an active organization OWNER or ADMIN actor'
            USING ERRCODE='42501';
    END IF;

    IF NEW.status='ACTIVE' THEN
        IF org_status <> 'ACTIVE' OR workspace_status <> 'ACTIVE'
           OR member_status <> 'ACTIVE'
           OR member_role IN ('OWNER','ADMIN') THEN
            RAISE EXCEPTION 'active workspace grants require an active workspace, organization and non-admin member'
                USING ERRCODE='23514';
        END IF;
    END IF;

    NEW.updated_at := NOW();
    RETURN NEW;
END
$$;
CREATE TRIGGER workspace_access_grants_authority_guard
    BEFORE INSERT OR UPDATE ON workspace_access_grants
    FOR EACH ROW EXECUTE FUNCTION enforce_workspace_access_grant();

CREATE FUNCTION record_workspace_access_grant_event()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    event_kind TEXT;
BEGIN
    IF TG_OP='INSERT' THEN
        event_kind := 'GRANTED';
        INSERT INTO workspace_access_grant_events (
            organization_id,workspace_id,user_id,actor_user_id,revision,event_type,
            previous_role,next_role,previous_status,next_status,reason
        ) VALUES (
            NEW.organization_id,NEW.workspace_id,NEW.user_id,NEW.updated_by,NEW.revision,event_kind,
            NULL,NEW.role,NULL,NEW.status,NEW.reason
        );
    ELSE
        IF OLD.status='REVOKED' AND NEW.status='ACTIVE' THEN
            event_kind := 'REACTIVATED';
        ELSIF NEW.status='REVOKED' THEN
            event_kind := 'REVOKED';
        ELSE
            event_kind := 'ROLE_CHANGED';
        END IF;
        INSERT INTO workspace_access_grant_events (
            organization_id,workspace_id,user_id,actor_user_id,revision,event_type,
            previous_role,next_role,previous_status,next_status,reason
        ) VALUES (
            NEW.organization_id,NEW.workspace_id,NEW.user_id,NEW.updated_by,NEW.revision,event_kind,
            OLD.role,NEW.role,OLD.status,NEW.status,NEW.reason
        );
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER workspace_access_grants_audit
    AFTER INSERT OR UPDATE ON workspace_access_grants
    FOR EACH ROW EXECUTE FUNCTION record_workspace_access_grant_event();

CREATE FUNCTION revoke_workspace_grants_on_membership_change()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    grant_row RECORD;
    revoke_reason TEXT;
BEGIN
    IF (NEW.status <> 'ACTIVE') OR NEW.role IN ('OWNER','ADMIN') THEN
        revoke_reason := CASE
            WHEN NEW.status <> 'ACTIVE' THEN 'Automatically revoked because organization membership became ' || NEW.status
            ELSE 'Automatically revoked because organization role became ' || NEW.role
        END;
        FOR grant_row IN
            UPDATE workspace_access_grants
               SET status='REVOKED',
                   revision=revision+1,
                   updated_by=NEW.updated_by,
                   reason=revoke_reason
             WHERE organization_id=NEW.organization_id
               AND user_id=NEW.user_id
               AND status='ACTIVE'
            RETURNING workspace_id
        LOOP
            NULL;
        END LOOP;
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER organization_membership_revokes_workspace_grants
    AFTER UPDATE OF role,status ON organization_memberships
    FOR EACH ROW
    WHEN (OLD.role IS DISTINCT FROM NEW.role OR OLD.status IS DISTINCT FROM NEW.status)
    EXECUTE FUNCTION revoke_workspace_grants_on_membership_change();

CREATE FUNCTION prevent_workspace_access_grant_history_change()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'workspace access grant history is append-only'
        USING ERRCODE='23514';
END
$$;
CREATE TRIGGER workspace_access_grant_events_no_update_delete
    BEFORE UPDATE OR DELETE ON workspace_access_grant_events
    FOR EACH ROW EXECUTE FUNCTION prevent_workspace_access_grant_history_change();

CREATE FUNCTION prevent_workspace_access_grant_delete()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'workspace grants cannot be deleted; revoke them instead'
        USING ERRCODE='23514';
END
$$;
CREATE TRIGGER workspace_access_grants_no_delete
    BEFORE DELETE ON workspace_access_grants
    FOR EACH ROW EXECUTE FUNCTION prevent_workspace_access_grant_delete();

COMMENT ON TABLE workspace_access_grants IS
    'Explicit workspace access for non-OWNER/ADMIN organization members. Grants never elevate organization-level capabilities; OWNER/ADMIN access is implicit.';
COMMENT ON TABLE workspace_access_grant_events IS
    'Append-only grant, role-change, revocation and reactivation history. Membership suspension/revocation and promotion to OWNER/ADMIN automatically revoke active grants.';
