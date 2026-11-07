-- Explicit tenant/user ownership boundary for dashboard read APIs.
-- Existing core trading accounts are intentionally not auto-granted to any user:
-- they predate the EdgeNest identity/account-access mapping.
CREATE TABLE IF NOT EXISTS dashboard_account_access (
    owner_id TEXT NOT NULL CHECK (length(trim(owner_id)) > 0),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    access_role TEXT NOT NULL DEFAULT 'owner'
        CHECK (access_role IN ('owner', 'trader', 'viewer')),
    granted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    granted_by TEXT,
    PRIMARY KEY (owner_id, account_id)
);

CREATE INDEX IF NOT EXISTS idx_dashboard_account_access_account
    ON dashboard_account_access(account_id);
