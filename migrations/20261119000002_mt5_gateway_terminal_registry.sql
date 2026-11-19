-- MT5-MVP-02B: durable registry for EdgeNest-managed gateway hosts and isolated terminal slots.
-- This migration records desired/observed infrastructure identity only. It does not
-- provision hosts, start terminals, accept credentials, or enable order execution.

CREATE TABLE mt5_gateway_hosts (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9][a-z0-9._-]{0,127}$'),
    provider TEXT NOT NULL CHECK (length(trim(provider)) BETWEEN 1 AND 80),
    region TEXT NOT NULL CHECK (length(trim(region)) BETWEEN 1 AND 80),
    host_ref TEXT NOT NULL UNIQUE CHECK (length(trim(host_ref)) BETWEEN 1 AND 256),
    status TEXT NOT NULL DEFAULT 'PLANNED'
        CHECK (status IN ('PLANNED','PROVISIONING','READY','DEGRADED','DRAINING','DISABLED')),
    terminal_capacity INTEGER NOT NULL DEFAULT 1 CHECK (terminal_capacity BETWEEN 1 AND 128),
    registered_terminal_slots INTEGER NOT NULL DEFAULT 0 CHECK (registered_terminal_slots >= 0 AND registered_terminal_slots <= terminal_capacity),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX mt5_gateway_hosts_status_region_idx ON mt5_gateway_hosts(status,region);

CREATE TABLE mt5_terminal_slots (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9][a-z0-9._-]{0,127}$'),
    gateway_host_id TEXT NOT NULL REFERENCES mt5_gateway_hosts(id) ON DELETE RESTRICT,
    broker_id TEXT NOT NULL,
    broker_server_id TEXT NOT NULL,
    terminal_id TEXT NOT NULL CHECK (length(trim(terminal_id)) BETWEEN 1 AND 128),
    terminal_profile_ref TEXT NOT NULL UNIQUE CHECK (length(trim(terminal_profile_ref)) BETWEEN 1 AND 256),
    service_identity_ref TEXT NOT NULL UNIQUE CHECK (length(trim(service_identity_ref)) BETWEEN 1 AND 256),
    worker_ref TEXT NOT NULL UNIQUE CHECK (length(trim(worker_ref)) BETWEEN 1 AND 256),
    status TEXT NOT NULL DEFAULT 'PLANNED'
        CHECK (status IN ('PLANNED','PROVISIONING','READY','DEGRADED','DRAINING','DISABLED')),
    isolation_version SMALLINT NOT NULL DEFAULT 1 CHECK (isolation_version > 0),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (broker_server_id,broker_id)
        REFERENCES broker_servers(id,broker_id) ON DELETE RESTRICT,
    UNIQUE (gateway_host_id,terminal_id)
);

CREATE INDEX mt5_terminal_slots_gateway_status_idx
    ON mt5_terminal_slots(gateway_host_id,status);
CREATE INDEX mt5_terminal_slots_broker_server_idx
    ON mt5_terminal_slots(broker_id,broker_server_id,status);

ALTER TABLE broker_worker_assignments
    ADD COLUMN terminal_slot_id TEXT
    REFERENCES mt5_terminal_slots(id) ON DELETE RESTRICT;

-- Existing single-slot assignments remain valid and are intentionally not
-- backfilled. New assignments may opt into the registry once the runtime
-- admission path has been upgraded to require and verify slot identity.
CREATE UNIQUE INDEX broker_worker_assignment_active_terminal_slot_uq
    ON broker_worker_assignments(terminal_slot_id)
    WHERE terminal_slot_id IS NOT NULL AND status = 'ACTIVE';

CREATE FUNCTION validate_mt5_terminal_slot_assignment() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    slot mt5_terminal_slots%ROWTYPE;
BEGIN
    IF NEW.terminal_slot_id IS NULL THEN
        RETURN NEW;
    END IF;

    SELECT * INTO slot
      FROM mt5_terminal_slots
     WHERE id = NEW.terminal_slot_id
     FOR SHARE;

    IF NOT FOUND OR slot.status <> 'READY'
       OR slot.worker_ref <> NEW.worker_ref
       OR slot.broker_id <> NEW.broker_id
       OR slot.broker_server_id <> NEW.broker_server_id THEN
        RAISE EXCEPTION 'worker assignment does not match a READY isolated MT5 terminal slot';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER broker_worker_assignment_terminal_slot_guard
    BEFORE INSERT ON broker_worker_assignments
    FOR EACH ROW EXECUTE FUNCTION validate_mt5_terminal_slot_assignment();

