-- TraderOS migration account provenance/risk profile metadata.
ALTER TABLE journal_accounts
  ADD COLUMN IF NOT EXISTS migration_metadata JSONB NOT NULL DEFAULT '{}'::jsonb
    CHECK (jsonb_typeof(migration_metadata) = 'object');
