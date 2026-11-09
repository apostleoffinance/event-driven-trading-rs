# EdgeNest Frontend API Configuration

The Next.js frontend consumes Rust APIs through HTTPS. These are **public origin values**, not credentials. Next.js `NEXT_PUBLIC_*` values are embedded into the browser bundle at build time.

## Origins

## Data source

The frontend is API-only. It does not load seeded or fabricated trading records. If an API is unavailable, the interface displays a loading, error, or empty state rather than substituting sample data.


Configure one gateway origin or the three service origins:

| Variable | Service | Required routes |
|---|---|---|
| `NEXT_PUBLIC_API_URL` | Shared gateway (optional fallback) | routes below |
| `NEXT_PUBLIC_IDENTITY_API_URL` | Identity API | `/api/v1/auth/*` |
| `NEXT_PUBLIC_JOURNAL_API_URL` | Journal API | `/api/v1/journal/*` |
| `NEXT_PUBLIC_MARKET_DATA_API_URL` | Market Data API | `/api/v1/market-data/*` |

Service-specific origin wins over `NEXT_PUBLIC_API_URL`. Origins should be HTTPS in production and should not include a path prefix unless your gateway is configured to accept the appended service paths.

Set:
```env
NEXT_PUBLIC_ENVIRONMENT=PAPER
NEXT_PUBLIC_IDENTITY_API_URL=https://<identity-api-host>
NEXT_PUBLIC_JOURNAL_API_URL=https://<journal-api-host>
NEXT_PUBLIC_MARKET_DATA_API_URL=https://<market-data-api-host>
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

Market Data (read-only):
- `GET /api/v1/market-data/providers`
- `GET /api/v1/market-data/instruments`
- `GET /api/v1/market-data/candles?instrument=EURUSD&timeframe=M1&limit=100`
- `GET /api/v1/market-data/mt5/latest`

The frontend exposes sign-in/register at `/login`, Journal at `/journal`, and read-only Market Data at `/market-data`.

## CORS requirements

For the Identity and Journal Rust services, set their origin allowlists to the exact frontend origin (for the temporary deployment, the exact workers.dev hostname):
- `IDENTITY_CORS_ORIGINS`
- `JOURNAL_CORS_ORIGINS`

Do not use `*` with credentialed browser sessions. This client currently sends bearer tokens in the Authorization header, so the server must allow Authorization and Content-Type headers. Market Data requests are read-only and currently send Accept.

## Auth session notes

Current client implementation stores access and refresh tokens in tab-scoped `sessionStorage`, rotates tokens via the Identity API when access-token requests return 401, and clears session state on logout or failed refresh. This is a functional first integration, **not the preferred final production security posture**: sessionStorage is accessible to JavaScript and vulnerable to token theft under XSS. Before production use, move token handling to an HttpOnly/Secure/SameSite cookie-backed BFF/session boundary, add CSRF controls as appropriate, and test refresh-token rotation/replay behavior.

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
- Verify CORS preflight and expired-token refresh.
- Confirm dashboard figures originate from authenticated API responses.

Backend API hosts are **deployment-specific** and cannot be inferred from the Cloudflare frontend Worker URL. Deploy/route the Rust services first, then set their actual public HTTPS origins. Do not put private service hosts or database ports directly in the browser if a public gateway is available.
