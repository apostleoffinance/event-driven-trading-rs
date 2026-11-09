# Quant OS Frontend

Operational control plane for the Rust quantitative trading infrastructure.

**Mode:** PAPER · API-backed dashboard and Journal  
The dashboard is read-only for execution. Journal records are owner-scoped; Paper account creation is supported.

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

## Cloudflare Workers

This frontend deploys to Cloudflare Workers through OpenNext (not Pages or Vercel).

- Workers Builds root directory: `frontend`
- Install: `npm ci`
- Build: `npm run cloudflare:build`
- Deploy command: `npx wrangler deploy`
- Worker config: `wrangler.jsonc`
- Deployment runbook: `DEPLOY_CLOUDFLARE.md`

For local Workers-runtime validation:

```bash
npm run cloudflare:build
npx opennextjs-cloudflare preview
```

The frontend uses authenticated API responses only. When a service is unavailable, it displays an explicit error or empty state; it never substitutes fabricated trading records.

## Related docs

- `docs/frontend-api-contract.md`
- `docs/frontend/IMPLEMENTATION_NOTE.md`
- `docs/architecture/STRATEGY_BRIDGE.md`
