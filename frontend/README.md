# Quant OS Frontend

Operational control plane for the Rust quantitative trading infrastructure.

**Mode:** PAPER · mock repositories · mock event stream  
**Does not** execute trades, evaluate risk, or hold secrets.

## Setup

```bash
cd frontend
cp .env.example .env.local
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

## Scripts

| Command | Purpose |
|---------|---------|
| `npm run dev` | Local development |
| `npm run lint` | ESLint |
| `npm run typecheck` | `tsc --noEmit` |
| `npm run build` | Production build |

## Architecture

```text
UI → repository interfaces → MockDataProvider (today)
                          → ApiDataProvider (future Rust REST)
UI → useEventStream → MockEventStream (today)
                   → WebSocket/SSE (future)
UI → TanStack Query (cache/loading) + TanStack Table (sortable grids)
```

Domain types under `src/types` align with Rust crates (`Buy`/`Sell`, order state
machine including `Unknown`, `TradeIntentCreated`, Account ≠ Venue, correlation IDs).

Money values are **strings** (`Money`), never JS floats for balances/P&L.

Light/dark theme toggle is available in the top bar.

## Routes

`/` Command Center · `/accounts` · `/portfolio` · `/strategies` · `/positions` · `/orders` · `/risk` · `/events` · `/reconciliation` · `/system`

## Vercel

- Root Directory: `frontend`
- Build: `npm run build`
- Install: `npm install`

Mock mode works without the Rust backend.

## Related docs

- `docs/frontend-api-contract.md`
- `docs/frontend/IMPLEMENTATION_NOTE.md`
- `docs/architecture/STRATEGY_BRIDGE.md`

## EdgeNest Identity + Journal integration

Set these public variables at build time:
- `NEXT_PUBLIC_IDENTITY_API_URL` — Identity API origin
- `NEXT_PUBLIC_JOURNAL_API_URL` — Journal API origin
- `NEXT_PUBLIC_API_URL` — compatibility fallback if services share an API origin

The browser uses:
- Access JWT in React memory only (never localStorage/sessionStorage).
- Refresh token in Identity API's Secure, HttpOnly, SameSite=None `__Host-edgenest_refresh` cookie.
- Credentialed fetches to Identity API; configure exact `IDENTITY_CORS_ORIGINS` values on the server.
- Bearer access JWT for Journal API.

Public routes: `/login`, `/register`, `/verify-email`, `/forgot-password`, `/reset-password`. Application routes require an authenticated EdgeNest user.

### Identity flows
- Registration triggers email verification.
- Login supports TOTP MFA challenges.
- Password reset uses a single-use emailed link.
- Security settings at `/security` enroll/disable TOTP and show recovery codes only once.

### Production checklist
- Use HTTPS for app and APIs.
- Refresh cookie must be Secure, HttpOnly, Path=/, SameSite=None, with no Domain attribute (required for `__Host-` prefix).
- Exact CORS allowlist + credentials on Identity API and Journal API.
- Deploy Cloudflare rate-limit rules in `infra/cloudflare` and keep API origin ingress Cloudflare-only.
- Configure Resend SPF/DKIM/DMARC and verified sender domain.
- Configure Identity and Journal API with the same issuer, audience, active KID and public keyring.
- Keep JWT signing private key and TOTP encryption secret server-side in a secret manager.
- Next.js is pinned to patched 15.5.29; frontend security CI runs `npm audit --omit=dev --audit-level=high`.
