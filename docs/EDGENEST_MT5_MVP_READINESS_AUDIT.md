# EdgeNest MT5 MVP — Managed Connectivity Readiness Audit

**Audit type:** repository and documentation review; read-only  
**Decision:** **NOT READY** for customer self-service or a verified real-terminal connection  
**Objective assessed:** first real MT5 DEMO-account connection through a managed EdgeNest gateway, read-only only  
**Source baseline:** `/Users/olaoluwatunmise/event-driven-trading-rs`, branch `main`, HEAD `f6446dd33dc74a86ede9064f35aef2ec082a1c59`

## Executive assessment

The repository already contains a meaningful read-only data-plane foundation: a Windows-only wrapper over the official MetaTrader5 Python package; a schema-v2 worker protocol; a Rust TLS 1.3 mTLS bridge; account, open-order, position, symbol-specification and tick snapshots; sequence checks; freshness handling; reconciliation persistence; and tenant/account broker-registry foundations.

It does **not** yet provide the product path in the objective. There is no customer-facing broker connect workflow, no credential intake and managed secret-delivery path, no gateway/terminal provisioning controller, and no demonstrated binding from an organization’s broker-account registry record to a particular worker certificate, terminal session and persisted account. Broker-account registration currently creates pending records; it does not prove ownership or provision a gateway. Existing account screens create/display Quant OS paper accounts, not connected MT5 accounts.

No managed Windows host, installed terminal, demo login, service state, database contents, credentials, or deployed certificate were inspected in this audit. Source and test code establish intended behavior, not Windows terminal conformance. No build, test, migration, database operation, Docker operation, host configuration, broker login, deployment, or trading operation was performed.

**Shortest safe path:** a controlled, one-organization/one-account/one-terminal Windows DEMO vertical slice. First make the existing worker/daemon path an authenticated, identity-bound and observable managed service; then persist and expose its read-only state through tenant-scoped APIs and a minimal customer connection/status screen. Keep gateway assignment and credential intake operator-assisted for this first slice if that is the shortest reviewed secret-handling path. Do not present the current broker registry's pending state as verified ownership.

## Evidence classification

- **Implemented:** source has a concrete path for the stated function.
- **Partially implemented:** a foundation exists, but a required link or invariant is missing.
- **Missing:** no implementation was found in the inspected source/UI.
- **Implemented but not qualified:** code exists, but required real-host, security, integration, or operational evidence is absent.

## Existing implementation inventory

