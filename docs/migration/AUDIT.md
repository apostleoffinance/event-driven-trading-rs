# Migration Audit — Phase 0

**Repository:** `event-driven-trading-rs` (local path `event-trading/`)  
**Baseline commit:** `a84b42e` (`main`)  
**Baseline tag:** `v0.1-paper-engine`  
**Migration branch:** `refactor/trading-infrastructure`  
**Audit date:** 2026-09-06

---

## Baseline toolchain results

| Command | Result |
|---------|--------|
| `cargo check` | **Pass** |
| `cargo test` | **Pass** — 9 tests |
| `cargo clippy --all-targets` | **Pass** with warnings (redundant closures, module inception) |

---

## Current architecture

Single crate paper-trading demo:

```text
REST ticker (Binance|Bybit)
  → normalize / dedupe
  → Strategy.signal()
  → ExecutionEngine.execute()   # risk + size + order + fill + position
  → println + exit
```

Sync `Mutex` event bus is observational, not the control plane.

### Principle violations (target architecture)

| Principle | Current state |
|-----------|---------------|
| Strategy ≠ orders | `Signal` flows directly into `ExecutionEngine::execute` |
| Strategy ≠ account risk | Strategy holds `risk_percentage`; `RiskProfile` under strategy config |
| Account ≠ Venue | Only `ExchangeType` for market data; no account / execution venue split |

---

## File inventory & migration matrix

| CURRENT FILE | ACTION | TARGET | REASON |
|--------------|--------|--------|--------|
| `Cargo.toml` | REFACTOR | workspace root | Modular monolith |
| `src/**` (pre-move) | KEEP | `crates/event-trading/` | Preserve paper engine |
| `src/engine/bus.rs` | REPLACE (Phase 2) | `crates/events` | Async typed channels |
| `src/engine/event_loop.rs` | DELETE later | — | Empty stub |
| `src/market_data/**` | KEEP/MOVE (Phase later) | `crates/market-data` | Highest-value reuse |
| `src/strategy/mean_reversion.rs` | REFACTOR (Phase 3) | `strategies/mean-reversion` | Fix window; emit `TradeIntent` |
| `src/execution/**` | REFACTOR (Phase 6–7) | execution-engine + SimulatedVenue | Split OMS from venue |
| `src/risk/**` | REFACTOR (Phase 4) | `crates/risk-engine` | Deterministic rule chain |
| `src/portfolio/**` | REFACTOR (Phase later) | `crates/portfolio-engine` | Attribution |
| `src/instrument/**` | REPLACE (Phase 1) | `crates/domain` | Stub → real types |
| `src/utils/clock.rs` | REPLACE later | domain/runtime clock | Stub |
| `src/error.rs` | REPLACE gradually | per-crate errors | Reduce coupling |
| *(new)* `crates/domain` | CREATE (Phase 1) | — | Shared primitives |

---

## Dependencies (baseline)

`tokio`, `reqwest`, `serde`, `serde_json`, `rust_decimal`, `thiserror`, `anyhow`, `async-trait`, `dotenv`

**Added for domain (Phase 1):** `chrono` (workspace).

---

## Strengths

- Live Binance/Bybit public market data with Decimal normalization
- Resilient primary/secondary fetcher + price monitor
- Paper OMS / fill simulation seed for `SimulatedVenue`
- Portfolio PnL primitives
- Risk limit / kill-switch concepts already present

## Weaknesses / known bugs

1. Mean reversion never mutates price window → permanent `Hold`
2. Strategy owns account risk parameters
3. Hardcoded `2%` sizing inside execution engine
4. No `TradeIntent`, `Account`, execution `Venue`, `ClientOrderId`, idempotency
5. One-shot runtime; empty event loop stub
6. Global `TradingError`; sync event bus
7. No persistence, reconciliation, or audit trail
8. MovingAverage / Alpaca referenced but unimplemented

## Migration risks

- Workspace move must keep legacy binaries green
- Splitting risk/execution without a shim can break paper path
- Introducing Postgres (Phase 8) too early would stall domain work
- Over-renaming working market-data code

---

## Phase 0 decision

Proceed to Phase 1: Cargo workspace + `crates/domain` only.  
Do **not** implement Phases 2–11 in the same change.
