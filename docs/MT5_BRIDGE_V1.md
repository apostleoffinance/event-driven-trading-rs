# MT5 Bridge v1 Protocol — QOS-014

Status: **protocol + durable idempotency/checkpoints + transport-independent session gate implemented; authenticated transport integration pending**. This is not an MT5 terminal connector and is not evidence of live execution support.

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

- `account_id` is the trading account identity; `terminal_id` and `server` identify the terminal connection only. `Mt5SessionGate` binds one post-authentication session to an allow-listed account/terminal/server tuple and closes on identity changes. The future transport must authenticate its peer before constructing this gate; the gate itself does not authenticate a socket.
- `instrument_id` is the broker's exact symbol (including suffixes); do not silently normalize `EURUSD`, `EURUSD.a`, and `EURUSDm` as identical.
- All quantity, price, stop, fill and average-fill values are base-10 decimal **strings**. JSON numeric values and exponent notation are prohibited. The adapter must map broker lot/contract/tick units explicitly; the protocol does not infer pip value or account-currency conversion.
- Quantity is positive for submit and non-negative for filled quantity. Price/stop values, when supplied, must be positive. Market orders omit price; limit/stop/stop-limit orders require the applicable trigger/limit prices. Instrument tick/volume/minimum constraints must be validated against fresh broker metadata before submission.

## Idempotency and ambiguous outcomes

Rust persistence now provides an account-scoped `mt5_bridge_order_keys` ledger. `claim_submit_order` commits the client-order ID and canonical JSON payload before transport send. Only a `New` claim authorizes the first send; any existing row, including `prepared`, is ambiguous after restart and must reconcile rather than resend. JSONB structural comparison rejects same-ID/different-payload reuse. `persist_submit_outcome` stores accepted/rejected/unknown outcomes; unknown is not treated as failure, and accepted/rejected states cannot be rewritten by a late conflicting response. Duplicate identical outcomes are idempotent.\n\n`mt5_bridge_streams` stores account/terminal sequence checkpoints. New streams, duplicates, out-of-order messages and gaps require reconciliation; readiness returns to ready only through the explicit reconciliation checkpoint API. The caller must reconcile orders, positions and account state before invoking it. If a terminal restarts or loses its response after sending, it must reconcile by stable ID, broker ticket, order history, open orders and deals. It must not blindly retry an ambiguous submit.

Sequence numbers, when available, must be monotonic within the documented terminal stream. A gap, reconnect, heartbeat timeout, account mismatch or stale terminal time marks the bridge degraded and blocks new exposure until order/position/account reconciliation succeeds. Reconnect does not by itself restore readiness.

## Session readiness and dispatch gate

`crates/intent-bridge/src/mt5_session.rs` provides a transport-independent state gate. The host config supplies the account allow-list; the terminal cannot expand it. Once the first valid heartbeat binds an account, terminal ID and server, any identity change closes that session. Connected state alone is insufficient: the terminal must declare order lookup, stable client-order idempotency, sequence numbers, and at least one order type. Submit/cancel checks require the same bound account and supported capabilities.

A new session or a degraded session remains blocked until the host has reconciled external orders, positions and account state, persisted the stream checkpoint, and explicitly called the session reconciliation transition. Heartbeats and reconnect alone do not clear this latch. These methods are guards only: the caller must still pass through risk authorization, OMS lifecycle validation, and durable order claim/outcome persistence.

## Durable dispatch coordinator

`crates/intent-bridge/src/mt5_dispatch.rs` adds a transport-independent coordinator for the pre-send and outcome-persistence boundary. `prepare_submit` first checks the session gate and validates the frame, then claims the account-scoped client-order ID in PostgreSQL. Only a newly committed claim yields a private `Mt5DispatchPermit`; an existing `prepared`, `unknown`, `accepted`, or `rejected` claim yields `ReconcileRequired` and never a new send permit.

The permit exposes the exact validated request frame for the future transport's single initial send. Outcome persistence consumes the permit and verifies the response account, correlation ID, client-order ID, schema/timestamp and unknown-reason requirements before writing the durable state. A transport failure after the permit is issued must be recorded as `unknown`, consumes the permit, and degrades the session. A persistence failure while recording an outcome also degrades the session because venue execution may already have happened. Callers must reconcile before any subsequent submit; this API does not retry, does not perform network I/O, and does not replace strategy risk authorization or OMS lifecycle checks.

The coordinator is a safety boundary, not a transport. It does not implement the authenticated handshake, sockets, Expert Advisor, broker symbol mapping, or actual venue send/reconciliation queries. Transport integration must use this coordinator rather than calling raw persistence helpers ad hoc.

## Read-only market-data snapshots (QOS-015B)

The Python MT5 worker may emit `market_data_snapshot` frames between heartbeats and reconciliation snapshots. Configure `MT5_SYMBOLS` as a comma-separated list of 1–64 exact broker symbol names; symbols are not normalized or guessed. Each frame pairs one instrument specification with one quote for every configured symbol.

Instrument metadata includes base/profit currency, digits, point, tick size, tick value in account currency, contract size, volume minimum/maximum/step, and trade mode. Each quote includes bid, ask, and the broker tick timestamp. The adapter fails closed if a configured symbol cannot be selected, metadata or tick data is unavailable, the account identity changes, or a tick is older than 60 seconds or more than five seconds in the future.

Rust validates that quote symbols exactly match metadata symbols, bid is positive, ask is not below bid, sizing constraints are positive/consistent, and quote timestamps are fresh (maximum 60 seconds). Accepted frames are persisted to `mt5_bridge_market_data_snapshots`, separately from the reconciliation snapshot table. A market-data write does not by itself restore stream readiness. Consumers must re-check quote freshness at the point of sizing and use the broker's exact symbol and account-currency tick-value semantics.

This is still a read-only feed. It does not authorize or submit orders, and it does not make QOS-013 sizing production-ready until the sizing engine consumes these records with account-currency conversion, cost assumptions, exposure/margin reserves, and symbol-specific volume rounding.

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
- [x] Transport-independent session gate for account allow-list, immutable session identity, capability checks and explicit reconciliation latch.
- [x] Transport-independent durable dispatch coordinator for session authorization, pre-send claim, no-resend on existing claims, response binding, and ambiguous-outcome persistence.
- [ ] Authenticated transport implementation, real send/reconnect orchestration, and cross-language conformance against an MT5-side implementation.
- [x] DB-backed persistence tests cover prepared-claim replay after store restart, same-ID/different-payload rejection, unknown-to-accepted recovery, conflicting terminal outcome rejection, sequence gap degradation, explicit recovery, and sequence overflow.
- [ ] End-to-end transport integration around actual send/response paths and concurrent duplicate-claim stress tests.
- [x] Full workspace CI passed on the dispatch implementation head; rerun after final verification-document updates is pending.


## Read-only reconciliation snapshot extension (QOS-015A)

The additive `reconciliation_snapshot` payload carries a complete point-in-time account state, open orders and positions from the Python terminal worker. It is identity-bound to the enclosing frame's account ID and to the terminal/server pair learned from the authenticated heartbeat. The Rust host validates freshness, decimal lexemes, duplicate IDs, identity and sequence before persisting the snapshot.

This is a coordinated v1 extension: older clients that strictly reject unknown payload variants must not be used with this worker. The worker advertises order submission and cancellation capabilities as false. A reconciled read-only session is not permission to trade; Rust submit authorization independently requires stable client-order idempotency and the relevant order capability. No order-send/cancel operation is implemented in QOS-015A.
