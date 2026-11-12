# EdgeNest Identity API

This is the EdgeNest-owned identity issuer. It does not migrate or depend on TraderOS user records, password hashes, sessions, JWTs, refresh tokens, or MT5 auth.

## Run

```bash
cargo run -p identity-api
```

Required:
- `DATABASE_URL`
- `MIGRATOR_DATABASE_URL` (separate schema-owner/migration principal; runtime startup applies migrations through this connection, then closes it)
- `EDGENEST_JWT_ISSUER`
- `EDGENEST_JWT_AUDIENCE`
- `EDGENEST_JWT_ACTIVE_KID` (identifier for the one private signing key loaded by this issuer)
- `EDGENEST_JWT_SIGNING_KEY_FILES` (semicolon-separated `kid=/absolute/path/private.pem`; must contain only the active private key)
- `EDGENEST_JWT_VERIFICATION_KEY_FILES` (semicolon-separated `kid=/absolute/path/public.pem`; active key plus any overlapping retired verification keys)

Optional:
- `IDENTITY_API_BIND` default `0.0.0.0:8081`
- `IDENTITY_CORS_ORIGINS` comma-separated browser origin allowlist
- `EDGENEST_ACCESS_TOKEN_TTL_SECONDS` default 900, clamped 60–3600
- `EDGENEST_REFRESH_TOKEN_TTL_DAYS` default 30, clamped 1–90
- `RUST_LOG`

Startup applies forward-only EdgeNest migrations through `MIGRATOR_DATABASE_URL`, closes that pool, then connects through `DATABASE_URL`. The runtime role must not own or inherit the owner role for `identity_session_events`; it must have SELECT/INSERT only for that table, no TRIGGER or schema CREATE privilege, and must not be superuser, CREATEROLE, or CREATEDB. Startup fails closed when the database is not at the complete supported schema or these grants are not in place. Identity and Journal binaries must be rolled together across the I1-R1 migration boundary; old binaries are not supported against the upgraded migration history. The forward-only migration cannot be rolled back by an older binary: recovery is forward-fix or a separately approved restore, and sessions must be reauthenticated after cutover.

## API

| Method | Endpoint | Purpose |
|---|---|---|
| POST | `/api/v1/auth/register` | Register email/password user (password min 12 chars) |
| POST | `/api/v1/auth/login` | Authenticate and issue access + refresh tokens |
| POST | `/api/v1/auth/refresh` | Rotate refresh token and issue fresh token pair |
| POST | `/api/v1/auth/logout` | Revoke current session and its tokens (requires bearer access token) |
| POST | `/api/v1/auth/access-token/revoke` | Revoke the current access token only |
| GET | `/api/v1/auth/me` | Return authenticated identity |
| GET | `/healthz` | Liveness |
| GET | `/readyz` | DB readiness |

Access tokens use RS256 and include `sub` (immutable EdgeNest user ID), `iss`, `aud`, `iat`, `exp`, `token_type=access`, `jti`, `sid` (server-managed session ID), and `srv` (session revision). A database allowlist row binds each token ID to its user and session. Journal API validates the signature and claims, then checks the live user, session revision/state, token expiry, and revocation record on every API request.

Refresh tokens are random opaque bearer values. Only SHA-256 hashes are persisted. Refresh rotation, session updates, access-token issuance, and audit events are transactional. Reusing a consumed refresh token revokes its active session. Logout revokes the session and all its refresh/access tokens; `/api/v1/auth/access-token/revoke` revokes the current access token. Existing unbound refresh tokens are revoked by the forward-only I1 migration; users holding them must authenticate again.

## Security/deployment checklist

- Keep the one active private signing PEM in externally managed secret storage with restrictive file permissions; never in Git, browser env, logs, or frontend builds. This service reads configured files and does not itself integrate with KMS/HSM custody.
- Configure the exact same issuer/audience and all currently trusted public keys in Journal API.
- Enforce HTTPS only.
- Apply Cloudflare/WAF rate limiting to register/login/refresh and bot protection; this service does not yet include a distributed rate limiter.
- This first-party identity MVP does not yet implement email verification, password reset/recovery, MFA/passkeys, or admin user management. Do not describe those as available. Those flows require additional operational policy and email delivery integration before production onboarding is considered complete.
- For rotation, distribute the new public key to all verifiers first, then switch the issuer's active `kid` and private-key file and restart the issuer. Keep the old public key trusted for at least the maximum access-token lifetime plus deployment overlap. Remove it after overlap. Emergency compromise response removes the compromised key immediately and invalidates its outstanding access tokens. A rollback may restore the prior configuration only while its private key remains trusted and uncompromised; retain both verification keys during the rollback window. Missing/invalid active key configuration fails startup closed. Journal verification is a configured key ring, not JWKS.
- Access token TTL is short; refresh tokens are rotated and revocable. Refresh tokens should be stored securely by the client (prefer HttpOnly Secure SameSite cookies in browser apps; avoid localStorage).
