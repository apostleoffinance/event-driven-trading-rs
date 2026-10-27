# MT5 Bridge v1 Protocol — QOS-014

Status: **protocol + durable idempotency/reconnect checkpoint slice implemented; transport integration pending**. This is a wire contract and Rust validator, not an MT5 terminal connector and not evidence of live execution support.

## Transport and framing

- Transport v1 is newline-delimited JSON (NDJSON): exactly one UTF-8 JSON frame per line. Maximum frame size for an implementation is 64 KiB; oversized lines must be rejected before deserialization.
- Every frame carries `schema_version: 1`, `message_id`, `correlation_id`, `account_id`, `sent_at` (RFC 3339 UTC timestamp), and a typed `payload`.
- The schema is [`mt5_bridge_v1.schema.json`](contracts/mt5_bridge_v1.schema.json). Rust reference types and validation live in `crates/intent-bridge/src/mt5_v1.rs`. `NdjsonFrameDecoder` accepts arbitrary transport chunking, emits only newline-terminated validated frames, accepts CRLF, rejects blank/non-UTF-8/oversized lines, and fails closed after a malformed frame. EOF with a partial frame is an error. A canonical submit example is [`fixtures/mt5_v1_submit_order.json`](contracts/fixtures/mt5_v1_submit_order.json).
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

Rust persistence now provides an account-scoped `mt5_bridge_order_keys` ledger. `claim_submit_order` commits the client-order ID and canonical JSON payload before transport send. Only a `New` claim authorizes the first send; any existing row, including `prepared`, is ambiguous after restart and must reconcile rather than resend. JSONB structural comparison rejects same-ID/different-payload reuse. `persist_submit_outcome` stores accepted/rejected/unknown outcomes; unknown is not treated as failure, and accepted/rejected states cannot be rewritten by a late conflicting response. Duplicate identical outcomes are idempotent.\n\n`mt5_bridge_streams` stores account/terminal sequence checkpoints. New streams, duplicates, out-of-order messages and gaps require reconciliation; readiness returns to ready only through the explicit reconciliation checkpoint API. The caller must reconcile orders, positions and account state before invoking it. If a terminal restarts or loses its response after sending, it must reconcile by stable ID, broker ticket, order history, open orders and deals. It must not blindly retry an ambiguous submit.

Sequence numbers, when available, must be monotonic within the documented terminal stream. A gap, reconnect, heartbeat timeout, account mismatch or stale terminal time marks the bridge degraded and blocks new exposure until order/position/account reconciliation succeeds. Reconnect does not by itself restore readiness.

## Security and deployment boundary

- Use an authenticated local IPC channel or mutually authenticated transport; do not expose an unauthenticated order socket to a LAN or public internet.
- Configure an explicit account allow-list and paper-only default. Secrets stay in the Rust service/terminal secret store, never in strategy payloads, logs or frontend code.
- Validate message size, JSON shape, timestamps, IDs, enums and decimal lexemes before acting. Rate-limit inbound messages and redact broker diagnostics.
- Strategies produce intents only. Rust risk authorization and OMS lifecycle checks must precede any terminal submit. The MT5 side must not independently decide to execute an unauthorised intent.
- This PR does **not** implement an Expert Advisor, socket transport, authenticated handshake, broker order mapping, end-to-end account reconciliation, or live venue adapter. Persistence APIs and protocol helpers exist, but the actual transport must invoke them before and after real sends. Those belong to follow-on work and require simulated contract tests before any live-mode review.

## Acceptance checks for QOS-014

- [x] Versioned typed Rust message models and JSON schema.
- [x] Explicit account/message/correlation IDs and timestamp.
- [x] Decimal-string validation and side/type/TIF enums.
- [x] Heartbeat with connection and capability state.
- [x] Accepted/rejected/unknown submit outcomes.
- [x] Cancellation request distinct from terminal state.
- [x] Snapshot/reconciliation messages and focused unit tests.
- [x] Bounded incremental NDJSON framing with malformed, blank, non-UTF-8, oversized, split-chunk, multi-frame and incomplete-EOF tests.
- [ ] Cross-language golden fixtures shared with a terminal-side implementation.
- [x] Durable account-scoped idempotency ledger, canonical-payload conflict detection, ambiguous prepared/unknown recovery boundary, and persistent sequence checkpoint/readiness API implemented.
- [x] DB-backed persistence tests cover prepared-claim replay after store restart, same-ID/different-payload rejection, unknown-to-accepted recovery, conflicting terminal outcome rejection, sequence gap degradation, explicit recovery, and sequence overflow.
- [ ] End-to-end transport integration around actual send/response paths and concurrent duplicate-claim stress tests.
- [ ] Full workspace CI on the latest branch head.
