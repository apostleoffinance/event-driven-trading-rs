-- Phase 8 initial schema: institutional trading persistence
-- Money columns use NUMERIC (never floating point).

CREATE TABLE IF NOT EXISTS venues (
    id              TEXT PRIMARY KEY,
    venue_type      TEXT NOT NULL,
    name            TEXT NOT NULL,
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS instruments (
    id              TEXT PRIMARY KEY,
    instrument_type TEXT NOT NULL,
    symbol          TEXT NOT NULL,
    base_asset      TEXT,
    quote_asset     TEXT,
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    tick_size       NUMERIC(38, 18),
    lot_size        NUMERIC(38, 18),
    min_quantity    NUMERIC(38, 18),
    min_notional    NUMERIC(38, 18),
    price_precision INTEGER,
    quantity_precision INTEGER
);

CREATE TABLE IF NOT EXISTS accounts (
    id              TEXT PRIMARY KEY,
    account_type    TEXT NOT NULL,
    status          TEXT NOT NULL,
    venue_id        TEXT NOT NULL REFERENCES venues(id),
    starting_capital NUMERIC(38, 18) NOT NULL,
    label           TEXT,
    created_at      TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS account_states (
    account_id          TEXT PRIMARY KEY REFERENCES accounts(id),
    balance             NUMERIC(38, 18) NOT NULL,
    equity              NUMERIC(38, 18) NOT NULL,
    used_margin         NUMERIC(38, 18) NOT NULL,
    open_position_count INTEGER NOT NULL,
    daily_realized_pnl  NUMERIC(38, 18) NOT NULL,
    peak_equity         NUMERIC(38, 18) NOT NULL,
    kill_switch         BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at          TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS account_snapshots (
    id              BIGSERIAL PRIMARY KEY,
    account_id      TEXT NOT NULL REFERENCES accounts(id),
    status          TEXT NOT NULL,
    balance         NUMERIC(38, 18) NOT NULL,
    equity          NUMERIC(38, 18) NOT NULL,
    used_margin     NUMERIC(38, 18) NOT NULL,
    open_position_count INTEGER NOT NULL,
    daily_realized_pnl NUMERIC(38, 18) NOT NULL,
    peak_equity     NUMERIC(38, 18) NOT NULL,
    kill_switch     BOOLEAN NOT NULL,
    captured_at     TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS strategies (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS strategy_versions (
    strategy_id     TEXT NOT NULL REFERENCES strategies(id),
    version         TEXT NOT NULL,
    PRIMARY KEY (strategy_id, version)
);

CREATE TABLE IF NOT EXISTS risk_profiles (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    policy_json     JSONB NOT NULL
);

CREATE TABLE IF NOT EXISTS strategy_deployments (
    id                  TEXT PRIMARY KEY,
    strategy_id         TEXT NOT NULL,
    strategy_version    TEXT NOT NULL,
    account_id          TEXT NOT NULL REFERENCES accounts(id),
    risk_profile_id     TEXT NOT NULL REFERENCES risk_profiles(id),
    enabled             BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (strategy_id, strategy_version)
        REFERENCES strategy_versions(strategy_id, version)
);

CREATE TABLE IF NOT EXISTS trade_intents (
    id                  TEXT PRIMARY KEY,
    strategy_id         TEXT NOT NULL,
    strategy_version    TEXT NOT NULL,
    deployment_id       TEXT NOT NULL REFERENCES strategy_deployments(id),
    instrument_id       TEXT NOT NULL REFERENCES instruments(id),
    side                TEXT NOT NULL,
    target_quantity     NUMERIC(38, 18),
    target_notional     NUMERIC(38, 18),
    entry_price         NUMERIC(38, 18),
    stop_loss           NUMERIC(38, 18),
    take_profit         NUMERIC(38, 18),
    confidence          NUMERIC(38, 18),
    created_at          TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS risk_decisions (
    id                  BIGSERIAL PRIMARY KEY,
    trade_intent_id     TEXT NOT NULL REFERENCES trade_intents(id),
    account_id          TEXT NOT NULL REFERENCES accounts(id),
    decision_type       TEXT NOT NULL,
    quantity            NUMERIC(38, 18),
    reason              TEXT,
    decision_json       JSONB NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS orders (
    id                  TEXT PRIMARY KEY,
    client_order_id     TEXT NOT NULL UNIQUE,
    account_id          TEXT NOT NULL REFERENCES accounts(id),
    venue_id            TEXT NOT NULL REFERENCES venues(id),
    deployment_id       TEXT NOT NULL,
    strategy_id         TEXT NOT NULL,
    trade_intent_id     TEXT REFERENCES trade_intents(id),
    instrument_id       TEXT NOT NULL REFERENCES instruments(id),
    side                TEXT NOT NULL,
    order_type          TEXT NOT NULL,
    time_in_force       TEXT NOT NULL,
    quantity            NUMERIC(38, 18) NOT NULL,
    price               NUMERIC(38, 18),
    filled_quantity     NUMERIC(38, 18) NOT NULL,
    status              TEXT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL,
    updated_at          TIMESTAMPTZ NOT NULL,
    submitted_at        TIMESTAMPTZ,
    filled_at           TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_orders_account ON orders(account_id);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);

CREATE TABLE IF NOT EXISTS fills (
    id              TEXT PRIMARY KEY,
    order_id        TEXT NOT NULL REFERENCES orders(id),
    instrument_id   TEXT NOT NULL REFERENCES instruments(id),
    price           NUMERIC(38, 18) NOT NULL,
    quantity        NUMERIC(38, 18) NOT NULL,
    fee             NUMERIC(38, 18) NOT NULL,
    filled_at       TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_fills_order ON fills(order_id);

CREATE TABLE IF NOT EXISTS positions (
    id                  TEXT PRIMARY KEY,
    account_id          TEXT NOT NULL REFERENCES accounts(id),
    instrument_id       TEXT NOT NULL REFERENCES instruments(id),
    side                TEXT NOT NULL,
    quantity            NUMERIC(38, 18) NOT NULL,
    avg_entry_price     NUMERIC(38, 18) NOT NULL,
    stop_loss           NUMERIC(38, 18),
    mark_price          NUMERIC(38, 18) NOT NULL,
    realized_pnl        NUMERIC(38, 18) NOT NULL,
    opened_at           TIMESTAMPTZ NOT NULL,
    updated_at          TIMESTAMPTZ NOT NULL,
    UNIQUE (account_id, instrument_id)
);

CREATE TABLE IF NOT EXISTS performance_snapshots (
    id              BIGSERIAL PRIMARY KEY,
    account_id      TEXT NOT NULL REFERENCES accounts(id),
    equity          NUMERIC(38, 18) NOT NULL,
    balance         NUMERIC(38, 18) NOT NULL,
    realized_pnl    NUMERIC(38, 18) NOT NULL,
    unrealized_pnl  NUMERIC(38, 18) NOT NULL,
    drawdown_pct    NUMERIC(38, 18) NOT NULL,
    captured_at     TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_events (
    id                  BIGSERIAL PRIMARY KEY,
    order_id            TEXT,
    client_order_id     TEXT,
    account_id          TEXT,
    trade_intent_id     TEXT,
    kind                TEXT NOT NULL,
    detail              TEXT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS execution_events (
    id                  BIGSERIAL PRIMARY KEY,
    order_id            TEXT,
    venue_id            TEXT,
    event_type          TEXT NOT NULL,
    payload_json        JSONB NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL
);
