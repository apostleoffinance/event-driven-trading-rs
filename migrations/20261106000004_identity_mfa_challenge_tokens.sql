CREATE TABLE IF NOT EXISTS identity_mfa_challenges (
    jti_hash TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_identity_mfa_challenges_user
    ON identity_mfa_challenges(user_id, expires_at DESC);
