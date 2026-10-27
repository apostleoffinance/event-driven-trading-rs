# MT5 Bridge v1 Protocol — QOS-014

Status: **protocol implementation in progress**. This is a wire contract and Rust validator, not an MT5 terminal connector and not evidence of live execution support.

## Transport and framing

- Transport v1 is newline-delimited JSON (NDJSON): exactly one UTF-8 JSON frame per line. Maximum frame size for an implementation is 64 KiB; oversized lines must be rejected before deserialization.
- Every frame carries `schema_version: 1`, `message_id`, `correlation_id`, `account_id`, `sent_at` (RFC 3339 UTC timestamp), and a typed `payload`.
- The schema is [`mt5_bridge_v1.schema.json`](contracts/mt5_bridge_v1.schema.json). Rust reference types and validation live in `crates/intent-bridge/src/mt5_v1.rs`.
- Unknown schema versions and unknown fields are rejected. IDs are opaque non-empty strings; never encode credentials or secrets in them.

## Message types

| Payload | Purpose | Safety rule |
|---|---|---|
| `heartbeat` | Terminal/server identity, terminal time, sequence and declared capabilities | Connected is not equivalent to trading-ready. Missing capability is unsupported. |
| `submit_order` | Risk-authorized order request with stable `client_order_id` | Retries reuse the same ID and identical canonical payload. Same ID/different payload is a conflict. |
| `submit_outcome` | `accepted`, `rejected`, or `unknown` | Lost response after a possible send is `unknown`; query/reconcile before retry. |
| `cancel_order` | Cancellation request | Request acknowledgement is not terminal cancellation. |
| `cancel_outcome` | Cancellation request result and optional terminal observation | Only authoritative terminal evidence can close the lifecycle. |
| `order_snapshot` | Order state, fill quantity/price, source timestamp and optional sequence | Preserve partial fills and detect stale/out-of-order snapshots. |
| `reconcile_request` | Ask the terminal side to query a client order ID | Not-found is not proof an ambiguous submission never reached the venue. |

## Identity, precision and units

- `account_id` is the trading account identity; `terminal_id` and `server` identify the terminal connection only. The receiver must bind the authenticated connection to an allow-listed account and reject mismatches.
- `instrument_id` is the broker's exact symbol (including suffixes); do not silently normalize `EURUSD`, `EURUSD.a`, and `EURUSDm` as identical.
- All quantity, price, stop, fill and average-fill values are base-10 decimal **strings**. JSON numeric values and exponent notation are prohibited. The adapter must map broker lot/contract/tick units explicitly; the protocol does not infer pip value or account-currency conversion.
- Quantity is positive for submit and non-negative for filled quantity. Price/stop values, when supplied, must be positive. Market orders omit price; limit/stop/stop-limit orders require the applicable trigger/limit prices. Instrument tick/volume/minimum constraints must be validated against fresh broker metadata before submission.

## Idempotency and ambiguous outcomes

The terminal-side implementation must persist the mapping from `client_order_id` to canonical payload and broker ticket/order identifier. Replaying the same ID and same payload returns the known outcome; replaying the same ID with a different payload returns a conflict without submitting. If a terminal restarts or loses its response after sending, it must report `unknown` and reconcile by stable ID, broker ticket, order history, open orders and deals. It must not blindly retry an ambiguous submit.

Sequence numbers, when available, must be monotonic within the documented terminal stream. A gap, reconnect, heartbeat timeout, account mismatch or stale terminal time marks the bridge degraded and blocks new exposure until order/position/account reconciliation succeeds. Reconnect does not by itself restore readiness.

## Security and deployment boundary

- Use an authenticated local IPC channel or mutually authenticated transport; do not expose an unauthenticated order socket to a LAN or public internet.
- Configure an explicit account allow-list and paper-only default. Secrets stay in the Rust service/terminal secret store, never in strategy payloads, logs or frontend code.
- Validate message size, JSON shape, timestamps, IDs, enums and decimal lexemes before acting. Rate-limit inbound messages and redact broker diagnostics.
- Strategies produce intents only. Rust risk authorization and OMS lifecycle checks must precede any terminal submit. The MT5 side must not independently decide to execute an unauthorised intent.
- This PR does **not** implement an Expert Advisor, socket transport, authenticated handshake, persistent idempotency store, broker order mapping, account reconciliation, or live venue adapter. Those belong to follow-on work and require simulated contract tests before any live-mode review.

## Acceptance checks for QOS-014

- [x] Versioned typed Rust message models and JSON schema.
- [x] Explicit account/message/correlation IDs and timestamp.
- [x] Decimal-string validation and side/type/TIF enums.
- [x] Heartbeat with connection and capability state.
- [x] Accepted/rejected/unknown submit outcomes.
- [x] Cancellation request distinct from terminal state.
- [x] Snapshot/reconciliation messages and focused unit tests.
- [ ] NDJSON size/framing transport with malformed-frame tests.
- [ ] Cross-language golden fixtures shared with a terminal-side implementation.
- [ ] Persisted idempotency/reconciliation integration and simulated reconnect/sequence-gap tests.
- [ ] Full workspace CI on the latest branch head.
