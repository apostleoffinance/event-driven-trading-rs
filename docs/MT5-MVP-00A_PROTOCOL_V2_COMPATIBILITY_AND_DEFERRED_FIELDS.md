# MT5-MVP-00A ADR-04 — Schema v2 Compatibility and Deferred Fields

**Status:** Proposed compatibility decision for 01A; no protocol change authorized.  
**Source of truth for existing capabilities:** `docs/MT5-MVP-01A_READ_ONLY_DATA_CONTRACT.md`.

## Decision

Preserve `schema_version: 2` for the initial supported read-only subset. Do not add unmodeled fields to strict v2 frames. For 01A, transmit only existing v2 fields and obtain account ownership/environment admission through a distinct authenticated control-plane admission decision. Do not put unnecessary customer ownership evidence into ordinary market data.

V2 remains sufficient only when all use cases require its current fields: account currency/balance/equity/margin/free margin; configured symbol specifications; bid/ask with freshness state; complete open orders and positions using the existing limited representation; heartbeat, identity, sequence and capture time; persistence and reconciliation. It is not sufficient for every optional attribute listed in MVP-00.

Do not create v3 merely to include optional provider fields. A future v3 is warranted only if a required, justified field cannot be carried as a separately versioned optional capability without ambiguity, and after compatibility, persistence, consumer and rollout decisions are approved. Keep old/new worker/daemon pairs explicit: artifact manifest records protocol major, worker version/hash, daemon version/hash, admission protocol version, and required schema range. Each side fails closed on an unsupported version. Roll out worker and daemon as an approved compatible pair; no negotiation may silently downgrade to v1 or unknown formats. Strict unknown-field rejection remains.

## Field inventory

Legend: **A** already supported by v2; **B** Python provider exposes it but current v2 does not represent it; **C** availability/meaning not reliably established across provider/brokers; **D** deferred from initial 01A unless a separately approved need exists. A field can carry multiple labels, e.g. provider-visible but semantics are not sufficiently normalized.

| Group / field | Class | Current evidence and 01A treatment |
|---|---|---|
| Envelope schema version, message/correlation IDs, account ID, sent time | A | `BridgeFrame`; v2 strict JSONL, 64 KiB. Pin v2 pair. |
| Worker source, terminal ID, broker server, sequence, captured time | A | Present in heartbeat/snapshots; `sent_at`/`captured_at` are worker host UTC. `terminal_time` is currently generated from host UTC despite its name. |
| Worker certificate fingerprint and TLS principal | A | Authenticated out-of-band at transport, not payload; static configured principal today. |
| Broker numeric login | B | Official provider exposes `account_info().login`; worker compares it locally to configured `MT5_LOGIN` but does not transmit it. Use control-plane admission observation, compute HMAC server-side, never put raw login in logs/database. |
| Broker environment (`DEMO/LIVE/UNKNOWN`) | C/D | Provider response does not supply a universal trusted enum; use approved broker/server/account evidence and versioned registry policy. Never infer from server substring. |
| Quant environment (`PAPER/SHADOW/LIVE`) | A outside wire / D in bridge admission | Persisted on Quant account with verification provenance. Must remain separate from broker environment. |
| Account type/trade mode, margin level, leverage, trade permissions/account state | B/C/D | Some MT5 account-info attributes exist, but exact semantic mapping, broker variability and policy meaning are not established in current v2. Defer; do not invent null/default values in v2. |
| Account currency, balance, equity, margin used, free margin | A | Current `account_info` mapping; exact decimal strings and currency units; source timestamp absent; `captured_at` is collection time. |
| Symbol name, base/profit currencies, digits, point, tick size/value, contract size, volume min/max/step, trade mode | A | Current v2 instrument wire type. Worker requires several provider fields; currency/spec availability may vary. |
| Symbol description | B/D | Provider may expose `description`, absent from v2; defer. |
| Tick bid, ask, source tick time, per-symbol/aggregate freshness state | A | Current v2 includes fresh/stale/unavailable/invalid. Fresh age <=60 sec, future tolerance 5 sec; no fabricated quote. |
| Tick last and tick volume | B/D | Provider tick object may expose these; current v2 omits them. Defer pending semantic/unit decision. |
| Position ticket, symbol, side, volume, entry price, profit | A | Current reconciliation position structure; decimals as strings. |
| Position current price, SL, TP, swap, open time | B/D | Provider may expose these values, but v2 does not carry them. Defer; source-time semantics and netting/hedging meaning require review. |
| Open-order ticket, symbol, side/type, current remaining volume, status | A | Current worker uses `volume_current`; wire field `quantity` does not preserve initial volume separately. |
| Open-order initial volume | B/D | Provider exposes `volume_initial`; not represented in v2. |
| Open-order trigger/entry price, SL, TP, setup time | B/D | Provider may expose related fields; absent from v2. Setup-time semantics and order-type variants need normalization before inclusion. |
| External client order ID | C | External broker order may not carry a Quant client ID; current wire sends null. Do not infer lineage. |
| Last successful observation and last accepted sequence | A | Persisted capture/stream data keyed by `(account_id, terminal_id)`. |
| Last failed observation and normalized error category | C/D | Safe logs exist, but no normalized durable state in v2. Define categories without provider text/secrets before any future contract addition. |
| Worker restart counter | D | Not carried. Supervisor evidence may track process restarts; do not call it a protocol field. |
| Reconciliation readiness and snapshot | A | Durable stream readiness plus append-only complete reconciliation snapshot; market freshness remains independent. |
| Independent reconciliation revision | D | Protocol schema version is not a reconciliation revision; no field currently exists. Use accepted binding/stream revision metadata outside v2 until requirement is established. |

## Identity minimum outside v2 telemetry

The minimum admission data is not a new ownership assertion embedded in every market-data frame. Before data persistence, an authenticated control-plane exchange must resolve:

- registered workload certificate identity and assigned host/terminal;
- Quant account ID, organization entitlement and current account revision/environment;
- provider-observed login transformed to registry HMAC under retained key versions;
- exact broker/server catalog identity and broker environment;
- external account verification/evidence revision and binding revision;
- protocol/build identity and policy version.

The registry independently proves operator-approved ownership; the worker observation proves only what that workload reported. mTLS does not make a compromised worker truthful. The admission decision must bind both evidence sources and deny if they disagree.

## Compatibility, rollout and acceptance

Release manifest must pin source commit, artifact SHA-256, target OS/architecture, Python/provider package, daemon, wire schema, admission protocol and config revision. Bridge startup and worker preflight must reject unsupported versions before ingestion. No rolling mix of unverified old/new combinations: one compatible pair, one assigned stream, explicit drain/stop of old processes, new admission, initial reconciliation, then readiness.

Acceptance criteria: contract schema and compatibility range are pinned; generated fixtures conform to strict v2; all deferred fields are explicitly absent and never fabricated; v2 unsupported/unknown versions reject; old/new pair mismatch rejects before persistence; admission identity does not depend on ordinary telemetry claims; no execution operation is introduced.

