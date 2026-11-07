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


-- Extended journal facts. Nullable/defaulted fields preserve existing trade API writes.
ALTER TABLE journal_trades
  ADD COLUMN IF NOT EXISTS venue TEXT,
  ADD COLUMN IF NOT EXISTS external_account_id TEXT,
  ADD COLUMN IF NOT EXISTS raw_symbol TEXT,
  ADD COLUMN IF NOT EXISTS instrument_type TEXT,
  ADD COLUMN IF NOT EXISTS timeframe TEXT,
  ADD COLUMN IF NOT EXISTS timezone TEXT,
  ADD COLUMN IF NOT EXISTS in_preferred_session BOOLEAN,
  ADD COLUMN IF NOT EXISTS planned_risk_percent NUMERIC(38,18),
  ADD COLUMN IF NOT EXISTS planned_reward NUMERIC(38,18),
  ADD COLUMN IF NOT EXISTS planned_rr NUMERIC(38,18),
  ADD COLUMN IF NOT EXISTS stop_distance NUMERIC(38,18),
  ADD COLUMN IF NOT EXISTS target_distance NUMERIC(38,18),
  ADD COLUMN IF NOT EXISTS setup_valid BOOLEAN,
  ADD COLUMN IF NOT EXISTS rules_followed BOOLEAN,
  ADD COLUMN IF NOT EXISTS emotional_trade BOOLEAN,
  ADD COLUMN IF NOT EXISTS mistake BOOLEAN,
  ADD COLUMN IF NOT EXISTS mistake_notes TEXT,
  ADD COLUMN IF NOT EXISTS discipline_score SMALLINT,
  ADD COLUMN IF NOT EXISTS acknowledged_warnings JSONB NOT NULL DEFAULT '[]'::jsonb,
  ADD COLUMN IF NOT EXISTS mfe_price NUMERIC(38,18),
  ADD COLUMN IF NOT EXISTS mae_price NUMERIC(38,18),
  ADD COLUMN IF NOT EXISTS mfe_r NUMERIC(38,18),
  ADD COLUMN IF NOT EXISTS mae_r NUMERIC(38,18),
  ADD COLUMN IF NOT EXISTS mfe_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS mae_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS mfe_source TEXT,
  ADD COLUMN IF NOT EXISTS mae_source TEXT,
  ADD COLUMN IF NOT EXISTS mfe_precision TEXT,
  ADD COLUMN IF NOT EXISTS mae_precision TEXT,
  ADD COLUMN IF NOT EXISTS source_metadata JSONB NOT NULL DEFAULT '{}'::jsonb;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'journal_trades_discipline_score_range'
      AND conrelid = 'journal_trades'::regclass
  ) THEN
    ALTER TABLE journal_trades
      ADD CONSTRAINT journal_trades_discipline_score_range
      CHECK (discipline_score IS NULL OR discipline_score BETWEEN 0 AND 100);
  END IF;
END $$;

CREATE INDEX IF NOT EXISTS journal_trades_owner_account_closed_idx
  ON journal_trades (owner_id, journal_account_id, closed_at DESC)
  WHERE status = 'closed';
CREATE INDEX IF NOT EXISTS journal_trades_owner_symbol_opened_idx
  ON journal_trades (owner_id, symbol, opened_at DESC);
