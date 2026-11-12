-- QOS-021A-E3A-R3B-A-I1: access tokens and refresh tokens are session-bound.
-- Existing refresh rows are revoked; no legacy session is inferred or created.

CREATE TABLE identity_sessions (
    id TEXT PRIMARY KEY CHECK (id ~ '^[0-9a-f]{64}$'),
    user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','REVOKED','EXPIRED')),
    issued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ,
    revocation_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (expires_at > issued_at),
    CHECK ((status = 'REVOKED') = (revoked_at IS NOT NULL)),
    CHECK ((status = 'REVOKED') = (revocation_reason IS NOT NULL)),
    UNIQUE (id,user_id)
);
CREATE INDEX identity_sessions_user_status_idx
    ON identity_sessions(user_id,status,expires_at DESC);

ALTER TABLE identity_refresh_tokens
    ADD COLUMN session_id TEXT,
    ADD COLUMN token_family_id TEXT,
    ADD CONSTRAINT identity_refresh_tokens_session_fk
        FOREIGN KEY (session_id,user_id) REFERENCES identity_sessions(id,user_id) ON DELETE RESTRICT,
    ADD CONSTRAINT identity_refresh_tokens_family_ck
        CHECK (token_family_id IS NULL OR token_family_id ~ '^[0-9a-f-]{36}$');

-- Existing refresh tokens have no session binding. Revoke them instead of
-- silently promoting them into a new session; users authenticate again.
UPDATE identity_refresh_tokens
   SET revoked_at=COALESCE(revoked_at,NOW()),
       last_used_at=COALESCE(last_used_at,NOW());

ALTER TABLE identity_refresh_tokens
    ADD CONSTRAINT identity_refresh_tokens_session_or_revoked_ck
        CHECK (session_id IS NOT NULL OR revoked_at IS NOT NULL);

CREATE INDEX identity_refresh_tokens_session_idx
    ON identity_refresh_tokens(session_id,expires_at DESC)
    WHERE revoked_at IS NULL;
CREATE INDEX identity_refresh_tokens_family_idx
    ON identity_refresh_tokens(token_family_id,created_at DESC);

CREATE TABLE identity_access_tokens (
    jti TEXT PRIMARY KEY CHECK (length(trim(jti)) BETWEEN 1 AND 128),
    session_id TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    session_revision BIGINT NOT NULL CHECK (session_revision > 0),
    issued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    revocation_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (session_id,user_id) REFERENCES identity_sessions(id,user_id) ON DELETE RESTRICT,
    CHECK (expires_at > issued_at),
    CHECK ((revoked_at IS NULL) = (revocation_reason IS NULL))
);
CREATE INDEX identity_access_tokens_session_idx
    ON identity_access_tokens(session_id,expires_at DESC);

CREATE TABLE identity_session_events (
    id BIGSERIAL PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES identity_sessions(id) ON DELETE RESTRICT,
    user_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    actor_user_id TEXT REFERENCES identity_users(id) ON DELETE RESTRICT,
    event_type TEXT NOT NULL CHECK (event_type IN (
        'SESSION_ISSUED','SESSION_REFRESHED','SESSION_REVOKED','SESSION_EXPIRED',
        'ACCESS_TOKEN_REVOKED','REFRESH_TOKEN_REUSE_REVOKED_SESSION'
    )),
    session_revision BIGINT NOT NULL CHECK (session_revision > 0),
    token_jti TEXT,
    token_family_id TEXT,
    reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 500),
    correlation_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX identity_session_events_history_idx
    ON identity_session_events(session_id,created_at DESC);

CREATE FUNCTION reject_identity_session_event_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'identity session events are append-only';
END
$$;
CREATE TRIGGER identity_session_events_no_update_delete
    BEFORE UPDATE OR DELETE ON identity_session_events
    FOR EACH ROW EXECUTE FUNCTION reject_identity_session_event_mutation();

COMMENT ON TABLE identity_sessions IS
    'Server-managed identity sessions. Access JWTs bind to this session id and revision.';
COMMENT ON TABLE identity_access_tokens IS
    'Server-side access-token allowlist and explicit revocation state keyed by JWT jti.';
COMMENT ON TABLE identity_session_events IS
    'Append-only security lifecycle events for sessions and access-token revocation.';
