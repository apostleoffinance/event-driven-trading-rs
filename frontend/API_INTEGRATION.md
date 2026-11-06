# EdgeNest Frontend API Configuration

The Next.js frontend consumes Rust APIs through HTTPS. These are **public origin values**, not credentials. Next.js `NEXT_PUBLIC_*` values are embedded into the browser bundle at build time.

## Origins

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
NEXT_PUBLIC_DATA_MODE=api
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

- Mock mode (`NEXT_PUBLIC_DATA_MODE=mock`) remains the explicit demo mode.
- API mode never silently substitutes seeded dashboard fixtures.
- Trading dashboard accounts/portfolio/strategies/positions/orders/risk/events/reconciliation/system endpoints are not yet implemented as Rust REST routes. Their repository methods fail with an explicit not-implemented error until the backend contract and handlers are implemented.
- Journal and Market Data API routes use real API clients and display loading/error/empty states.
- Market Data classification/freshness must be checked; a response is not automatically production/live feed data.

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
- Confirm no mock dashboard figures are shown in API mode.

Backend API hosts are **deployment-specific** and cannot be inferred from the Cloudflare frontend Worker URL. Deploy/route the Rust services first, then set their actual public HTTPS origins. Do not put private service hosts or database ports directly in the browser if a public gateway is available.
