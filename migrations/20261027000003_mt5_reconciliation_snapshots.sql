CREATE TABLE IF NOT EXISTS mt5_bridge_reconciliation_snapshots (
    id BIGSERIAL PRIMARY KEY,
    account_id TEXT NOT NULL,
    terminal_id TEXT NOT NULL,
    reconciled_sequence BIGINT NOT NULL CHECK (reconciled_sequence >= 0),
    source TEXT NOT NULL CHECK (length(trim(source)) > 0),
    captured_at TIMESTAMPTZ NOT NULL,
    snapshot JSONB NOT NULL CHECK (jsonb_typeof(snapshot) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_mt5_reconciliation_account_terminal_created
    ON mt5_bridge_reconciliation_snapshots (account_id, terminal_id, created_at DESC);

COMMENT ON TABLE mt5_bridge_reconciliation_snapshots IS
    'Append-only evidence snapshots of authoritative external MT5 account state used to gate reconnect readiness.';
