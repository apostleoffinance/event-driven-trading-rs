-- MT5-MVP-01A-1: one durable publisher lease per Quant account and terminal.
-- Fencing tokens are monotonic across expiry/reacquisition. The lease token is
-- stored only as SHA-256; callers must keep the raw token process-local.

CREATE SEQUENCE broker_worker_lease_fencing_token_seq AS BIGINT START WITH 1;

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
