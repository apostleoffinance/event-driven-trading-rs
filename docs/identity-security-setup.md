# Identity and Journal production setup

## 1. Generate signing and MFA secrets

Generate a strong RSA key pair (3072 bits recommended):

```bash
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:3072 -out edgenest-jwt-private.pem
openssl pkey -in edgenest-jwt-private.pem -pubout -out edgenest-jwt-public.pem
openssl rand -hex 32
```

The final command is a candidate for `IDENTITY_MFA_ENCRYPTION_KEY`. Store the RSA private PEM and MFA encryption key in a secret manager. Never commit them or place them in `NEXT_PUBLIC_*` variables.

Identity startup checks that the private/public key pair matches and the RSA modulus is at least 2048 bits.

## 2. Configure email delivery

Create a Resend API key and verify your sender domain. Configure SPF, DKIM and DMARC, then set:
- `RESEND_API_KEY`
- `IDENTITY_EMAIL_FROM` to your verified sender
- `EDGENEST_PUBLIC_APP_URL` to the real HTTPS frontend origin

Identity `/readyz` stays unavailable until email delivery configuration exists. This prevents the API from appearing production-ready while email verification and recovery cannot work.

## 3. Deploy Identity and Journal APIs

Both services use the same PostgreSQL database and run migrations at startup.

- Identity API default bind: `0.0.0.0:8081`
- Journal API default bind: `0.0.0.0:8082`

Set the same issuer and audience on both services. Journal API needs the Identity public key or public key ring, never the signing private key. Set exact frontend CORS origins and configure HTTPS at the edge.

## 4. Frontend configuration

Set build-time values:
- `NEXT_PUBLIC_IDENTITY_API_URL`
- `NEXT_PUBLIC_JOURNAL_API_URL`

Refresh tokens use an HttpOnly Secure cookie; access JWTs are held only in browser memory. Cross-origin requests need CORS credential support and the Identity cookie must remain host-only (no Domain attribute).

## 5. Cloudflare rate limiting

Set `CLOUDFLARE_API_TOKEN` and `cloudflare_zone_id` in infrastructure CI. Review the existing `http_ratelimit` phase first; Cloudflare permits one zone entry-point Ruleset per phase. If a rate-limit Ruleset already exists, import it and merge the EdgeNest rules instead of creating a second Ruleset.

Then:

```bash
cd infra/cloudflare
terraform init
terraform plan -var="cloudflare_zone_id=$CLOUDFLARE_ZONE_ID"
terraform apply -var="cloudflare_zone_id=$CLOUDFLARE_ZONE_ID"
```

The rules cover login/MFA, register/email verification, password reset and refresh endpoints. Keep origin ingress Cloudflare-only. Enable HTTPS redirect and HSTS at Cloudflare, and monitor WAF events/false positives after rollout.

## 6. JWT key rotation

Identity and Journal API accept `EDGENEST_JWT_PUBLIC_KEYS_JSON`, a JSON array of `{kid,pem}` public keys.

1. Add the new public key and current old public key to both services' key rings.
2. Switch Identity's active private key, public PEM and active `EDGENEST_JWT_KEY_ID`.
3. Verify Identity JWKS includes both keys and Journal API accepts tokens signed by the new key.
4. Retain the old public key for at least the maximum access-token lifetime plus clock-skew margin.
5. Remove the old key from the ring and redeploy.

Identity rejects duplicate KIDs and active key mismatch. Journal API rejects duplicate KIDs and challenge-token JWTs.

## 7. Production smoke tests

- Register a new user and confirm the email link.
- Sign in and verify refresh cookie attributes in browser devtools: `Secure`, `HttpOnly`, `Path=/`, `SameSite=None`, no `Domain`.
- Refresh, logout, then verify the old refresh token cannot be reused.
- Enroll TOTP, save recovery codes, verify MFA login, and confirm a recovery code can only be used once.
- Request password reset; use the link once; verify all existing refresh sessions are revoked.
- Check Identity JWKS and test Journal API with a valid access token and with an MFA challenge token (challenge must be rejected).
- Verify Cloudflare rules block burst traffic at configured thresholds.
