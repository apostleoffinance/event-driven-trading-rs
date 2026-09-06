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
```

Domain types under `src/types` align with Rust crates (`Buy`/`Sell`, order state machine, `TradeIntentCreated`, Account ≠ Venue).

Money values are **strings** (`Money`), never JS floats for balances/P&L.

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
