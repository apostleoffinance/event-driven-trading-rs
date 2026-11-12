# EdgeNest Journal API

This service owns Journal HTTP CRUD in EdgeNest. It is independent from TraderOS and does not read/migrate TraderOS users, sessions, JWTs, refresh tokens, password hashes, MT5 tokens, or trading records. EdgeNest identity is the source of truth.

## Runtime

Run as the workspace binary:

```bash
cargo run -p journal-api
```

Required environment:

| Variable | Meaning |
|---|---|
| `DATABASE_URL` | EdgeNest PostgreSQL connection string |
| `MIGRATOR_DATABASE_URL` | Separate schema-owner/migration connection used only during startup migrations; the pool is closed before the runtime connection is opened |
| `EDGENEST_JWT_ISSUER` | Exact `iss` claim expected from EdgeNest Identity |
| `EDGENEST_JWT_AUDIENCE` | Exact API audience expected in JWT `aud` |
| `EDGENEST_JWT_VERIFICATION_KEY_FILES` | Semicolon-separated `kid=/absolute/path/public.pem` verification key ring; include active and overlapping old public keys |

Optional:
- `JOURNAL_API_BIND` defaults to `0.0.0.0:8080`.
- `JOURNAL_CORS_ORIGINS` is a comma-separated allowlist; default is no browser origins.
- `RUST_LOG` controls tracing.

The API applies forward-only SQL migrations through `MIGRATOR_DATABASE_URL`, closes that pool, then connects through `DATABASE_URL`. The runtime role must not own or inherit the owner role for `identity_session_events`; it must have SELECT/INSERT only for that table, no TRIGGER or schema CREATE privilege, and must not be superuser, CREATEROLE, or CREATEDB. Startup fails closed unless the full supported migration history and required grants are present. Identity and Journal binaries must be rolled together across the I1-R1 migration boundary; old binaries are not supported against the upgraded history. There is no down migration: rollback means a forward fix or separately approved restore. Existing users must reauthenticate because I1 revoked legacy refresh tokens and suspension invalidates prior sessions.

## EdgeNest JWT contract

- EdgeNest Identity issues tokens; this API is a relying party.
- RS256 only. The service validates `kid`, signature, `exp`, `iss`, `aud`, and the access-token type.
- Required claims: `sub`, `iss`, `aud`, `exp`, `iat`, `jti`, `sid`, and `srv`. `sub` is the canonical immutable EdgeNest user ID; `sid` and `srv` bind the token to a server-managed session and revision.
- Every `/api/` request also checks the user's active state, session state and expiry, session revision, and token allowlist/revocation record in PostgreSQL. Database errors fail closed with service unavailable; revoked or stale sessions return unauthorized.
- Owner/tenant scope is derived exclusively from verified `sub`; payloads and query strings cannot override owner ID.
- Keep the active RSA private signing key in externally managed secret storage; only public key files belong in this API's configuration. The service does not implement KMS/HSM retrieval.
- To rotate, add the new public key to this verifier before switching the issuer's active `kid`; retain the old public key through the maximum access-token lifetime and deployment overlap, then remove it. Emergency removal invalidates tokens signed by that key. Do not accept TraderOS tokens or reuse its auth infrastructure.
- No API keys, refresh-token endpoints, password storage, or login UI are implemented by this Journal service.

The EdgeNest Identity issuer is implemented in the sibling `identity-api` workspace service. Deploy both with identical issuer/audience/public-key configuration. Do not guess production values.

## Routes

All `/api/**` application endpoints require `Authorization: Bearer <EdgeNest JWT>` bound to an active server-managed identity session; `/healthz` and `/readyz` remain health probes.

| Method | Route | Notes |
|---|---|---|
| GET | `/api/v1/journal/accounts` | List active journal accounts |
| POST | `/api/v1/journal/accounts` | Create journal account |
| GET | `/api/v1/journal/accounts/{id}` | Owner-scoped account detail |
| PUT / PATCH | `/api/v1/journal/accounts/{id}` | Replace account attributes |
| DELETE | `/api/v1/journal/accounts/{id}` | Archive account (no destructive delete) |
| GET | `/api/v1/journal/setups` | List active trading setups |
| POST | `/api/v1/journal/setups` | Create setup |
| GET | `/api/v1/journal/setups/{id}` | Owner-scoped setup detail |
| PUT / PATCH | `/api/v1/journal/setups/{id}` | Replace setup attributes |
| DELETE | `/api/v1/journal/setups/{id}` | Archive setup |
| GET | `/api/v1/journal/trades?journal_account_id=&limit=&offset=` | Owner scoped, limit max 500 |
| POST | `/api/v1/journal/trades` | Creates a manual/import journal trade |
| GET | `/api/v1/journal/trades/{id}` | Owner scoped |
| PUT / PATCH | `/api/v1/journal/trades/{id}` | Full replacement payload (PATCH is currently an alias; send all fields) |
| DELETE | `/api/v1/journal/trades/{id}` | Owner scoped, returns 204 |
| GET / POST | `/api/v1/journal/trades/{trade_id}/attachments` | Owner-scoped attachment metadata / screenshot URL reference |
| DELETE | `/api/v1/journal/trades/{trade_id}/attachments/{attachment_id}` | Delete attachment metadata only |
| GET | `/api/v1/journal/trades/{trade_id}/psychology` | Owner-scoped psychology detail |
| PUT | `/api/v1/journal/trades/{trade_id}/psychology` | Upsert emotion/rating annotations (ratings 0–10) |
| POST | `/api/v1/journal/import` | Transactional idempotent legacy/TraderOS import with source lineage |
| GET | `/api/v1/journal/analytics/summary` | Read-only account/date-filtered closed-trade analytics |
| GET | `/healthz` | Liveness |
| GET | `/readyz` | DB readiness |

Create/update body fields follow the Rust JournalTrade domain: `journal_account_id`, `setup_id`, `symbol`, `direction` (`long`/`short`), `status` (`open`/`closed`/`cancelled`), `source`, source identity, timestamps, quantity, prices, P&L, fees, currency, risk amount, and R-multiple. Do not send `owner_id` or `id`; the API assigns both from verified identity / server UUID.

Journal account must already exist and belong to the authenticated owner. Cross-tenant lookups return 404. This API writes only `journal_trades`; it does not create/modify OMS orders, fills, positions, or execution state.

## Important current limitations

- Journal account/setup CRUD is available in this service. These are separate journal entities and do not create EdgeNest OMS accounts or broker connections.
- JWT keys are configured as a local verification key ring; network JWKS discovery and automatic key rollover are not implemented.
- `PATCH` is full replacement, not partial JSON Merge Patch.
- Screenshot URL/attachment metadata is available. Binary upload to R2/S3 is not implemented yet; object storage needs a separately configured upload service. Deleting metadata does not delete the external file.
- Analytics endpoint is descriptive only; it does not predict outcomes or authorize execution.
- Legacy CSV import requires a stable source trade ID for deduplication and must not contain broker credentials or tokens.
