-- Serialize workspace account moves against all durable execution-state writes.
-- The API acquires the same account-scoped transaction advisory lock before
-- checking flatness/orders/worker leases and updating the workspace assignment.
-- BEFORE triggers ensure independent API and worker processes participate too.

CREATE OR REPLACE FUNCTION workspace_account_state_advisory_lock()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    old_account_id TEXT;
    new_account_id TEXT;
BEGIN
    IF TG_OP <> 'INSERT' THEN
        old_account_id := to_jsonb(OLD) ->> TG_ARGV[0];
    END IF;
    IF TG_OP <> 'DELETE' THEN
        new_account_id := to_jsonb(NEW) ->> TG_ARGV[0];
    END IF;

    -- Lock both identities in a stable order if a writer changes account scope.
    IF old_account_id IS NOT NULL
       AND new_account_id IS NOT NULL
       AND old_account_id <> new_account_id THEN
        IF old_account_id < new_account_id THEN
            PERFORM pg_advisory_xact_lock(hashtextextended(old_account_id, 78123));
            PERFORM pg_advisory_xact_lock(hashtextextended(new_account_id, 78123));
        ELSE
            PERFORM pg_advisory_xact_lock(hashtextextended(new_account_id, 78123));
            PERFORM pg_advisory_xact_lock(hashtextextended(old_account_id, 78123));
        END IF;
    ELSE
        PERFORM pg_advisory_xact_lock(
            hashtextextended(COALESCE(new_account_id, old_account_id), 78123)
        );
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER workspace_account_lock_account_assignments
    BEFORE INSERT OR UPDATE OR DELETE ON workspace_account_assignments
    FOR EACH ROW EXECUTE FUNCTION workspace_account_state_advisory_lock('account_id');

CREATE TRIGGER workspace_account_lock_account_states
    BEFORE INSERT OR UPDATE OR DELETE ON account_states
    FOR EACH ROW EXECUTE FUNCTION workspace_account_state_advisory_lock('account_id');

CREATE TRIGGER workspace_account_lock_orders
    BEFORE INSERT OR UPDATE OR DELETE ON orders
    FOR EACH ROW EXECUTE FUNCTION workspace_account_state_advisory_lock('account_id');

CREATE TRIGGER workspace_account_lock_positions
    BEFORE INSERT OR UPDATE OR DELETE ON positions
    FOR EACH ROW EXECUTE FUNCTION workspace_account_state_advisory_lock('account_id');

CREATE TRIGGER workspace_account_lock_mt5_order_keys
    BEFORE INSERT OR UPDATE OR DELETE ON mt5_bridge_order_keys
    FOR EACH ROW EXECUTE FUNCTION workspace_account_state_advisory_lock('account_id');

CREATE TRIGGER workspace_account_lock_mt5_reconciliation
    BEFORE INSERT OR UPDATE OR DELETE ON mt5_bridge_reconciliation_snapshots
    FOR EACH ROW EXECUTE FUNCTION workspace_account_state_advisory_lock('account_id');

CREATE TRIGGER workspace_account_lock_worker_leases
    BEFORE INSERT OR UPDATE OR DELETE ON broker_worker_leases
    FOR EACH ROW EXECUTE FUNCTION workspace_account_state_advisory_lock('quant_account_id');

COMMENT ON FUNCTION workspace_account_state_advisory_lock() IS
    'Serializes account execution-state writes with workspace reassignment safety checks using advisory transaction lock seed 78123.';

-- A READY broker worker session is active even if its publisher lease has not
-- yet been acquired. Serialize session lifecycle changes with reassignment too.
CREATE OR REPLACE FUNCTION workspace_worker_session_advisory_lock()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    old_account_id TEXT;
    new_account_id TEXT;
BEGIN
    IF TG_OP <> 'INSERT' THEN
        SELECT quant_account_id INTO old_account_id
          FROM broker_worker_assignments
         WHERE id = OLD.assignment_id;
    END IF;
    IF TG_OP <> 'DELETE' THEN
        SELECT quant_account_id INTO new_account_id
          FROM broker_worker_assignments
         WHERE id = NEW.assignment_id;
    END IF;

    IF old_account_id IS NOT NULL
       AND new_account_id IS NOT NULL
       AND old_account_id <> new_account_id THEN
        IF old_account_id < new_account_id THEN
            PERFORM pg_advisory_xact_lock(hashtextextended(old_account_id, 78123));
            PERFORM pg_advisory_xact_lock(hashtextextended(new_account_id, 78123));
        ELSE
            PERFORM pg_advisory_xact_lock(hashtextextended(new_account_id, 78123));
            PERFORM pg_advisory_xact_lock(hashtextextended(old_account_id, 78123));
        END IF;
    ELSE
        PERFORM pg_advisory_xact_lock(
            hashtextextended(COALESCE(new_account_id, old_account_id), 78123)
        );
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER workspace_account_lock_worker_sessions
    BEFORE INSERT OR UPDATE OR DELETE ON broker_worker_sessions
    FOR EACH ROW EXECUTE FUNCTION workspace_worker_session_advisory_lock();
