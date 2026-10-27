-- QOS-013: append-only audit of instrument-aware sizing decisions.
-- Decimal financial values are represented as strings inside JSON to avoid
-- binary floating-point conversion and preserve exact decimal text.
CREATE TABLE IF NOT EXISTS sizing_decision_audits (
    id                      BIGSERIAL PRIMARY KEY,
    trade_intent_id         TEXT NOT NULL REFERENCES trade_intents(id),
    account_id              TEXT NOT NULL REFERENCES accounts(id),
    instrument_id           TEXT NOT NULL REFERENCES instruments(id),
    source_id               TEXT NOT NULL,
    snapshot_captured_at    TIMESTAMPTZ NOT NULL,
    evaluated_at            TIMESTAMPTZ NOT NULL,
    payload_json            JSONB NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sizing_audits_intent
    ON sizing_decision_audits(trade_intent_id, account_id, created_at);