| Area | Status | Evidence and limits |
|---|---|---|
| Official Windows MT5 provider | **Implemented but not qualified** | `adapters/mt5-python/mt5_adapter/worker.py`, `MetaTraderTerminal`: rejects non-Windows, imports `MetaTrader5`, calls `initialize(path, login, password, server)`, checks terminal connectivity and exact configured login/server, and wraps account/orders/positions/symbol/tick calls. `pyproject.toml` declares the optional `MetaTrader5` extra with a Windows platform marker. No Windows conformance evidence was inspected. |
| Worker configuration and read-only collection | **Implemented but not qualified** | `Config.from_env` requires configured login/server/terminal ID, account ID, bridge and client TLS paths, and a bounded symbol allowlist. `build_snapshot` captures currency, balance, equity, used/free margin, current orders and positions. `build_market_data_snapshot` captures symbol metadata and bid/ask tick state. The worker protocol has no history-deals or rates/candles method. |
| Worker reconnect and supervision | **Partially implemented** | `run_worker` retries provider/TLS/socket/protocol failures with in-process exponential backoff up to 30 seconds and shuts down in its finalizer. The repository has smoke/restart PowerShell scripts and macOS LaunchAgent examples, but no inspected Windows service definition, host provisioning controller, process watchdog policy, crash-loop alerting, or tested terminal relaunch/recovery evidence. In-process retry is not host-level process supervision. |
| Mac/EA provider | **Implemented but not qualified; not production choice for this objective** | `macos_socket_provider.py` and `mql5/QuantOSReadOnlyProvider.mq5` implement the experimental read-only loopback provider. Documentation explicitly says real-terminal conformance is pending. This audit follows the required official Windows Python-provider production direction; the EA is not a substitute. |
| Rust bridge daemon | **Implemented but not qualified** | `crates/intent-bridge/src/bin/mt5-bridge-daemon.rs` accepts one configured account ID, terminal ID, broker-server string and client-certificate fingerprint; connects to PostgreSQL; binds loopback by default; and consumes authenticated worker frames. It has no HTTP health endpoint or gateway registration/orchestration API in the inspected daemon. Its startup log is not a customer-facing readiness API. |
| Bridge protocol | **Implemented** | `crates/intent-bridge/src/mt5_v1.rs`, despite its historical module name, parses versioned frames including schema v2; `docs/contracts/mt5_bridge_v2.schema.json` documents the v2 contract. Worker and daemon must be upgraded together. Heartbeats explicitly advertise order/cancel capabilities as false. `mt5_dispatch.rs` is not wired to this read-only daemon path; E1 documentation records MT5 submit deny-all. |
| mTLS and workload identity | **Partially implemented** | `mt5_transport.rs` enforces TLS 1.3, client-CA chain validation, and exact leaf-certificate SHA-256 allowlisting; the daemon pins a configured certificate to configured terminal/server/account values. The worker validates the server CA/name and loads a client certificate/key. Certificate issuance, secure delivery, revocation enforcement policy, rotation, renewal, tenant-to-certificate registry and automated replacement are external/unimplemented. A heartbeat/configuration identity is not, by itself, broker ownership proof. |
| Broker/server catalogue | **Partially implemented** | `crates/domain/src/broker_registry.rs`, `crates/persistence/src/repos/broker_registry.rs`, and `crates/journal-api/src/broker_registry.rs` provide catalogue and tenant-scoped registry foundations. Migration `20261113000001_broker_account_registry.sql` models broker/server, external account HMAC tokens, organization binding, evidence and verification history. The API register path accepts an account identifier, HMACs it and creates pending records; active verification and gateway orchestration are not supplied by this path. |
| Organization ownership and entitlements | **Partially implemented** | R2 organization memberships and account entitlements are checked in the API/repository layer. Broker registration requires an active organization OWNER and active entitlement to an existing Quant account. R2 documentation says the database tests were blocked and not run because QOS-020 identity validation failed; tenant isolation therefore remains unqualified by the reported database evidence. |
| Customer broker connection workflow | **Missing** | No frontend route or form calls the broker catalogue/binding endpoints. `frontend/src/app/accounts/page.tsx` is a Quant account list and paper-account creation page; it expressly describes paper accounts and does not initiate broker connection. No secure credential-entry or enrollment flow was found. |
| Gateway/terminal lifecycle management | **Missing** | No control-plane service provisions a Windows host, terminal profile/data directory, worker process, daemon process, client certificate, or one-account assignment. No desired-state reconciliation or duplicate-publisher lease/fencing mechanism was found. |
| Account/terminal identity binding | **Partially implemented; critical gap** | Worker locally verifies its configured numeric MT5 login and server. It sends configured `account_id`, `terminal_id`, and server in frames; the daemon checks those values against its configured principal. The external account registry stores a keyed HMAC, but no inspected code joins that HMAC/account binding to the daemon's allowed account ID, certificate, terminal, or observed MT5 login. The numeric MT5 login is not carried as an authenticated registry comparison value in the snapshot. Configuration agreement does not establish that the configured external-account record belongs to the organization. |
| Event/data persistence | **Partially implemented** | `mt5_bridge_market_data_snapshots` stores only the latest snapshot per `(account_id, terminal_id)` (migration `20261027000004_mt5_market_data_snapshots.sql`). `mt5_bridge_reconciliation_snapshots` appends complete account/order/position snapshots (migration `20261027000003_mt5_reconciliation_snapshots.sql`). Stream sequence and readiness are persisted in `mt5_bridge_streams`. No customer API was found that returns these bridge snapshots through tenant authorization or maps them to the general customer account/position/order dashboard model. |
| Reconnection and reconciliation | **Implemented but not qualified** | `mt5_runtime.rs`, `mt5_reconciliation.rs`, `mt5_session.rs`, and `mt5_bridge.rs` enforce heartbeat/sequence/session checks, degrade on faults, persist complete reconciliation, and require a fresh snapshot to restore readiness. Daemon accepts a stream and discards it on failure; worker reconnects. No live Windows recovery/reconciliation evidence or cross-replica fencing exists. |
| Health/readiness | **Partially implemented** | Rust has `Mt5HealthSnapshot` signals and durable stream readiness; fresh/stale market data is represented explicitly. The daemon itself does not expose a health endpoint, and no tenant-scoped customer connection-status endpoint or alerting/metrics pipeline was found. `market-data/src/mt5_latest.rs` offers a server-configured latest quote provider, not a general per-tenant MT5 account dashboard. |
| Reconciliation of history | **Missing for MT5 historical deals** | Current worker sends only current open orders and positions. `crates/market-data/src/providers/mt5.rs` explicitly reports candle/rates history as not implemented. No `history_deals_get` call or historical-deal frame/persistence path was found. |

