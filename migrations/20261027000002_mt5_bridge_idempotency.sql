-- QOS-014: durable MT5 order idempotency and reconnect sequence checkpoints.
-- A row is committed before transport send. A pre-existing "prepared" row is
-- ambiguous after restart and MUST be reconciled, never blindly resubmitted.
CREATE TABLE IF NOT EXISTS mt5_bridge_order_keys (
    account_id          TEXT NOT NULL,
    client_order_id     TEXT NOT NULL,
    request_payload     JSONB NOT NULL,
    state               TEXT NOT NULL CHECK (
        state IN ('prepared', 'unknown', 'accepted', 'rejected')
    ),
    venue_order_id      TEXT,
    reason_code         TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, client_order_id)
);

CREATE INDEX IF NOT EXISTS idx_mt5_bridge_order_keys_state
    ON mt5_bridge_order_keys(account_id, state, updated_at);

CREATE TABLE IF NOT EXISTS mt5_bridge_streams (
    account_id          TEXT NOT NULL,
    terminal_id         TEXT NOT NULL,
    last_sequence       BIGINT NOT NULL CHECK (last_sequence >= 0),
    readiness           TEXT NOT NULL CHECK (readiness IN ('ready', 'degraded')),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, terminal_id)
);
