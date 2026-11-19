# MT5-MVP-01A Read-Only Data Contract

**Contract baseline:** EdgeNest MT5 Bridge schema version 2 (`schema_version: 2`).  
**Status:** V2's existing read-only subset is documented below. This document does not change the wire protocol. Fields marked **not carried** require an explicit schema/version decision and implementation before they may be treated as contract data.

## 1. Contract boundary

The primary 01A provider is the official Windows `MetaTrader5` Python package, running on the same managed Windows host as its terminal. The native MQL5 EA plus loopback Python socket provider remains a separate macOS development/conformance adapter. Do not run both providers for one terminal/account stream concurrently. The QOS-015 architecture also makes clear that macOS contract tests do not prove real-terminal conformance.

Existing v2 frames are UTF-8 JSON Lines (one newline-terminated frame), limited to 64 KiB. Rust rejects unknown fields, unsupported schema versions, invalid timestamps/decimals, duplicate symbols/tickets, invalid quote state, and identity/capability violations. Money, quantities and prices are canonical base-10 decimal strings, never JSON floating-point numbers. Canonical timestamps are UTC RFC3339. Financial values must retain their original decimal text/precision; do not round through binary floating point.

## 2. Envelope and stream identity

| Field | Source / canonical type | Nullability and units | Semantics and validation | V2 status |
|---|---|---|---|---|
| `schema_version` | Worker constant / unsigned integer | Required; version number | Must equal 2; Rust currently rejects other versions. | Present |
| `message_id`, `correlation_id` | Worker UUID strings | Required | Unique message and logical polling correlation identifiers; non-empty. | Present |
| `account_id` | Configured EdgeNest `accounts.id` / opaque text | Required | Quant OS account ID. It is not inherently the broker login. Must match the single allowlisted principal account. | Present |
| `terminal_id` | Host-assigned config / opaque text | Required in payload | Must match the mTLS principal; stable per assigned terminal. | Present |
| `server` | Exact configured MT5 server string / text | Required in payload | Checked against worker's observed account at provider and Rust principal configuration; do not lowercase or infer aliases. | Present |
| `source` | Worker provider identifier / text | Required | Identifies the adapter path (e.g. `mt5_python_readonly_v1`); not an authenticated account assertion. | Present on snapshots |
| `sequence` | Persistent worker checkpoint / signed 64-bit-compatible integer | Required; dimensionless | Monotonic in the `(account_id, terminal_id)` stream. Rust persists checkpoints on this pair and rejects stale/gapped observations. The worker checkpoint file must be uniquely assigned to that pair and protected against concurrent workers; the current default path is host-global and does not itself encode account/terminal. | Present; deployment must scope file |
| `sent_at` | Worker host UTC clock / timestamp | Required | Frame creation/send time, not a broker quote timestamp. | Present |
| `captured_at` | Worker host UTC clock / timestamp | Required for market/reconciliation snapshots | Collection time; distinct from source tick time. It is not an MT5 server time. | Present |
| broker login / account environment / account type | Not emitted in v2 | No current value | Do not overload `account_id` with the login or infer DEMO from a configured server string. They require separate trusted evidence/contract fields. | Not carried |

Heartbeat `terminal_time` currently receives `utc_now()` from the worker; it is therefore a worker host clock value, despite its name, and is not reliable terminal/server time evidence. This semantic must be corrected through an explicitly versioned contract change before using it as terminal time.

## 3. Account snapshot

V2 `reconciliation_snapshot` consists of the following account state:

| Field | Provider source | Canonical type / units | Nullability | Validation / status |
|---|---|---|---|---|
| `currency` | `account_info().currency` | Currency code text | Required | Non-empty; no explicit ISO-4217 validation today. |
| `balance` | `account_info().balance` | Decimal string in account currency | Required | Exact text conversion; finite numeric. |
| `equity` | `account_info().equity` | Decimal string in account currency | Required | Exact text conversion; finite numeric. |
| `margin_used` | `account_info().margin` | Decimal string in account currency | Required | Exact text conversion; finite numeric. |
| `free_margin` | `account_info().margin_free` | Decimal string in account currency | Required | Exact text conversion; finite numeric. |
| margin level, leverage, trade permissions, account state/type | Not mapped by worker | N/A | Not in v2 | Do not infer. Proposed optionality, validation and provider availability require a contract/version decision. |
| source timestamp | Not returned in current mapped account payload | N/A | Not in v2 | `captured_at` is the host collection time only. |

The official adapter checks `account_info().login` and `.server` against the configured `MT5_LOGIN` and `MT5_SERVER` at connect and before each snapshot. It does not put the numeric login into the frame. This is useful local consistency checking but does not by itself prove entitlement/account ownership to the Rust daemon.