## Official Windows provider assessment

### What source implements

`MetaTraderTerminal` in `worker.py` is explicitly Windows-only. It imports the official `MetaTrader5` package and passes the configured terminal executable path and account login/password/server to `initialize`. It verifies `terminal_info().connected` and the returned `account_info()` login/server before accepting the session. Collection calls cover account info, `orders_get`, `positions_get`, `symbol_select`, `symbol_info` and `symbol_info_tick`. A failed provider call raises an adapter error with an MT5 error code; credentials and provider response bodies are not intentionally logged by the worker. `shutdown()` calls the package shutdown method.

`run_worker` retries exceptions with exponential backoff inside the worker process. It persists a sequence checkpoint before each heartbeat/market/reconciliation frame set, uses TLS 1.3 and validates the daemon server name/CA, and sends the client certificate. The worker is single-configuration: one login, server, account ID, terminal ID and symbol set per process. This is a sound starting unit for one-account isolation, not a complete multi-account service manager.

### What source does not establish

- The official package and `initialize` lifecycle work with the chosen Windows Server image, terminal build, broker, or headless/service session.
- Who starts/stops/restarts `terminal64.exe`, how profiles/data folders are isolated, how terminal auto-update/reboot is handled, or whether API shutdown consistently releases the terminal under service-account operation.
- Secure password intake, encryption at rest, vault delivery, rotation, revocation, deletion, or access auditing. Worker configuration reads `MT5_PASSWORD` from its environment; the repo README says it is never logged, but there is no customer secret workflow.
- How concurrent terminals under isolated accounts behave on one host. No concurrency or resource-sizing qualification was found; default safe assumption is one terminal session per isolated Windows service identity/host slot until measured.
- Broker-side ownership proof. A successful login proves the supplied credentials could authenticate at that time; it does not independently prove legal account ownership or customer authorization to bind it.
- Historical deals or bars/candles. No deal-history or rates call exists in the worker's `Terminal` protocol. Current orders/positions are open state, not account history.

### Platform test boundary

Portable Python contract tests with fakes can validate mapping, retry behavior, protocol bounds and redaction contracts, but cannot exercise the official Windows-only package. The target Windows host must separately prove installed Python/package/terminal compatibility, account/server identity, process lifecycle, terminal reconnect, account-state retrieval, symbol selection, fresh ticks, positions/open orders, TLS, persistence and service restart. MQL5 compilation and the Mac EA are not evidence for this production provider.

## Target account connection journey and gaps

