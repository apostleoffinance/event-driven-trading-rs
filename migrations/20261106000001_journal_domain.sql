-- EdgeNest Journal domain (Phase 1).
-- Journal records are NOT OMS orders/fills/positions. They cannot authorize execution.
-- Monetary/price/quantity fields use NUMERIC; source facts are retained with provenance.

CREATE TABLE IF NOT EXISTS journal_accounts (
    id                      TEXT PRIMARY KEY,
    owner_id                TEXT NOT NULL CHECK (length(trim(owner_id)) > 0),
    name                    TEXT NOT NULL CHECK (length(trim(name)) > 0),
    broker                  TEXT,
    account_type            TEXT NOT NULL DEFAULT 'journal'
                                CHECK (account_type IN ('journal', 'paper', 'funded', 'live', 'demo')),
    currency                TEXT NOT NULL DEFAULT 'USD'
                                CHECK (currency ~ '^[A-Z]{3}$'),
    initial_balance         NUMERIC(38, 18) NOT NULL DEFAULT 0
                                CHECK (initial_balance >= 0),
    linked_trading_account_id TEXT REFERENCES accounts(id),
    external_source         TEXT,
    external_account_id     TEXT,
    archived                BOOLEAN NOT NULL DEFAULT FALSE,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (owner_id, external_source, external_account_id),
    UNIQUE (owner_id, id)
);

CREATE INDEX IF NOT EXISTS idx_journal_accounts_owner
    ON journal_accounts(owner_id, archived);

CREATE TABLE IF NOT EXISTS journal_setups (
    id              TEXT PRIMARY KEY,
    owner_id        TEXT NOT NULL CHECK (length(trim(owner_id)) > 0),
    name            TEXT NOT NULL CHECK (length(trim(name)) > 0),
    description     TEXT,
    rules_json      JSONB NOT NULL DEFAULT '{}'::jsonb
                        CHECK (jsonb_typeof(rules_json) = 'object'),
    archived        BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (owner_id, name),
    UNIQUE (owner_id, id)
);

CREATE TABLE IF NOT EXISTS journal_trades (
    id                  TEXT PRIMARY KEY,
    owner_id            TEXT NOT NULL CHECK (length(trim(owner_id)) > 0),
    journal_account_id  TEXT NOT NULL,
    setup_id            TEXT,
    symbol              TEXT NOT NULL CHECK (length(trim(symbol)) > 0),
    asset_class         TEXT NOT NULL DEFAULT 'forex',
    direction           TEXT NOT NULL CHECK (direction IN ('long', 'short')),
    status              TEXT NOT NULL DEFAULT 'closed'
                            CHECK (status IN ('open', 'closed', 'cancelled')),
    source              TEXT NOT NULL DEFAULT 'manual'
                            CHECK (source IN ('manual', 'mt5', 'legacy_import', 'other')),
    source_system       TEXT,
    source_trade_id     TEXT,
    source_deal_ids     JSONB NOT NULL DEFAULT '[]'::jsonb
                            CHECK (jsonb_typeof(source_deal_ids) = 'array'),
    source_payload      JSONB NOT NULL DEFAULT '{}'::jsonb
                            CHECK (jsonb_typeof(source_payload) = 'object'),
    opened_at           TIMESTAMPTZ NOT NULL,
    closed_at           TIMESTAMPTZ,
    quantity            NUMERIC(38, 18) NOT NULL CHECK (quantity > 0),
    entry_price         NUMERIC(38, 18) NOT NULL CHECK (entry_price > 0),
    exit_price          NUMERIC(38, 18) CHECK (exit_price IS NULL OR exit_price > 0),
    stop_loss           NUMERIC(38, 18) CHECK (stop_loss IS NULL OR stop_loss > 0),
    take_profit         NUMERIC(38, 18) CHECK (take_profit IS NULL OR take_profit > 0),
    gross_pnl           NUMERIC(38, 18),
    commission          NUMERIC(38, 18) NOT NULL DEFAULT 0,
    swap                NUMERIC(38, 18) NOT NULL DEFAULT 0,
    other_fees          NUMERIC(38, 18) NOT NULL DEFAULT 0,
    net_pnl             NUMERIC(38, 18),
    pnl_currency        TEXT NOT NULL DEFAULT 'USD'
                            CHECK (pnl_currency ~ '^[A-Z]{3}$'),
    risk_amount         NUMERIC(38, 18) CHECK (risk_amount IS NULL OR risk_amount >= 0),
    r_multiple          NUMERIC(38, 18),
    pips                NUMERIC(38, 18),
    session             TEXT,
    tags                JSONB NOT NULL DEFAULT '[]'::jsonb
                            CHECK (jsonb_typeof(tags) = 'array'),
    psychology          JSONB NOT NULL DEFAULT '{}'::jsonb
                            CHECK (jsonb_typeof(psychology) = 'object'),
    checklist           JSONB NOT NULL DEFAULT '{}'::jsonb
                            CHECK (jsonb_typeof(checklist) = 'object'),
    notes               TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (closed_at IS NULL OR closed_at >= opened_at),
    CHECK (status <> 'closed' OR closed_at IS NOT NULL),
    CHECK (status <> 'closed' OR exit_price IS NOT NULL),
    CHECK ((source_system IS NULL) = (source_trade_id IS NULL)),
    UNIQUE (owner_id, id),
    FOREIGN KEY (owner_id, journal_account_id)
        REFERENCES journal_accounts(owner_id, id),
    FOREIGN KEY (owner_id, setup_id)
        REFERENCES journal_setups(owner_id, id)
);