## 4. Instrument and quote snapshots

`market_data_snapshot` carries matched `instruments` and `quotes`, with 1–64 unique configured symbols.

| Field | Provider source | Type / units | Nullability | V2 status and rule |
|---|---|---|---|---|
| `symbol` | `symbol_info().name` / tick symbol | Exact broker symbol text | Required | Preserved exactly; tick symbol must match configured symbol. |
| `base_currency`, `quote_currency` | `currency_base`, `currency_profit` | Currency text | Required | Current worker treats these as mandatory, though broker/provider availability may vary. A nullable redesign needs versioning. |
| `digits` | `symbol_info().digits` | Integer decimal precision | Required | Maximum 16. |
| `point`, `tick_size`, `tick_value_account_currency`, `contract_size`, `volume_min`, `volume_max`, `volume_step` | Corresponding `symbol_info` fields | Decimal strings; broker-defined units; tick value in account currency | Required in current v2 | Positive finite decimals; volume range/step consistency checked. Optional-provider handling is not implemented for these fields. |
| `trade_mode` | `symbol_info().trade_mode` | Provider enum rendered as text | Required | Non-empty; no canonical cross-broker enum mapping today. |
| `description` | Not mapped | N/A | Not carried | Optional candidate; no provider contract commitment. |
| `bid`, `ask` | `symbol_info_tick().bid/.ask` | Decimal price in instrument quote convention | Required for `fresh`/`stale`; null otherwise | Positive bid; ask >= bid. |
| `last`, `volume` | Not mapped | N/A | Not carried | Optional candidate; not supplied by current wire structure. |
| `tick_time` | `tick.time`, normalized to UTC | Timestamp | Required for `fresh`/`stale`; null otherwise | `fresh` when age is at most 60 seconds and not more than 5 seconds future; `stale` when older than 60 seconds. macOS provider has additional server clock offset validation. |
| quote `status` | Worker validation | `fresh`, `stale`, `unavailable`, `invalid` | Required | `unavailable` and `invalid` contain null bid/ask/time and required reason; no fabricated prices. Aggregate status is derived from the least healthy per-symbol state. |

Operational note: Windows `MetaTraderTerminal.select_symbol()` calls `symbol_select(symbol, True)`, which may add symbols to Market Watch. That is not an order operation, but is a terminal configuration side effect and must be disclosed/accepted in 01A's operator procedure or removed in a separately authorized implementation change.

## 5. Position and open-order snapshots

These are complete current open-state snapshots, not historical deal/backfill feeds.

| Object / field | Provider source | Type / units | Nullability | V2 rule / gap |
|---|---|---|---|---|
| Position `venue_position_id` | `position.ticket` | Opaque ticket text | Required | Non-empty, unique within snapshot. |
| Position `instrument_id` | `position.symbol` | Exact symbol text | Required | Non-empty. |
| Position `side` | Position type constant | `buy`/`sell` text | Required | Current Python mapping covers these two types. |
| Position `quantity` | `position.volume` | Decimal lots | Required | Positive exact decimal string. |
| Position `average_price` | `position.price_open` | Decimal price | Required | Exact decimal string. |
| Position `unrealized_pnl` | `position.profit` | Decimal account-currency amount | Required | Exact decimal string. |
| Position current price, SL, TP, swap, open time | Not mapped | N/A | Not carried | Must remain unavailable, not inferred; additions require versioned design. |
| Order `venue_order_id` | `order.ticket` | Opaque ticket text | Required | Non-empty, unique within snapshot. |
| Order `instrument_id` | `order.symbol` | Exact symbol text | Required | Non-empty. |
| Order `side` / `order_type` | Provider order type enum | Canonical side plus enum text | Required | Current worker maps known market/limit/stop variants; unknown maps `unknown` side. |
| Order `quantity` | `order.volume_current` | Decimal lots, remaining volume | Required | Positive; v2 does not separately represent initial versus remaining volume. |
| Order `status` | `order.state` | Provider enum rendered as text | Required | Current value is not normalized across brokers. |
| Order client ID | Not supplied for external open order | Nullable text | Nullable | Current worker sends null. |
| Order trigger/entry price, SL, TP, initial volume, setup time | Not mapped | N/A | Not carried | No fabricated values; requires versioned design. |
| Source timestamp | Not supplied per position/order | N/A | Not carried | Snapshot `captured_at` indicates worker collection time only. |

## 6. Connection and reconciliation state