| Step | Desired behavior | Current state / gap |
|---|---|---|
| 1. Sign in | Customer authenticates and gets a revocable EdgeNest session. | Identity API and frontend login exist. The I1 acceptance audit recommends conditional acceptance; session-suspension remediation was blocked by QOS-020 guard failure. Resolve I1 before external customer onboarding. |
| 2. Select broker/server | Show only active, supported DEMO servers and provider capabilities. | Authenticated catalogue API exists. No customer UI; catalogue contents/runtime validity are not verified here. |
| 3. Choose organization/account | Resolve current organization membership and account entitlement, not a caller-selected arbitrary account. | R2 tenant context/entitlement foundations exist. Broker registration requires OWNER and an already entitled Quant account. DB-backed R2 verification was reported blocked; current production confidence is limited. |
| 4. Submit credentials safely | Collect MT5 login/password over protected HTTPS, keep out of browser storage/logs/database, and deliver only to the assigned workload through a secret manager or short-lived encrypted channel. | **Missing.** Current registry accepts an external account identifier for HMAC lookup; it does not accept or manage MT5 credentials. No gateway secret-delivery integration exists. |
| 5. Validate broker access | Use official API on the managed Windows host; verify exact server, login, environment is DEMO, currency and live terminal connectivity. | Worker validates configured login/server and reads currency, but has no explicit DEMO/LIVE assertion in its protocol/config validation and no registered-account comparison. Add a server-side onboarding verifier and fail closed if environment cannot be established. |
| 6. Bind tenant and external account | Record an audited organization ↔ broker/server ↔ external-account binding after evidence and approval policy. | Pending registry records and HMAC identity indexing exist. Registration alone is not proof. Evidence ingestion, attestation/key authority, approval integration and runtime authorization mapping are incomplete. |
| 7. Provision gateway | Assign one isolated terminal/worker/daemon identity and mTLS certificate; prevent a second active publisher for the same account. | **Missing.** Daemon config is static and single-account; there is no gateway registry/controller, lease/fencing, certificate lifecycle or duplicate-publisher control across replicas. |
| 8. Ingest | Authenticate mTLS worker; compare authenticated workload to provisioned account binding; validate freshness/schema/sequence; persist snapshot. | Transport, protocol validation, persistence and stream readiness exist. Strong registry-to-workload-to-observed-login binding is missing. |
| 9. Display | Show connection state, exact broker/server, masked login, currency/balance/equity/margin, freshness timestamps, positions/open orders, and supported history; scope each read to organization membership/entitlement. | **Missing for MT5-connected accounts.** Existing dashboard APIs/UI do not expose bridge snapshots through tenant-scoped customer endpoints. |

## Read-only data contract coverage

| Data | Current provider/protocol | Persistence/exposure status |
|---|---|---|
| Account identity | Local worker validates numeric login and exact server; frame account ID/terminal/server are configured identity values. Login itself is not sent as a registry-verifiable identity claim. | No complete external-account-HMAC ↔ observed login ↔ org entitlement ↔ worker certificate join. |
| Account state | Currency, balance, equity, margin used and free margin. | Complete JSON reconciliation snapshots are persisted append-only. No identified MT5 tenant dashboard projection/API. |
| Symbol specifications | Exact configured symbol, currencies, digits, point, tick size/value, contract and volume bounds, trade mode. | Stored inside latest market-data JSON snapshot per account/terminal; not an instrument history series. |
| Market data | Configured-symbol bid/ask with source timestamp, explicit fresh/stale/unavailable/invalid state. | Latest snapshot only; a lower sequence cannot overwrite it. A market-data consumer exists for configured scope, but no customer-scoped account feed/dashboard was found. |
| Open positions | Ticket, symbol, side, volume, open price and unrealized P&L. | Included in append-only reconciliation snapshots. No direct verified customer API projection found. |
| Pending/open orders | Ticket, symbol, side/type, current volume and state. | Included in reconciliation snapshots. This is read-only observation; order placement/cancellation is not available to the worker. |
| Historical deals | No provider method or frame. | Not transported or persisted. Must be added if included in MVP; otherwise expose this as unsupported in the first release. |
| Rates/candles | Explicitly not implemented by `Mt5Provider`. | Not transported/persisted as MT5 historical bars. |
| Connection/recovery state | Heartbeats, sequence, session/reconciliation and market freshness signals. | Stream readiness is persisted; no daemon health endpoint, gateway monitoring integration or tenant-scoped status UI. |

