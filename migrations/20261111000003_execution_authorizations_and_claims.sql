-- Durable authorization/claim records only. This schema does not issue or consume grants.
ALTER TABLE orders ADD CONSTRAINT orders_id_account_client_identity UNIQUE (id, account_id, client_order_id);
ALTER TABLE risk_decisions ADD CONSTRAINT risk_decisions_id_account_identity UNIQUE (id, account_id);

CREATE TABLE execution_authorizations (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) > 0),
    owner_id TEXT NOT NULL REFERENCES identity_users(id) ON DELETE RESTRICT,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    broker_id TEXT NOT NULL CHECK (length(trim(broker_id)) > 0),
    broker_server_id TEXT NOT NULL CHECK (length(trim(broker_server_id)) > 0),
    environment TEXT NOT NULL CHECK (environment IN ('PAPER','SHADOW','LIVE')),
    environment_revision BIGINT NOT NULL CHECK (environment_revision > 0),
    order_id TEXT NOT NULL REFERENCES orders(id) ON DELETE RESTRICT,
    canonical_order_sha256 TEXT NOT NULL CHECK (canonical_order_sha256 ~ '^[0-9a-f]{64}$'),
    client_order_id TEXT NOT NULL CHECK (length(trim(client_order_id)) > 0),
    risk_decision_id BIGINT NOT NULL REFERENCES risk_decisions(id) ON DELETE RESTRICT,
    nonce TEXT NOT NULL UNIQUE CHECK (length(trim(nonce)) > 0),
    state TEXT NOT NULL CHECK (state IN ('ISSUED','REVOKED','EXPIRED','CONSUMED')),
    issued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (expires_at > issued_at),
    CHECK ((state = 'CONSUMED') = (consumed_at IS NOT NULL)),
    UNIQUE (id, account_id, client_order_id)
    ,UNIQUE (id, account_id, client_order_id, order_id)
    ,FOREIGN KEY (owner_id, account_id) REFERENCES dashboard_account_access(owner_id, account_id) ON DELETE RESTRICT
    ,FOREIGN KEY (order_id, account_id, client_order_id) REFERENCES orders(id, account_id, client_order_id) ON DELETE RESTRICT
    ,FOREIGN KEY (risk_decision_id, account_id) REFERENCES risk_decisions(id, account_id) ON DELETE RESTRICT
);
CREATE INDEX execution_authorizations_account_state_idx
    ON execution_authorizations(account_id, state, expires_at);
CREATE INDEX execution_authorizations_order_idx ON execution_authorizations(order_id);

CREATE TABLE execution_claims (
    id TEXT PRIMARY KEY CHECK (length(trim(id)) > 0),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    client_order_id TEXT NOT NULL CHECK (length(trim(client_order_id)) > 0),
    authorization_id TEXT NOT NULL,
    order_id TEXT NOT NULL REFERENCES orders(id) ON DELETE RESTRICT,
    lifecycle_state TEXT NOT NULL CHECK (lifecycle_state IN ('CLAIMED','SUBMITTING','ACKNOWLEDGED','REJECTED','UNKNOWN','RECONCILIATION_REQUIRED')),
    submission_attempt_id TEXT UNIQUE,
    venue_correlation_id TEXT,
    outcome_detail JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(outcome_detail)='object'),
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_attempt_at TIMESTAMPTZ,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (authorization_id, account_id, client_order_id, order_id)
        REFERENCES execution_authorizations(id, account_id, client_order_id, order_id) ON DELETE RESTRICT,
    UNIQUE (account_id, client_order_id),
    CHECK ((lifecycle_state IN ('ACKNOWLEDGED','REJECTED') AND resolved_at IS NOT NULL)
        OR lifecycle_state NOT IN ('ACKNOWLEDGED','REJECTED')),
    CHECK (lifecycle_state NOT IN ('UNKNOWN','RECONCILIATION_REQUIRED') OR resolved_at IS NULL)
);
CREATE INDEX execution_claims_reconciliation_idx
    ON execution_claims(lifecycle_state, updated_at)
    WHERE lifecycle_state IN ('UNKNOWN','RECONCILIATION_REQUIRED');
