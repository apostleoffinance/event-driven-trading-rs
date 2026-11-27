# EdgeNest Frontend API Configuration

The Next.js frontend consumes Rust APIs through a same-origin Backend-for-Frontend (BFF) running in the Cloudflare Worker. Browser requests target `/api/bff/*`; the Worker forwards requests to the Rust services and attaches the bearer token server-side. The server-only API origins below are runtime Worker variables and must never use the `NEXT_PUBLIC_` prefix.

## Origins

## Data source

The frontend is API-only. It does not load seeded or fabricated trading records. If an API is unavailable, the interface displays a loading, error, or empty state rather than substituting sample data.


Configure the following server-only Worker variables for the BFF:

| Variable | Service | Required routes |
|---|---|---|
| `API_GATEWAY_ORIGIN` | Server-only shared gateway | `/api/v1/organizations/*`, `/api/v1/trading/*` |
| `IDENTITY_API_ORIGIN` | Server-only Identity API (falls back to gateway) | `/api/v1/auth/*` |
| `JOURNAL_API_ORIGIN` | Server-only Journal API (falls back to gateway) | `/api/v1/journal/*` |
| `MARKET_DATA_API_ORIGIN` | Server-only Market Data API (falls back to gateway) | `/api/v1/market-data/*` |


Set the four server-only origins in Cloudflare Worker Settings → Variables and Secrets. They must be HTTPS origins in production and should not include a path prefix unless the gateway is configured to accept the appended service paths. For local development, loopback HTTP origins are permitted only when the frontend is served from `localhost` or `127.0.0.1`. The public health indicator also uses the same-origin BFF, so no API origin is exposed in the browser bundle.

Set:
```env
NEXT_PUBLIC_ENVIRONMENT=PAPER

# Server-only values; use .dev.vars locally and Cloudflare Worker Variables in production.
API_GATEWAY_ORIGIN=https://<gateway-api-host>
IDENTITY_API_ORIGIN=https://<identity-api-host>
JOURNAL_API_ORIGIN=https://<journal-api-host>
MARKET_DATA_API_ORIGIN=https://<market-data-api-host>
```

Do not put private JWT keys, refresh-token signing keys, database URLs, broker/MT5 credentials, venue secrets, or privileged tokens in any `NEXT_PUBLIC_*` variable.

## Existing service route contracts

Identity:
- `POST /api/v1/auth/register`
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/refresh`
- `POST /api/v1/auth/logout`
- `GET /api/v1/auth/me`

Journal (JWT bearer):
- `GET/POST /api/v1/journal/accounts`
- `GET/POST /api/v1/journal/setups`
- `GET/POST /api/v1/journal/trades`
- `GET/PUT/PATCH/DELETE /api/v1/journal/trades/:trade_id`
- `GET /api/v1/journal/analytics/summary?journal_account_id=...&from=...&to=...`
- `GET /api/v1/journal/analytics/intelligence?journal_account_id=...&from=...&to=...`
- `GET/PUT /api/v1/journal/trades/:trade_id/psychology`
- `GET/POST/DELETE /api/v1/journal/trades/:trade_id/attachments`
- Analytics summary/intelligence accept optional `symbol` and `setup_id` filters. Trade listing accepts `symbol`, `setup_id`, and optional `closed_from`/`closed_to` UTC timestamps for date-bounded closed-trade drill-down.
- Analytics saved views are persisted through `GET/POST /api/v1/journal/analytics/saved-views` and `DELETE /api/v1/journal/analytics/saved-views/:id`. The API derives owner scope from the verified JWT and validates account ownership.

Market Data (read-only):
- `GET /api/v1/market-data/providers`
- `GET /api/v1/market-data/instruments`
- `GET /api/v1/market-data/candles?instrument=EURUSD&timeframe=M1&limit=100`
- `GET /api/v1/market-data/mt5/latest`

The frontend exposes sign-in/register at `/login`, Journal at `/journal`, and read-only Market Data at `/market-data`.

## CORS requirements

All browser-to-API traffic, including the health check, is same-origin through the BFF and does not require cross-origin CORS. Keep the exact-origin allow-list for approved external clients and direct service integrations. Use the exact browser-visible frontend origin (scheme + hostname, no path or trailing slash). Never use `*`.

For any approved external browser client that calls the combined API gateway directly (not through the EdgeNest BFF), configure its exact origin on the gateway service:
- `EDGENEST_CORS_ORIGINS=https://<external-client-host>`

