# EdgeNest Identity API

This is the EdgeNest-owned identity issuer. It does not migrate TraderOS users, password hashes, sessions, JWTs, refresh tokens, or MT5 auth.

## Required environment

- `DATABASE_URL`
- `EDGENEST_JWT_ISSUER`
- `EDGENEST_JWT_AUDIENCE`
- `EDGENEST_JWT_PRIVATE_KEY_PEM` (RSA private PEM, secret)
- `EDGENEST_JWT_PUBLIC_KEY_PEM` (matching RSA public PEM)
- `IDENTITY_MFA_ENCRYPTION_KEY` (random secret for encrypting TOTP seeds at rest)

Required for readiness and production account flows:
- `RESEND_API_KEY` (Resend API key)
- `IDENTITY_EMAIL_FROM` (verified sender address, e.g. `EdgeNest <auth@your-domain>`)
- `EDGENEST_PUBLIC_APP_URL` (real frontend base URL used in verification/reset links)

Optional:
- `EDGENEST_JWT_KEY_ID` (active signing KID; default `edgenest-rsa-2026-01`)
- `EDGENEST_JWT_PUBLIC_KEYS_JSON` (JSON array of `{kid,pem}` entries for active and overlapping old public keys)
- `IDENTITY_API_BIND` (default `0.0.0.0:8081`)
- `IDENTITY_CORS_ORIGINS` (comma-separated exact frontend origins)
- `EDGENEST_ACCESS_TOKEN_TTL_SECONDS` (default 900, 60–3600)
- `EDGENEST_REFRESH_TOKEN_TTL_DAYS` (default 30, 1–90)

Run with `cargo run -p identity-api`. The service runs migrations on startup; separate migration and runtime database roles in production.

## Endpoints

| Method | Endpoint | Purpose |
|---|---|---|
| POST | `/api/v1/auth/register` | Create account and send verification email |
| POST | `/api/v1/auth/verify-email` | Verify one-time email token |
| POST | `/api/v1/auth/resend-verification` | Resend verification (generic response) |
| POST | `/api/v1/auth/login` | Email/password login; MFA challenge if enabled |
| POST | `/api/v1/auth/mfa/verify-login` | Verify TOTP or recovery code |
| POST | `/api/v1/auth/refresh` | Rotate refresh cookie |
| POST | `/api/v1/auth/logout` | Revoke refresh cookie |
| GET | `/api/v1/auth/me` | Return authenticated account |
| POST | `/api/v1/auth/password-reset/request` | Request recovery link (generic response) |
| POST | `/api/v1/auth/password-reset/confirm` | Set password using one-time token |
| POST | `/api/v1/auth/mfa/setup` | Generate TOTP seed/otpauth URI |
| POST | `/api/v1/auth/mfa/confirm` | Confirm MFA and issue one-time recovery codes |
| POST | `/api/v1/auth/mfa/disable` | Disable MFA with current TOTP |
| GET | `/.well-known/jwks.json` | Public RSA JWKS |
| GET | `/healthz`, `/readyz` | Health/readiness |

## Security behavior

- Email verification required before password login.
- Passwords are Argon2id hashed; minimum 12 characters.
- Verification and reset links are single-use, hashed in storage, and time-limited.
- Password reset revokes all active refresh tokens.
- Refresh tokens are random opaque secrets, persisted as SHA-256 hashes, and rotated transactionally.
- Browser refresh token is only in an HttpOnly, Secure, SameSite=None `__Host-edgenest_refresh` cookie. Access token stays in memory in the frontend.
- TOTP seeds are AES-256-GCM encrypted at rest using `IDENTITY_MFA_ENCRYPTION_KEY`. Recovery codes are one-use hashes.
- JWTs are RS256 with `kid`. Identity and Journal API accept a configured public key ring for overlapping key rotation.
- Keep private signing PEM and MFA encryption key in KMS/secrets; never commit them or expose them in frontend env.

## Key rotation

1. Generate a new RSA key pair and a new KID.
2. Add the new public key and previous public key to `EDGENEST_JWT_PUBLIC_KEYS_JSON`.
3. Deploy the key ring to both Identity and Journal API.
4. Set Identity's active private key and `EDGENEST_JWT_KEY_ID` to the new key.
5. Keep the old public key until all old access tokens expire (at least maximum access TTL plus clock-skew margin).
6. Remove old key from the ring and redeploy.

JWKS publishes the active and overlapping public keys. Key IDs must be unique.

## Email delivery

Configure Resend with a verified sending domain and SPF/DKIM/DMARC. The app uses the Resend HTTPS API. Without `RESEND_API_KEY`, verification/reset messages cannot be delivered; do not enable production account creation without a working provider.

## Cloudflare

Rate limits are defined in `infra/cloudflare/identity-rate-limits.tf`. Apply via infrastructure CI with the correct Cloudflare zone ID. Keep the origin protected behind Cloudflare, and validate rule hits/false positives after deployment.
