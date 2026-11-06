-- EdgeNest-owned identity store. No TraderOS auth tables or credentials are imported.
CREATE TABLE IF NOT EXISTS identity_users (
    id                  TEXT PRIMARY KEY,
    email               TEXT NOT NULL UNIQUE CHECK (length(trim(email)) > 3),
    display_name        TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    password_hash       TEXT NOT NULL,
    is_active           BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_identity_users_email
    ON identity_users(email);

CREATE TABLE IF NOT EXISTS identity_refresh_tokens (
    id                  TEXT PRIMARY KEY,
    user_id             TEXT NOT NULL REFERENCES identity_users(id) ON DELETE CASCADE,
    token_hash          TEXT NOT NULL UNIQUE,
    expires_at          TIMESTAMPTZ NOT NULL,
    revoked_at          TIMESTAMPTZ,
    last_used_at        TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_identity_refresh_user_active
    ON identity_refresh_tokens(user_id, expires_at DESC)
    WHERE revoked_at IS NULL;

COMMENT ON TABLE identity_users IS
    'EdgeNest-owned identity records; do not import TraderOS auth credentials.';
COMMENT ON TABLE identity_refresh_tokens IS
    'Hashed, rotating EdgeNest refresh tokens; raw refresh secrets are never persisted.';