| State | Existing source / type | Semantics |
|---|---|---|
| Transport identity | TLS 1.3 mTLS peer certificate, exact leaf DER SHA-256 allowlist | Authenticates a worker workload. The configured principal also pins one terminal ID, server string, and EdgeNest account ID. It does not prove broker account ownership. |
| Connection | Heartbeat `connection` enum and transport watchdog | Heartbeat liveness is separate from data freshness and reconciliation. Rust heartbeat timeout defaults to 30 seconds. |
| Last successful observation | `captured_at`, persisted stream/snapshot rows | Collection timestamp; no separately persisted last-failed observation field in current v2 schema. |
| Last failed observation / error category | Worker safe stderr stage/error category; Rust logs/errors | No normalized durable last-error state is carried in v2. Do not persist raw exception/provider details or secrets. |
| Last successful sequence | Durable `mt5_bridge_streams` keyed by account/terminal | Worker increments sequence before sending; Rust persists sequence observation and requires reconciliation after gaps/reconnect. |
| Reconciliation state | Session gate: awaiting heartbeat, reconciliation required, ready, degraded, closed; durable stream readiness | Ready only after fresh identity-bound complete snapshot is persisted. Snapshot includes account state, all current open orders and positions. |
| Reconciliation version | No separate revision field | Schema version is protocol version, not business reconciliation version. No distinct reconciliation revision is present. |
| Market freshness | Per-symbol and aggregate status, quote source time | Market freshness is independent from account/order/position reconciliation. Stale/unavailable data cannot be treated as fresh merely because the socket is connected. |
| Restart/reconnect | Worker exponential retry 1–30 sec; durable local sequence file; daemon requires new authenticated connection and reconciliation | No explicit durable restart counter in v2. A local sequence file corruption fails worker startup. |

## 7. Persistence and account-binding semantics

- `mt5_bridge_market_data_snapshots` stores one latest row per `(account_id, terminal_id)` with the complete JSON snapshot, sequence, source, server, account currency, and capture time. Per-symbol freshness is retained in the JSON snapshot. Unavailable updates do not fabricate prices; repository logic maintains explicit state.
- `mt5_bridge_reconciliation_snapshots` receives a new row from the repository reconciliation path, keyed by account/terminal with reconciled sequence, source, timestamp, and JSON account/order/position state. The migration comment calls it append-only evidence, but the inspected migration defines no immutability trigger or privilege restriction; database-enforced append-only behavior is therefore **not established**. `mt5_bridge_streams` stores the durable sequence/readiness checkpoint.
- Broker/tenant registry is in migrations `20261112000002_organization_tenant_foundation.sql` and `20261113000001_broker_account_registry.sql`: organization account entitlement, broker/server, HMAC-keyed external broker account identity, evidence, verification decision, and immutable binding history. The schema avoids storing raw external login identifiers.
- The daemon's current `mt5-bridge-daemon` configuration accepts `MT5_ACCOUNT_ID`, `MT5_TERMINAL_ID`, `MT5_SERVER`, and certificate fingerprint as an environment-configured allowlist. The inspected daemon path does not query the broker registry or require a `VERIFIED` external-account binding before accepting and persisting a stream. This integration is a 01A prerequisite.
- Account environment provenance currently permits Quant account environments `PAPER`, `SHADOW`, `LIVE`; external broker server environment permits `DEMO`, `LIVE`, `UNKNOWN`. Do not silently map an external DEMO account to PAPER or SHADOW. Define the intended mapping and verified account eligibility before 01A.
- Existing MT5 tables use account and terminal text keys rather than composite foreign keys to the tenant/broker registry. A shared-name resemblance is not an enforced ownership relationship.

## 8. Immutable read-only invariants and unresolved decisions

1. No order send, modification, cancellation, close, signals, dispatch, historical deal backfill, or automated failover is in this contract. Worker heartbeat execution capabilities remain false.
2. Do not make an account operationally eligible until active organization entitlement, verified environment, broker/server, HMAC-matched observed login, approved binding revision, assigned terminal identity, and mTLS worker identity are checked together.
3. An authorized operator must independently confirm the exact DEMO login/server against broker terminal evidence, record the evidence and approval actor/reference through the broker registry, and verify the binding revision before ingestion eligibility. mTLS alone is not that evidence.
4. Resolve before 01A: DEMO mapping in Quant account environment; wire compatibility strategy for the fields absent from schema v2; whether Market Watch selection is acceptable; account-type and optional provider-field semantics; durable error/restart observability; stream sequence-file path and single-worker ownership; certificate issuance/storage/revocation/rotation.
5. No suitable approved secret-delivery mechanism is identified in the inspected repository. Current Windows provider reads `MT5_PASSWORD` from the process environment and client private-key path from configuration; README recommends ACL restriction but does not establish an approved secret manager. This is a blocker. Do not put secrets in source, committed config, command history, logs, or routine environment dumps.

