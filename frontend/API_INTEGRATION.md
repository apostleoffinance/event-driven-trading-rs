# EdgeNest Frontend API Configuration

The browser communicates with backend services only through same-origin Next.js BFF routes. Backend origins and credentials must remain server-side; the browser must not receive access or refresh tokens.

## Server-side origins

Configure the following Worker environment variables in the Cloudflare dashboard (or as non-public Wrangler variables) for the deployed frontend Worker. The BFF reads them from the OpenNext Cloudflare runtime context:

| Variable | Service |
|---|---|
| `EDGENEST_API_URL` | Organization/workspace and trading gateway |
| `EDGENEST_IDENTITY_API_URL` | Identity API |
| `EDGENEST_JOURNAL_API_URL` | Journal API |
| `EDGENEST_MARKET_DATA_API_URL` | Market Data API |

For local `next dev`, the BFF can also read matching `EDGENEST_*` values from the local process environment. A temporary fallback to legacy `NEXT_PUBLIC_*` origin variables remains for compatibility; remove those public variables once the Worker variables are configured. These values are origins only, with no path prefix.

Never put private JWT keys, database URLs, broker/MT5 credentials, venue secrets, or privileged tokens in frontend variables.

## BFF and session security

- Browser requests use `/api/bff/{identity|journal|market-data|gateway}/...` and remain same-origin.
- Access and refresh tokens are stored only in `HttpOnly`, `SameSite=Lax` cookies. Cookies use `Secure` on HTTPS.
- The BFF proxies only explicit service route prefixes; it does not act as an arbitrary URL proxy.
- Mutating requests require a same-origin `Origin` (when supplied) and a matching double-submit CSRF cookie/header.
- Obtain the CSRF token with `GET /api/auth/csrf`; send it as `X-CSRF-Token` on POST/PUT/PATCH/DELETE requests.
- Login, registration, refresh, and logout are handled by the BFF. Token material is never returned to browser JavaScript.
- The BFF rotates refresh tokens after an upstream 401 and retries the original request once.
- The small user profile cached in tab-scoped sessionStorage is a UI hint only, not an authentication credential or authorization signal. Backend authorization always depends on the HttpOnly session cookies and verified JWT.
- Logout revokes the identity session and clears both HttpOnly cookies.

The BFF must be deployed on the same origin as the frontend. Direct browser calls to backend service origins are not part of the production architecture.

## API route contracts

Identity:
- `POST /api/v1/auth/register`
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/refresh`
- `POST /api/v1/auth/logout`
- `GET /api/v1/auth/me`

Journal:
- `/api/v1/journal/*` (JWT-authenticated)

Market Data:
- `/api/v1/market-data/*` (read-only)

Organization/workspace gateway:
- `/api/v1/organizations/*`
- `/api/v1/trading/*`

## API mode behavior

- The frontend never substitutes seeded dashboard fixtures.
- API failures remain visible; the frontend does not silently fall back to mocks.
- Market Data classification/freshness must be checked; a response is not automatically production/live feed data.
- The dashboard is read-only; these endpoints do not submit, cancel, or modify orders.

## Validation

Run from `frontend/`:

```bash
npm ci
npm run typecheck
npm run build
npm run cloudflare:build
npm run cloudflare:preview
```

Test login/register, CSRF rejection, authenticated organization/workspace requests, Journal and Market Data access, expired access-token refresh, refresh-token replay, logout, and unavailable backend behavior. Verify no token appears in browser storage, JavaScript responses, URLs, or logs.
