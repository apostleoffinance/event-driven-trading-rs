ALTER TABLE identity_users
    ADD COLUMN IF NOT EXISTS email_verified_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS mfa_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS mfa_secret_encrypted BYTEA;

CREATE TABLE IF NOT EXISTS identity_email_verification_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_identity_email_tokens_user
    ON identity_email_verification_tokens(user_id, expires_at DESC);

CREATE TABLE IF NOT EXISTS identity_password_reset_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_identity_reset_tokens_user
    ON identity_password_reset_tokens(user_id, expires_at DESC);

CREATE TABLE IF NOT EXISTS identity_mfa_recovery_codes (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_identity_mfa_recovery_user
    ON identity_mfa_recovery_codes(user_id) WHERE used_at IS NULL;

COMMENT ON COLUMN identity_users.mfa_secret_encrypted IS
    'TOTP seed encrypted with IDENTITY_MFA_ENCRYPTION_KEY; never store plaintext seed.';