CREATE INDEX IF NOT EXISTS journal_trades_owner_setup_opened_idx
  ON journal_trades (owner_id, setup_id, opened_at DESC)
  WHERE setup_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS journal_trade_psychology (
  trade_id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  emotion_before TEXT,
  emotion_during TEXT,
  emotion_after TEXT,
  confidence SMALLINT,
  fear SMALLINT,
  fomo SMALLINT,
  frustration SMALLINT,
  revenge_intensity SMALLINT,
  boredom SMALLINT,
  notes TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT journal_trade_psychology_ratings_range CHECK (
    (confidence IS NULL OR confidence BETWEEN 0 AND 10) AND
    (fear IS NULL OR fear BETWEEN 0 AND 10) AND
    (fomo IS NULL OR fomo BETWEEN 0 AND 10) AND
    (frustration IS NULL OR frustration BETWEEN 0 AND 10) AND
    (revenge_intensity IS NULL OR revenge_intensity BETWEEN 0 AND 10) AND
    (boredom IS NULL OR boredom BETWEEN 0 AND 10)
  ),
  FOREIGN KEY (owner_id, trade_id)
    REFERENCES journal_trades(owner_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS journal_trade_psychology_owner_idx
  ON journal_trade_psychology (owner_id, trade_id);

CREATE TABLE IF NOT EXISTS journal_trade_executions (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  trade_id TEXT NOT NULL,
  source_system TEXT,
  source_execution_id TEXT,
  execution_type TEXT NOT NULL DEFAULT 'fill',
  side TEXT NOT NULL,
  executed_at TIMESTAMPTZ NOT NULL,
  quantity NUMERIC(38,18) NOT NULL CHECK (quantity > 0),
  price NUMERIC(38,18) NOT NULL CHECK (price > 0),
  commission NUMERIC(38,18) NOT NULL DEFAULT 0,
  swap NUMERIC(38,18) NOT NULL DEFAULT 0,
  other_fees NUMERIC(38,18) NOT NULL DEFAULT 0,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (source_system IS NULL OR source_execution_id IS NOT NULL),
  UNIQUE (trade_id, source_system, source_execution_id),
  FOREIGN KEY (owner_id, trade_id)
    REFERENCES journal_trades(owner_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS journal_trade_executions_owner_trade_time_idx
  ON journal_trade_executions (owner_id, trade_id, executed_at);

CREATE TABLE IF NOT EXISTS journal_trade_attachments (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  trade_id TEXT NOT NULL,
  attachment_type TEXT NOT NULL DEFAULT 'screenshot',
  storage_provider TEXT NOT NULL,
  storage_key TEXT NOT NULL,
  original_filename TEXT,
  content_type TEXT,
  byte_size BIGINT CHECK (byte_size IS NULL OR byte_size >= 0),
  captured_at TIMESTAMPTZ,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (owner_id, storage_provider, storage_key),
  FOREIGN KEY (owner_id, trade_id)
    REFERENCES journal_trades(owner_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS journal_trade_attachments_owner_trade_idx
  ON journal_trade_attachments (owner_id, trade_id);

CREATE TABLE IF NOT EXISTS journal_import_batches (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  journal_account_id TEXT NOT NULL,
  source_system TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  source_record_count BIGINT NOT NULL DEFAULT 0 CHECK (source_record_count >= 0),
  imported_count BIGINT NOT NULL DEFAULT 0 CHECK (imported_count >= 0),
  skipped_count BIGINT NOT NULL DEFAULT 0 CHECK (skipped_count >= 0),
  failed_count BIGINT NOT NULL DEFAULT 0 CHECK (failed_count >= 0),
  started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  completed_at TIMESTAMPTZ,
  reconciliation JSONB NOT NULL DEFAULT '{}'::jsonb,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  UNIQUE (owner_id, id),
  FOREIGN KEY (owner_id, journal_account_id)
    REFERENCES journal_accounts(owner_id, id)
);
CREATE INDEX IF NOT EXISTS journal_import_batches_owner_account_idx
  ON journal_import_batches (owner_id, journal_account_id, started_at DESC);

CREATE TABLE IF NOT EXISTS journal_trade_source_records (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  journal_account_id TEXT NOT NULL,
  trade_id TEXT,
  import_batch_id TEXT,
  source_system TEXT NOT NULL,
  source_record_id TEXT NOT NULL,
  source_position_id TEXT,
  source_deal_id TEXT,
  raw_symbol TEXT,
  source_opened_at TIMESTAMPTZ,
  source_closed_at TIMESTAMPTZ,
  source_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
  imported_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (journal_account_id, source_system, source_record_id),
  FOREIGN KEY (owner_id, journal_account_id)
    REFERENCES journal_accounts(owner_id, id),
  FOREIGN KEY (owner_id, trade_id)
    REFERENCES journal_trades(owner_id, id),
  FOREIGN KEY (owner_id, import_batch_id)
    REFERENCES journal_import_batches(owner_id, id)
);
CREATE INDEX IF NOT EXISTS journal_trade_source_records_owner_trade_idx
  ON journal_trade_source_records (owner_id, trade_id);
