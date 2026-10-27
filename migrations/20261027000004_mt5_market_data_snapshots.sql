CREATE TABLE IF NOT EXISTS mt5_bridge_market_data_snapshots (
    id BIGSERIAL PRIMARY KEY,
    account_id TEXT NOT NULL,
    terminal_id TEXT NOT NULL,
    server TEXT NOT NULL CHECK (length(trim(server)) > 0),
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    source TEXT NOT NULL CHECK (length(trim(source)) > 0),
    account_currency TEXT NOT NULL CHECK (length(trim(account_currency)) > 0),
    captured_at TIMESTAMPTZ NOT NULL,
    snapshot JSONB NOT NULL CHECK (jsonb_typeof(snapshot) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_mt5_market_data_account_terminal_captured
    ON mt5_bridge_market_data_snapshots (account_id, terminal_id, captured_at DESC);

CREATE INDEX IF NOT EXISTS idx_mt5_market_data_symbol
    ON mt5_bridge_market_data_snapshots USING GIN ((snapshot -> 'instruments'));

COMMENT ON TABLE mt5_bridge_market_data_snapshots IS
    'Read-only, broker-authoritative MT5 instrument specifications and bid/ask quotes. Consumers must enforce quote freshness.';
