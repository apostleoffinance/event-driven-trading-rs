# MT5 Bridge v2 — explicit market-data freshness

Status: **implemented as a versioned contract and code path; affected Rust and Python automated suites pass locally, while real-terminal/managed-host conformance remains open**. Order execution remains disabled.

## Compatibility and transport

- v2 uses the same bounded NDJSON and TLS 1.3 mTLS transport as v1, with `schema_version: 2`.
- The v1 schema remains unchanged. The v2 schema reuses v1 definitions for payloads whose shape did not change and defines the expanded market-data snapshot itself.
- The Python worker and Rust daemon must be upgraded together. Unknown protocol versions and unknown fields remain rejected; v1 market-data frames are not silently interpreted as v2.
- See [`mt5_bridge_v2.schema.json`](contracts/mt5_bridge_v2.schema.json), the historical [`mt5_bridge_v1.schema.json`](contracts/mt5_bridge_v1.schema.json), and Rust validation in `crates/intent-bridge/src/mt5_v1.rs`.

## Market-data status

Each `market_data_snapshot` has an aggregate `status`, and each quote has its own `status`:

| Status | Wire requirements | Meaning |
|---|---|---|
| `fresh` | Positive decimal-string bid/ask and actual `tick_time`; `reason_code: null` | Actual broker quote is no more than 60 seconds old at bridge validation time. |
| `stale` | Actual bid/ask and original `tick_time`; `reason_code: null` | Quote is older than 60 seconds at bridge validation time. Its prices and timestamp remain visible, but it is never fresh. |
| `unavailable` | `bid`, `ask`, and `tick_time` are `null`; `reason_code: "no_tick"` | The provider has no quote. No zero/default price is synthesized. |
| `invalid` | Quote values and timestamp are `null`; a validation `reason_code` is required | A future timestamp, invalid bid/ask, or unusable server-time reference failed quote validation. |

Aggregate status is conservative: `invalid` takes precedence over `unavailable`, then `stale`; it is `fresh` only when every configured symbol is fresh. A malformed provider response remains a protocol error, socket EOF/I/O remains a transport failure, and TLS authentication errors remain fail-closed. These failures are not converted into quote statuses.

## Connection, reconciliation, and exposure safety

- Stale, unavailable, and invalid market data are valid status outcomes, not exceptions that end the Python worker's TLS session. The worker continues the heartbeat → market-data → authoritative account/orders/positions reconciliation sequence.
- Rust persists each snapshot. On `unavailable`/`invalid`, it preserves prior actual priced quotes separately as `last_known_quotes`, explicitly marked stale and retaining their original timestamp. Current quote entries stay null. A later fresh snapshot supersedes this preserved cache.
- Any non-fresh market-data snapshot degrades durable stream readiness and the in-memory session gate, but does not poison or close the authenticated TLS connection. Reconciliation snapshots continue to persist while readiness stays degraded. A later fresh market-data snapshot followed by successful full account/order/position reconciliation restores readiness.
- Risk sizing requires both aggregate and selected quote status to be `fresh`, and continues to enforce quote timestamp age. Stale, unavailable, invalid, missing, and malformed market data cannot authorize new exposure.
- A real socket disconnect, EOF, invalid protocol frame, identity mismatch, persistence failure, or TLS authentication failure continues through the existing fail-closed recovery path.
- No quote age threshold has been relaxed, no simulated price is introduced, and no order-execution capability has been enabled.

The automated tests cover freshness-state encoding/validation, stale quote continuity, continued reconciliation frame generation, unavailable quote representation, last-known quote preservation, protocol-versus-transport errors, and fail-closed sizing. Full local verification and the isolated MetaEditor compile evidence are recorded in `REVIEWS.md` (REV-075). Real terminal, database deployment, Windows host, reconnect recovery and sustained fresh-quote evidence must still be collected before Phase 1 can be marked complete.