CREATE INDEX IF NOT EXISTS idx_journal_trades_owner_opened
    ON journal_trades(owner_id, opened_at DESC);
CREATE INDEX IF NOT EXISTS idx_journal_trades_account_opened
    ON journal_trades(journal_account_id, opened_at DESC);
CREATE INDEX IF NOT EXISTS idx_journal_trades_setup
    ON journal_trades(setup_id);
CREATE INDEX IF NOT EXISTS idx_journal_trades_symbol
    ON journal_trades(owner_id, symbol, opened_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS uq_journal_trades_source
    ON journal_trades(journal_account_id, source_system, source_trade_id)
    WHERE source_system IS NOT NULL AND source_trade_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS journal_screenshots (
    id                  TEXT PRIMARY KEY,
    owner_id            TEXT NOT NULL CHECK (length(trim(owner_id)) > 0),
    trade_id            TEXT NOT NULL,
    storage_provider    TEXT NOT NULL CHECK (storage_provider IN ('s3', 'r2', 'minio', 'local', 'external')),
    object_key          TEXT NOT NULL CHECK (length(trim(object_key)) > 0),
    original_filename   TEXT,
    content_type        TEXT,
    byte_size           BIGINT CHECK (byte_size IS NULL OR byte_size >= 0),
    sha256              TEXT,
    captured_at         TIMESTAMPTZ,
    metadata             JSONB NOT NULL DEFAULT '{}'::jsonb
                            CHECK (jsonb_typeof(metadata) = 'object'),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (storage_provider, object_key),
    FOREIGN KEY (owner_id, trade_id)
        REFERENCES journal_trades(owner_id, id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_journal_screenshots_trade
    ON journal_screenshots(trade_id);

CREATE TABLE IF NOT EXISTS journal_trade_events (
    id              BIGSERIAL PRIMARY KEY,
    trade_id        TEXT NOT NULL,
    owner_id        TEXT NOT NULL CHECK (length(trim(owner_id)) > 0),
    event_type      TEXT NOT NULL CHECK (length(trim(event_type)) > 0),
    actor_id        TEXT,
    event_payload   JSONB NOT NULL DEFAULT '{}'::jsonb
                        CHECK (jsonb_typeof(event_payload) = 'object'),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (owner_id, trade_id)
        REFERENCES journal_trades(owner_id, id)
);

CREATE INDEX IF NOT EXISTS idx_journal_trade_events_trade_time
    ON journal_trade_events(trade_id, created_at);

COMMENT ON TABLE journal_trades IS
    'User journal ledger. Distinct from OMS orders/fills/positions; never an execution authorization source.';
COMMENT ON COLUMN journal_trades.source_payload IS
    'Immutable source snapshot/provenance for imports and broker facts; avoid storing secrets or credentials.';
COMMENT ON COLUMN journal_accounts.linked_trading_account_id IS
    'Optional explicit link to an EdgeNest execution account. Journal accounts are not automatically execution accounts.';
