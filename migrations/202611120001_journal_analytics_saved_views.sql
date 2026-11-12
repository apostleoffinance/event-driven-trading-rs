-- Owner-scoped Analytics Lab saved views. Preferences are account-scoped and portable across devices.
CREATE TABLE IF NOT EXISTS journal_analytics_saved_views (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL CHECK (length(trim(owner_id)) > 0),
    scope_key TEXT NOT NULL CHECK (length(trim(scope_key)) > 0),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    filters JSONB NOT NULL CHECK (jsonb_typeof(filters) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (owner_id, scope_key, name)
);

CREATE INDEX IF NOT EXISTS journal_analytics_saved_views_owner_scope_idx
    ON journal_analytics_saved_views (owner_id, scope_key, updated_at DESC);
