-- MT5-MVP-01A-1: one durable publisher lease per Quant account and terminal.
-- Fencing tokens are monotonic across expiry/reacquisition. The lease token is
-- stored only as SHA-256; callers must keep the raw token process-local.

CREATE TABLE broker_worker_leases (
    quant_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    terminal_id TEXT NOT NULL CHECK (length(trim(terminal_id)) BETWEEN 1 AND 128),
    assignment_id TEXT NOT NULL REFERENCES broker_worker_assignments(id) ON DELETE RESTRICT,
    worker_ref TEXT NOT NULL CHECK (length(trim(worker_ref)) BETWEEN 1 AND 128),
    lease_token_hash BYTEA NOT NULL CHECK (octet_length(lease_token_hash) = 32),
    fencing_token BIGINT NOT NULL CHECK (fencing_token > 0),
    lease_until TIMESTAMPTZ NOT NULL,
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    renewed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    released_at TIMESTAMPTZ,
    PRIMARY KEY (quant_account_id, terminal_id),
    CHECK (lease_until > acquired_at OR released_at IS NOT NULL)
);

CREATE INDEX broker_worker_leases_expiry_idx
    ON broker_worker_leases(lease_until)
    WHERE released_at IS NULL;

COMMENT ON TABLE broker_worker_leases IS
    'Single active publisher lease per Quant account and MT5 terminal. Every takeover receives a strictly increasing fencing token.';
COMMENT ON COLUMN broker_worker_leases.lease_token_hash IS
    'SHA-256 of a process-unique random bearer token; the raw token is never persisted.';
COMMENT ON COLUMN broker_worker_leases.fencing_token IS
    'Monotonic sequence used to reject stale publishers after expiry or takeover.';

CREATE FUNCTION validate_mt5_worker_lease_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF ROW(NEW.quant_account_id, NEW.terminal_id)
       IS DISTINCT FROM ROW(OLD.quant_account_id, OLD.terminal_id) THEN
        RAISE EXCEPTION 'MT5 worker lease scope is immutable';
    END IF;

    IF NEW.fencing_token < OLD.fencing_token
       OR NEW.fencing_token > OLD.fencing_token + 1 THEN
        RAISE EXCEPTION 'MT5 worker lease fencing token must be monotonic';
    END IF;

    IF NEW.fencing_token = OLD.fencing_token + 1 THEN
        IF (OLD.released_at IS NULL AND OLD.lease_until > NOW())
           OR NEW.released_at IS NOT NULL
           OR NEW.lease_until <= NOW()
           OR NEW.acquired_at <= OLD.acquired_at
           OR NEW.lease_token_hash = OLD.lease_token_hash THEN
            RAISE EXCEPTION 'MT5 worker lease takeover requires expiry/release and a fresh token';
        END IF;
    ELSIF NEW.lease_token_hash IS DISTINCT FROM OLD.lease_token_hash
       OR NEW.assignment_id IS DISTINCT FROM OLD.assignment_id
       OR NEW.worker_ref IS DISTINCT FROM OLD.worker_ref
       OR NEW.acquired_at IS DISTINCT FROM OLD.acquired_at THEN
        RAISE EXCEPTION 'MT5 worker lease owner may change only with a new fencing token';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER broker_worker_leases_fencing_guard
    BEFORE UPDATE ON broker_worker_leases
    FOR EACH ROW EXECUTE FUNCTION validate_mt5_worker_lease_update();

CREATE FUNCTION reject_mt5_worker_lease_delete() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'MT5 worker lease rows cannot be deleted; fencing history must be retained';
END;
$$;

CREATE TRIGGER broker_worker_leases_no_delete
    BEFORE DELETE ON broker_worker_leases
    FOR EACH ROW EXECUTE FUNCTION reject_mt5_worker_lease_delete();