## Multi-tenant and identity risk assessment

The intended chain is:

`identity user → active organization membership → account entitlement → broker/server catalogue → external broker account → verified binding → gateway assignment → mTLS worker principal → terminal session → daemon account scope → persisted snapshots → tenant-scoped read API`.

The first four links have partial implementation in the Identity, R2 organization and R3 registry work. The active verification and runtime links are not complete. The daemon currently relies on static configuration and frame identity assertions. The registry's HMAC prevents storing a raw account identifier and provides duplicate lookup, but it does not establish ownership; no code path was found that validates incoming MT5 telemetry against the HMAC record and exact organization entitlement.

Risks to close before customer data is exposed:

1. **Cross-tenant read:** bridge snapshot tables are keyed by account/terminal, not organization. Every customer query must resolve authorization from server-side binding and entitlement before selecting that key; never accept raw account/terminal IDs as authority.
2. **Publisher collision:** certificate allowlisting and one sequential daemon process do not fence duplicate daemon replicas or stale gateway publishers. No automatic failover is safe until a durable lease/fencing token, owner epoch and rejection of stale epochs are enforced end-to-end.
3. **Identity ambiguity:** terminal/server/account ID are configuration assertions. Establish observed login/environment matching to the protected external-account binding during onboarding and bind workload certificate to that record.
4. **Credential custody:** plaintext password exists in worker process configuration. Do not put it in PostgreSQL, logs, frontend persistence, deployment manifests or general environment dumps; use a managed secret store with per-account access and short-lived retrieval.
5. **Tenant foundation qualification:** R2 documentation records QOS-020 database integration tests not run. Do not expose customer MT5 snapshots until tenant-scope API tests pass under the isolated QOS-020 guard.
6. **Privileged verification:** a customer-provided account number or successful login cannot be presented as EdgeNest's privileged verification. R3A evidence/authority and R3B step-up requirements govern sensitive platform approvals.

## QOS dependency map

| Dependency/workstream | Before first controlled DEMO connection | Before external customer onboarding | Before privileged account verification | Before live execution | Parallel work |
|---|---|---|---|---|---|
| Official Windows provider and QOS-015A host conformance | **Required** | **Required** | — | **Required** | Build packaging and test harness can start now. |
| MT5 v2 bridge, mTLS and persistence | **Required**, including an isolated target DB with approved schema | **Required** | — | **Required** | Add identity mapping and health contract in parallel with UI design. |
| QOS-020 isolated DB test infrastructure | Required for safe DB integration/migration regression; current guard failed to inspect its container in the last authorized attempt, so tests remain blocked until repaired/verified | **Required** for tenant isolation and schema confidence | Required for approval/evidence persistence tests | Required for all persistence/authorization tests | Repairing/renewing exact QOS-020 identity is independent infrastructure work. No fallback to staging. |
| QOS-021A-E3A-R2 organization/tenant foundation | A controlled internal demo can use an explicitly provisioned internal tenant, but never rely on untested tenant filters for customer access | **Required**: qualify membership/entitlement and cross-tenant API isolation | **Required** for resource scope | **Required** | Complete API-level tests. |
| QOS-021A-E3A-R3/R3A broker registry, evidence and authority | Minimal internal demo may use reviewed operator provisioning with no ownership claim; do not mark the account `VERIFIED` based only on login | **Required** for account registration, evidence and safe reassignment | **Required** | Required before any broker-linked execution authorization | Finish evidence ingestion and authority model independently of gateway packaging. |
| QOS-021A-E3A-R3B-A I1 sessions | **Required** for a customer-authenticated journey; internal host proof may be operator-assisted | **Required**; acceptance audit is conditional and remediation is blocked until QOS-020 guard works | **Required** | **Required** | Resolve suspension/revocation and guarded regression gates. |
| I2 WebAuthn | **Not blanket-required** for a read-only internal demo or ordinary customer login | Not a blanket dependency for a customer connecting their own account under normal OWNER authorization; may be required by policy for specific high-risk actions | **Required** for privileged approver step-up under accepted ADRs | **Required** for privileged execution approval if policy design requires it | Can develop after I1. |
| I3 operation-bound step-up; I4 recovery/audit; I5 security acceptance | Not blanket prerequisites for a tightly controlled read-only internal demo; keep privileged mutations unavailable | Required before enabling privileged platform operations; I4/I5 before their recovery/security release | **Required** as the approved sensitive-operation lifecycle (I3/I4) and validation (I5) | **Required** for privileged execution authorization | Can proceed in parallel with non-privileged gateway work after dependencies are respected. |
| QOS-021A-E1/E2/E3 execution authorization and provenance | Not required to ingest/display read-only account data | Not required for read-only account connection | Not required unless privileged approval targets execution | **Required**, plus independent execution risk controls and acceptance | Keep broker dispatch disabled; workstreams remain separate. |
| QOS-015B/QOS-013 and release evidence | Fresh feed and persistence checks are required for connected-data acceptance; sizing gates are not needed to show read-only data | Required where risk/sizing features consume MT5 data | — | **Required** for execution readiness | Data quality work can proceed in parallel. |

