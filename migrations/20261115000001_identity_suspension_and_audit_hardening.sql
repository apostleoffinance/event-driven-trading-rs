-- QOS-021A-E3A-R3B-A-I1-R1
-- Suspend sessions in the same transaction as identity deactivation and harden
-- session-event immutability and attribution.

ALTER TABLE identity_session_events
    DROP CONSTRAINT identity_session_events_session_id_fkey,
    ADD CONSTRAINT identity_session_events_session_user_fk
        FOREIGN KEY (session_id,user_id)
        REFERENCES identity_sessions(id,user_id)
        ON DELETE RESTRICT;

CREATE OR REPLACE FUNCTION reject_identity_session_event_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'identity session events are append-only';
END
$$;

DROP TRIGGER IF EXISTS identity_session_events_no_update_delete ON identity_session_events;
CREATE TRIGGER identity_session_events_no_update_delete
    BEFORE UPDATE OR DELETE ON identity_session_events
    FOR EACH ROW EXECUTE FUNCTION reject_identity_session_event_mutation();

DROP TRIGGER IF EXISTS identity_session_events_no_truncate ON identity_session_events;
CREATE TRIGGER identity_session_events_no_truncate
    BEFORE TRUNCATE ON identity_session_events
    FOR EACH STATEMENT EXECUTE FUNCTION reject_identity_session_event_mutation();

REVOKE UPDATE, DELETE, TRUNCATE ON identity_session_events FROM PUBLIC;
REVOKE ALL PRIVILEGES ON SEQUENCE identity_session_events_id_seq FROM PUBLIC;

-- Install the least-privilege grants when the deployment has provisioned its
-- runtime role. Application startup separately fails closed unless its actual
-- database role has these privileges, is not the owner, and cannot alter the
-- table. SQLx's isolated role lacks CREATEROLE, so role provisioning remains an
-- explicit database-bootstrap responsibility rather than a migration side effect.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'edgenest_runtime') THEN
        EXECUTE 'REVOKE ALL PRIVILEGES ON identity_session_events FROM edgenest_runtime';
        EXECUTE 'GRANT SELECT, INSERT ON identity_session_events TO edgenest_runtime';
        EXECUTE 'REVOKE ALL PRIVILEGES ON SEQUENCE identity_session_events_id_seq FROM edgenest_runtime';
        EXECUTE 'GRANT USAGE, SELECT ON SEQUENCE identity_session_events_id_seq TO edgenest_runtime';
    END IF;
END
$$;

CREATE OR REPLACE FUNCTION revoke_identity_sessions_on_suspension() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
DECLARE
    session_row RECORD;
    revoked_revision BIGINT;
BEGIN
    IF NEW.is_active IS FALSE THEN
        -- The identity row is already locked by the UPDATE. Session locks are
        -- acquired in a stable order, followed by refresh rows, access rows,
        -- then append-only audit inserts.
        FOR session_row IN
            SELECT id, user_id
              FROM public.identity_sessions
             WHERE user_id = NEW.id AND status = 'ACTIVE'
             ORDER BY id
             FOR UPDATE
        LOOP
            UPDATE public.identity_sessions
               SET status = 'REVOKED',
                   revision = revision + 1,
                   revoked_at = COALESCE(revoked_at, NOW()),
                   revocation_reason = COALESCE(revocation_reason, 'identity account suspended'),
                   updated_at = NOW()
             WHERE id = session_row.id AND user_id = session_row.user_id
               AND status = 'ACTIVE'
            RETURNING revision INTO revoked_revision;

            IF FOUND THEN
                UPDATE public.identity_refresh_tokens
                   SET revoked_at = COALESCE(revoked_at, NOW()),
                       last_used_at = COALESCE(last_used_at, NOW())
                 WHERE session_id = session_row.id AND revoked_at IS NULL;

                UPDATE public.identity_access_tokens
                   SET revoked_at = COALESCE(revoked_at, NOW()),
                       revocation_reason = COALESCE(revocation_reason, 'identity account suspended')
                 WHERE session_id = session_row.id AND revoked_at IS NULL;

                INSERT INTO public.identity_session_events
                    (session_id,user_id,event_type,session_revision,reason)
                VALUES
                    (session_row.id,session_row.user_id,'SESSION_REVOKED',revoked_revision,
                     'identity account suspended');
            END IF;
        END LOOP;
    END IF;
    RETURN NEW;
END
$$;

REVOKE ALL ON FUNCTION revoke_identity_sessions_on_suspension() FROM PUBLIC;

DROP TRIGGER IF EXISTS identity_user_suspension_revokes_sessions ON identity_users;
CREATE TRIGGER identity_user_suspension_revokes_sessions
    AFTER UPDATE OF is_active ON identity_users
    FOR EACH ROW EXECUTE FUNCTION revoke_identity_sessions_on_suspension();

COMMENT ON FUNCTION revoke_identity_sessions_on_suspension() IS
    'Atomically revoke active sessions and their refresh/access tokens when an identity user is deactivated.';