The gateway answers those external clients’ `OPTIONS` preflight requests and validates the origin, requested method, and requested headers before proxying the actual request. Its allow-list includes the methods used by the APIs and the headers `Authorization`, `Content-Type`, `Accept`, and `X-Organization-Id`, and `X-Workspace-Id`. The workspace selector is required on workspace-scoped trading requests and is not authorization evidence. The gateway passes the configured origin list to its child API processes as `IDENTITY_CORS_ORIGINS`, `JOURNAL_CORS_ORIGINS`, and `MARKET_DATA_CORS_ORIGINS`.

If approved external clients call the Identity, Journal, or Market Data services directly rather than through the combined gateway, configure the corresponding service allow-list on each service:
- `IDENTITY_CORS_ORIGINS`
- `JOURNAL_CORS_ORIGINS`
- `MARKET_DATA_CORS_ORIGINS`

The browser does not send bearer tokens or call the Rust service origins directly. The BFF attaches the bearer token to its server-to-server request. Do not enable credentialed wildcard CORS as a substitute for the BFF.

## Auth session notes

Access and refresh tokens are held only in host-only `HttpOnly` cookies managed by the BFF. The browser cannot read them. Refresh occurs only through the managed `/api/bff/session` endpoint; direct browser `/auth/me` and `/auth/refresh` proxy routes are blocked. The frontend serializes session checks with the browser Web Locks API across tabs, while the Worker coalesces duplicate refresh attempts within an isolate. This prevents ordinary parallel API requests from replaying one rotating refresh token. The BFF rotates both cookies and clears them when the refresh token is invalid or logout is requested. The browser `sessionStorage` entry contains only the non-authoritative user display hint and is not authorization evidence. Keep refresh-token replay/rotation tests and CSRF tests as mandatory release gates.

## API mode behavior

- The frontend never substitutes seeded dashboard fixtures.
- Trading dashboard read endpoints are exposed by the Rust API under `/api/v1/trading/*` and use bearer JWT authentication plus explicit `dashboard_account_access` ownership grants. No mock data is used in the frontend.
- Journal and Market Data API routes use real API clients and display loading/error/empty states.
- Market Data classification/freshness must be checked; a response is not automatically production/live feed data.

## Trading dashboard API (read-only)

The following authenticated GET endpoints are currently implemented by the Journal API service and proxied by the combined gateway:

- `GET /api/v1/trading/accounts` and `GET /accounts/:id`
- `POST /api/v1/trading/accounts` creates an authenticated owner-scoped PAPER account only
- `/api/v1/trading/portfolio`
- `/api/v1/trading/strategies`
- `/api/v1/trading/positions`
- `/api/v1/trading/orders?limit=...`
- `/api/v1/trading/risk` and `/risk/decisions`
- `/api/v1/trading/events?limit=...`
- `/api/v1/trading/reconciliation`
- `/api/v1/trading/system/components` and `/system/pipeline`

The dashboard is intentionally read-only; these endpoints do not submit, cancel, or modify orders.

**Account access:** the core execution accounts predate EdgeNest identity grants. Migration `20261107000002_dashboard_account_access.sql` creates `dashboard_account_access`. Accounts are not automatically exposed to a JWT subject. A trusted provisioning/administrative workflow must explicitly grant an account to the authenticated owner; users must never be able to self-claim an arbitrary trading account. Without grants, The API returns truthful empty account/position/order data and disconnected/degraded status rather than fabricated fixtures.

Risk limits are read from linked risk profile policy JSON. Missing risk limits are returned as `null` and rendered as “Not configured”; open risk and exposure values are unknown when the current schema cannot authoritatively calculate them. System health distinguishes the API/database checks from unavailable risk/OMS/venue runtime telemetry.

## Local build and smoke-test sequence

From `frontend/`:

```bash
cp .env.example .env.local
# Fill in API origin(s) locally; do not commit .env.local.
npm ci
npm run typecheck
npm run build
npm run cloudflare:build
npm run cloudflare:preview
```

Test the deployed frontend from browser devtools/network:
- Login and register (use a non-production test user).
- Check `/api/v1/auth/me` after login.
- Load Journal accounts and trade list; verify empty state if no records.
- Load Market Data provider/instrument list and request a candle batch.
- Verify BFF CSRF enforcement and refresh-token rotation; verify CORS preflight only for approved external browser clients.
- Confirm dashboard figures originate from authenticated API responses.

Backend API hosts are **deployment-specific** and cannot be inferred from the Cloudflare frontend Worker URL. Deploy/route the Rust services first, then set their actual public HTTPS origins. Do not put private service hosts or database ports directly in the browser if a public gateway is available.