No I2–I5 blanket prerequisite is imposed for the first read-only demo connection. The specific dependency is narrower: I1 session safety and tenant-scoped reads are required before exposing customer data; WebAuthn/operation-bound proof is required for privileged approval actions, not ordinary read-only connectivity by default.

## Security, reliability and explicit exclusions

- Keep every provider and API capability read-only. No order submit, modify, cancel, close-position, or broker execution request is in scope.
- Do not enable `MT5_BRIDGE_ALLOW_REMOTE_BIND` without an explicitly approved private network/firewall model. Prefer colocated daemon/worker and loopback for the first slice.
- Use TLS 1.3 mTLS with individual workload certificates. CA/fingerprint rotation, revocation and key custody need an operator-owned lifecycle and alerting; current static config requires controlled process replacement to rotate.
- Keep per-account worker configuration and secrets isolated. Do not share a Windows user profile or credential scope across customer accounts.
- No automatic failover until lease/fencing prevents old and new gateways publishing concurrently.
- Do not claim account ownership from account number, server name, successful login alone, or a pending registry row. Distinguish connectivity proof from ownership/authorization evidence.
- Historical deals and candle history are excluded from the first connection slice unless the scope is expanded and protocol/storage/API acceptance is added. Clearly mark unsupported history to customers.
- No production deployment, shared staging migration, credentials, MetaTrader installation, Windows host modification, or trading execution was performed or authorized by this audit.

## Recommended order and exact first milestone

Recommended sequence and detailed acceptance gates are in `docs/EDGENEST_MT5_MVP_IMPLEMENTATION_ROADMAP.md`.

**Exact first engineering milestone: `MT5-MVP-01A — Single-account managed Windows DEMO vertical slice`.** Provision one isolated Windows service identity and terminal slot; use the official MetaTrader5 Python provider; configure one demo login via an approved secret source; use one dedicated mTLS worker identity; connect to the read-only Rust daemon; verify the observed login/server/environment against an operator-approved external-account record; persist fresh market, account, position and open-order snapshots into an isolated approved database; and produce redacted, timestamped host/reconnect evidence. Do not build general multi-tenant self-service or enable order execution in this milestone. The next milestone turns this proven path into tenant-scoped APIs/UI and customer-safe credential orchestration.

## Audit limits and preservation

This audit inspected source, migrations, documentation, and frontend code only. No credentials or environment values were printed or inspected. No database or Docker state, Windows host, terminal, process, certificate material, or broker account was accessed. No build/test was run. The pre-audit worktree had 97 non-ignored status entries, including staged/unstaged/deleted/untracked work; these were left intact. The only authorized file additions for this task are this audit and its roadmap.
