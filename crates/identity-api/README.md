# EdgeNest Identity API

This is the EdgeNest-owned identity issuer. It does not migrate or depend on TraderOS user records, password hashes, sessions, JWTs, refresh tokens, or MT5 auth.

## Run

```bash
cargo run -p identity-api
```

Required:
- `DATABASE_URL`
- `EDGENEST_JWT_ISSUER`
- `EDGENEST_JWT_AUDIENCE`
- `EDGENEST_JWT_PRIVATE_KEY_PEM` (RSA private PEM; secret)
- `EDGENEST_JWT_PUBLIC_KEY_PEM` (matching RSA public PEM)

Optional:
- `IDENTITY_API_BIND` default `0.0.0.0:8081`
- `IDENTITY_CORS_ORIGINS` comma-separated browser origin allowlist
- `EDGENEST_ACCESS_TOKEN_TTL_SECONDS` default 900, clamped 60–3600
- `EDGENEST_REFRESH_TOKEN_TTL_DAYS` default 30, clamped 1–90
- `RUST_LOG`

The service runs the EdgeNest database migrations. Production should use separate migration and runtime DB roles where possible.

## API

| Method | Endpoint | Purpose |
|---|---|---|
| POST | `/api/v1/auth/register` | Register email/password user (password min 12 chars) |
| POST | `/api/v1/auth/login` | Authenticate and issue access + refresh tokens |
| POST | `/api/v1/auth/refresh` | Rotate refresh token and issue fresh token pair |
| POST | `/api/v1/auth/logout` | Revoke current user's refresh token (requires bearer access token) |
| GET | `/api/v1/auth/me` | Return authenticated identity |
| GET | `/healthz` | Liveness |
| GET | `/readyz` | DB readiness |

Access tokens use RS256 and include `sub` (immutable EdgeNest user ID), `iss`, `aud`, `iat`, `exp`, and `token_type=access`. Journal API validates the same issuer, audience, signature, expiry, and subject.

Refresh tokens are random opaque bearer values. Only SHA-256 hashes are persisted; refresh rotation consumes the old token in a single SQL update and stores a new token hash. Do not log tokens.

## Security/deployment checklist

- Keep private signing PEM in a secrets manager/KMS; never in Git, browser env, logs, or frontend builds.
- Configure the exact same issuer/audience/public PEM in Journal API.
- Enforce HTTPS only.
- Apply Cloudflare/WAF rate limiting to register/login/refresh and bot protection; this service does not yet include a distributed rate limiter.
- This first-party identity MVP does not yet implement email verification, password reset/recovery, MFA/passkeys, or admin user management. Do not describe those as available. Those flows require additional operational policy and email delivery integration before production onboarding is considered complete.
- Plan key rotation/JWKS with overlap before multiple signing keys or longer token lifetimes are needed.
- Access token TTL is short; refresh tokens are rotated and revocable. Refresh tokens should be stored securely by the client (prefer HttpOnly Secure SameSite cookies in browser apps; avoid localStorage).
