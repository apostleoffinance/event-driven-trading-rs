-- Account-level customizable prop-firm/personal risk policy.
-- Policies are tenant scoped and attached to Journal accounts (calculator account context).
CREATE TABLE IF NOT EXISTS journal_account_risk_policies (
    owner_id        TEXT NOT NULL,
    account_id      TEXT NOT NULL,
    policy_json     JSONB NOT NULL CHECK (jsonb_typeof(policy_json) = 'object'),
    version         INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (owner_id, account_id),
    FOREIGN KEY (owner_id, account_id)
        REFERENCES journal_accounts(owner_id, id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_journal_account_risk_policy_updated
    ON journal_account_risk_policies(owner_id, updated_at DESC);