CREATE FUNCTION enforce_mt5_gateway_terminal_capacity() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.terminal_capacity < OLD.registered_terminal_slots THEN
        RAISE EXCEPTION 'gateway terminal capacity cannot be lower than its registered slots';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER mt5_gateway_capacity_guard
    BEFORE UPDATE OF terminal_capacity ON mt5_gateway_hosts
    FOR EACH ROW EXECUTE FUNCTION enforce_mt5_gateway_terminal_capacity();

CREATE FUNCTION enforce_mt5_terminal_slot_capacity() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND
       ROW(NEW.gateway_host_id,NEW.terminal_id,NEW.terminal_profile_ref,
           NEW.service_identity_ref,NEW.worker_ref,NEW.broker_id,NEW.broker_server_id)
       IS DISTINCT FROM
       ROW(OLD.gateway_host_id,OLD.terminal_id,OLD.terminal_profile_ref,
           OLD.service_identity_ref,OLD.worker_ref,OLD.broker_id,OLD.broker_server_id) THEN
        RAISE EXCEPTION 'MT5 terminal slot identity is immutable';
    END IF;

    IF TG_OP = 'INSERT' THEN
        IF NEW.status <> 'DISABLED' THEN
            UPDATE mt5_gateway_hosts
               SET registered_terminal_slots=registered_terminal_slots+1,
                   updated_at=NOW()
             WHERE id=NEW.gateway_host_id
               AND status NOT IN ('DRAINING','DISABLED')
               AND registered_terminal_slots < terminal_capacity;
            IF NOT FOUND THEN
                RAISE EXCEPTION 'MT5 gateway is unavailable or terminal capacity has been reached';
            END IF;
        END IF;
        RETURN NEW;
    END IF;

    IF NEW.revision <> OLD.revision + 1 THEN
        RAISE EXCEPTION 'MT5 terminal slot status changes require the next revision';
    END IF;

    IF OLD.status = 'DISABLED' AND NEW.status <> 'DISABLED' THEN
        UPDATE mt5_gateway_hosts
           SET registered_terminal_slots=registered_terminal_slots+1,
               updated_at=NOW()
         WHERE id=NEW.gateway_host_id
           AND status NOT IN ('DRAINING','DISABLED')
           AND registered_terminal_slots < terminal_capacity;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'MT5 gateway is unavailable or terminal capacity has been reached';
        END IF;
    ELSIF OLD.status <> 'DISABLED' AND NEW.status = 'DISABLED' THEN
        UPDATE mt5_gateway_hosts
           SET registered_terminal_slots=registered_terminal_slots-1,
               updated_at=NOW()
         WHERE id=NEW.gateway_host_id AND registered_terminal_slots > 0;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'MT5 gateway terminal slot counter is inconsistent';
        END IF;
    END IF;

    NEW.updated_at := NOW();
    RETURN NEW;
END;
$$;

CREATE TRIGGER mt5_terminal_slot_identity_capacity_guard
    BEFORE INSERT OR UPDATE ON mt5_terminal_slots
    FOR EACH ROW EXECUTE FUNCTION enforce_mt5_terminal_slot_capacity();

CREATE FUNCTION reject_mt5_registry_delete() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'MT5 gateway and terminal registry history cannot be deleted';
END;
$$;

CREATE TRIGGER mt5_gateway_hosts_no_delete
    BEFORE DELETE ON mt5_gateway_hosts
    FOR EACH ROW EXECUTE FUNCTION reject_mt5_registry_delete();
CREATE TRIGGER mt5_terminal_slots_no_delete
    BEFORE DELETE ON mt5_terminal_slots
    FOR EACH ROW EXECUTE FUNCTION reject_mt5_registry_delete();

COMMENT ON TABLE mt5_gateway_hosts IS
    'EdgeNest-owned gateway inventory. host_ref is an opaque infrastructure reference; no IP, password, private key or RDP secret is stored.';
COMMENT ON TABLE mt5_terminal_slots IS
    'One isolated MT5 terminal profile, service identity and worker principal per broker account slot. Capacity is explicitly bounded per gateway.';
COMMENT ON COLUMN mt5_terminal_slots.terminal_profile_ref IS
    'Opaque reference to a protected terminal data/profile location; never a credential or filesystem secret.';
COMMENT ON COLUMN mt5_terminal_slots.service_identity_ref IS
    'Opaque reference to the dedicated OS service identity for this slot.';
COMMENT ON COLUMN mt5_terminal_slots.worker_ref IS
    'Unique mTLS workload principal reference; it must not be shared across terminal slots.';
COMMENT ON COLUMN broker_worker_assignments.terminal_slot_id IS
    'Optional during compatibility transition. New registry-backed assignments must match the READY slot worker, broker and server.';
