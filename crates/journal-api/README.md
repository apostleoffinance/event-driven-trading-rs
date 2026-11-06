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
| `EDGENEST_JWT_ISSUER` | Exact `iss` claim expected from EdgeNest Identity |
| `EDGENEST_JWT_AUDIENCE` | Exact API audience expected in JWT `aud` |
| `EDGENEST_JWT_PUBLIC_KEY_PEM` | RSA public key in PEM format (literal newlines or escaped `\\n`) |

Optional:
- `JOURNAL_API_BIND` defaults to `0.0.0.0:8080`.
- `JOURNAL_CORS_ORIGINS` is a comma-separated allowlist; default is no browser origins.
- `RUST_LOG` controls tracing.

The API applies existing SQL migrations on startup. Use a migration-capable deployment identity in production; consider separating migration execution from runtime DB permissions for a hardened deployment.

## EdgeNest JWT contract

- EdgeNest Identity issues tokens; this API is a relying party.
- RS256 only. The service pins the algorithm and validates signature, `exp`, `iss`, and `aud`.
- Required claims: `sub`, `iss`, `aud`, `exp`. `sub` is the canonical immutable EdgeNest user ID.
- Owner/tenant scope is derived exclusively from verified `sub`; payloads and query strings cannot override owner ID.
- Keep the RSA private signing key in EdgeNest Identity secret storage/KMS. Only the public PEM belongs in this API's configuration.
- Rotate keys deliberately (future JWKS support should overlap old/new keys during token TTL). Do not accept TraderOS tokens or reuse its auth infrastructure.
- No API keys, refresh-token endpoints, password storage, or login UI are implemented by this Journal service.

The identity issuer itself is not present in this repo yet. Deploy this API only once the EdgeNest Identity issuer emits tokens conforming to the contract above. Issuer/audience/key values are environment-configured; do not guess production values.

## Routes

All `/api/v1/journal/**` endpoints require `Authorization: Bearer <EdgeNest JWT>`.

| Method | Route | Notes |
|---|---|---|
| GET | `/api/v1/journal/trades?journal_account_id=&limit=&offset=` | Owner scoped, limit max 500 |
| POST | `/api/v1/journal/trades` | Creates a manual/import journal trade |
| GET | `/api/v1/journal/trades/:id` | Owner scoped |
| PUT / PATCH | `/api/v1/journal/trades/:id` | Full replacement payload (PATCH is currently an alias; send all fields) |
| DELETE | `/api/v1/journal/trades/:id` | Owner scoped, returns 204 |
| GET | `/healthz` | Liveness |
| GET | `/readyz` | DB readiness |

Create/update body fields follow the Rust JournalTrade domain: `journal_account_id`, `setup_id`, `symbol`, `direction` (`long`/`short`), `status` (`open`/`closed`/`cancelled`), `source`, source identity, timestamps, quantity, prices, P&L, fees, currency, risk amount, and R-multiple. Do not send `owner_id` or `id`; the API assigns both from verified identity / server UUID.

Journal account must already exist and belong to the authenticated owner. Cross-tenant lookups return 404. This API writes only `journal_trades`; it does not create/modify OMS orders, fills, positions, or execution state.

## Important current limitations

- Journal account/setup CRUD is not exposed by this service yet; create them through the current EdgeNest account/setup service or a follow-up API implementation.
- JWT key discovery/JWKS and key rotation are not implemented; the API accepts one configured RSA public key.
- `PATCH` is full replacement, not partial JSON Merge Patch.
- Screenshot upload/storage and journal analytics endpoints are not part of this API phase.
