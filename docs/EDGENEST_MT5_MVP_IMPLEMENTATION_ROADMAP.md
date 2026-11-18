# EdgeNest MT5 MVP — Implementation Roadmap

**Goal:** first real MT5 DEMO account connected through a managed EdgeNest Windows gateway, with authenticated read-only account and market data.  
**Authority boundary:** this roadmap is planning, not deployment authorization. Broker order execution remains disabled and out of scope.  
**Audit basis:** `docs/EDGENEST_MT5_MVP_READINESS_AUDIT.md`.

## Product release definition

For the first MVP, a user can authenticate to EdgeNest, select a supported broker/server, connect one DEMO account through a managed Windows terminal, see connection/health status and current account information, market data, open positions and orders, and view history only if a separately accepted history capability is delivered. EdgeNest owns terminal, worker, daemon, workload certificates and recovery. The customer does not install an EA, local terminal or VPS.

The first pilot can be operator-assisted for broker/account selection and gateway assignment. Do not imply verified ownership until the R3 evidence and authority process has actually completed. Do not expose another organization's snapshots. The first implementation is one active publisher per external account; failover requires a lease/fencing design first.

## Milestones

### MT5-MVP-00 — Restore safe engineering test lane and freeze data contract

**Scope**

- Repair or reprovision the exact disposable QOS-020 resource only under its separately authorized infrastructure procedure; inspect its manifest and pass the unchanged identity guard.
- Do not use staging/production or the recovery dump. No shared migration.
- Approve the minimal v2 account/market/reconciliation and connection-health DTOs, freshness semantics, identifier binding and unsupported-history behavior.
- Reconcile QOS-020 database test availability with the recent guard failure (`docker inspect` could not find the manifest's container); until the guard passes, run no database-capable tests.

**Acceptance criteria**

- Unchanged QOS-020 guard verifies exact run/project/container/volume/network/database identity and cleanup check passes.
- SQLx tests use only the disposable instance and cleanup leaves no generated test DBs.
- Data contract has stable account/terminal/broker/server IDs, event sequence, source/captured timestamps, freshness and reconciliation version; never accepts client-supplied tenant scope as authorization.
- Order-execution capability remains absent/deny-all in the runtime path.

**Dependencies:** QOS-020 exact resource availability; architecture review.  
**Parallel:** Windows image and service-account design, no production host changes.

### MT5-MVP-01A — Single-account managed Windows DEMO vertical slice (**first engineering milestone**)

**Scope**

- Provision one approved Windows Server demo host/VM slot, one restricted service identity, one MT5 terminal installation/profile, one official MetaTrader5 Python worker and one Rust daemon. Keep worker and daemon colocated for loopback bridge transport.
- Obtain one DEMO account through an approved secure channel. Use a secret manager or short-lived protected delivery; do not store plaintext credentials in Postgres, logs, source, CI artifacts or frontend/browser storage.
- Generate/issue distinct server and worker mTLS identities under an approved CA; pin only the worker identity needed for the account; restrict key ACLs and host network access.
- Add a provisioning-time binding from the approved organization/Quant account/external broker account to observed login/server/environment and worker certificate/terminal identity. Treat observed heartbeat IDs as assertions until compared with that record.
- Run the worker, collect account state, symbol specifications/ticks, current open positions and open orders; persist market and reconciliation state into a dedicated development/demo database with the required forward migrations applied through the approved procedure.
- Capture redacted host/build/package/terminal identity evidence, a first successful connection, a worker-only restart and restored data flow. No terminal or credential operation is performed by the planning phase.

**Acceptance criteria**

1. The actual host OS, Python version, official package version, terminal build, daemon source/artifact version, worker source/artifact version and config revision are recorded.
2. The official provider starts and initializes the terminal under the intended service identity; exact DEMO server, observed login, currency and expected broker-account record match. LIVE/unknown environment is rejected.
3. The worker certificate is accepted by TLS 1.3 mTLS and maps to exactly one provisioned worker/account/terminal identity; unknown, expired, mismatched and retired certificates are rejected according to the documented policy.
4. Fresh ticks and symbol specifications, account information, open orders and positions reach the daemon and persist with matching account/broker/server/terminal identity, source timestamps and advancing sequence.
5. Readiness becomes healthy only after authentication, fresh market data, database persistence and full reconciliation; stale/unavailable data is visible as degraded, never represented as fresh.
6. Stop/restart of the worker alone reconnects without changing the daemon, reuses monotonic sequence state and persists a new complete reconciliation. Connection loss and recovery are observed in timestamped logs/metrics.
7. No submit/modify/cancel/close message is sent or accepted. Execution remains disabled by configuration and code boundary.
8. Logs/evidence are redacted and contain no password, complete database URL, private key, shared secret or raw account credential.

**Dependencies:** Windows access; dedicated demo account; approved certificate/secret owners; QOS-020 guard; migrations validated on disposable DB; minimal identity/tenant account mapping.  
**Not included:** general customer UI, multiple accounts per host, automatic failover, historical deals/bars or live accounts.

### MT5-MVP-01B — Tenant-scoped read APIs and connection dashboard

**Scope**

- Add authenticated endpoints for connection list/detail, current account state, latest market snapshot, positions/open orders and readiness/reconnect timestamps.
- Resolve every request through authenticated user → active organization membership → active account entitlement → verified broker binding → gateway/account record. Do not trust path IDs, account IDs, terminal IDs or `x-organization-id` without current server-side authorization.
- Return a masked login/account label and controlled display fields. Return explicit freshness and stale timestamps. Keep raw snapshot JSON and certificate identity private.
- Add customer UI: broker/server selection, chosen organization/account, status progress and errors that do not disclose secrets; account overview with balances, margin, currency, freshness, positions/orders and unsupported history notice.
- Start with operator-assisted gateway assignment if fully automated credential custody is not ready. Customer credential capture is not allowed until a reviewed secret-intake design exists.

**Acceptance criteria**

- API and UI show a real DEMO account only after the 01A accepted verification/binding state.
- Cross-organization reads, guessed IDs, suspended membership/entitlement, revoked binding and missing binding all return no data.
- Current dashboard state is read-only; no order action/control is rendered or accepted.
- Component/UI tests cover loading, disconnected, authenticating, connected/fresh, stale, degraded, revoked and database-unavailable states.
- Guarded PostgreSQL/API tests prove tenant boundaries before pilot customer data is accessible.

**Dependencies:** 01A identity model; QOS-020 restored; R2 membership/entitlement tests passing; I1 session remediation accepted before external customer exposure.  
**Parallel:** UX and API contract may be designed during 01A.

### MT5-MVP-01C — Secure credential intake and account enrollment

**Scope**

- Add a dedicated server-side credential intake/tokenization path over TLS with CSRF/session protections, strict request limits, structured secret redaction, and no browser persistence.
- Place credentials in an approved secret manager under per-account scope. Worker retrieves only its account secret using workload identity; never use a shared account-wide environment dump. Define retention, rotation, disable/delete, operator access audit and incident revocation.
- Validate with the official provider on the assigned Windows host; capture observed login/server/currency/environment, compare to requested broker/server/account, and produce signed/authenticated evidence. On success, create the pending binding/evidence record; never equate credential possession with independent legal ownership.
- Define user-facing challenge for any broker-required verification, independent platform review/approval authority, and transparent status lifecycle.

**Acceptance criteria**

- Automated scans/tests show submitted password never appears in logs, traces, analytics, error responses, database rows, queues, browser local/session storage or CI output.
- Secret access is scoped to one gateway/account, auditable, revocable and rotated; revoked credentials cannot reconnect.
- Invalid credentials, wrong server, wrong account, LIVE/unknown environment and mismatched binding fail closed without creating an active binding.
- A connection check proves current access; separate evidence/approval proves any platform-defined ownership/verification status.

**Dependencies:** reviewed secret manager and key custody; secure Windows workload identity; R3 evidence/authority design; I1 for customer sessions. I2/I3 are required before privileged platform approval, not automatically before an ordinary account connection.

### MT5-MVP-02 — Gateway registration, operations and scale-out

**Scope**

- Create gateway/terminal/worker desired-state registry; assign one terminal session per external account and unique workload identity.
- Add supervisor/installer with deterministic startup ordering (secret/identity → terminal → daemon connectivity → worker), bounded restart/backoff, failure classification, logs, metrics and alerts.
- Add durable account lease/fencing epoch accepted by daemon and persistence. Prevent stale/duplicate publishers across daemon replicas, host replacement and network partitions.
- Define certificate issuance, overlap rotation, revocation, host rebuild, terminal update, backup/restore and access audit.
- Add resource quota/capacity measurements to set a supported terminals-per-host limit; default to one isolated account per service identity/slot until proven otherwise.

**Acceptance criteria**

- Simulated duplicate/stale gateway epochs are rejected by both ingestion and persistence.
- Host reboot, terminal crash, worker crash, daemon restart, database outage and certificate expiry are detected and recover or remain safely degraded.
- No automatic failover promotes a second writer without acquiring a current fenced lease.
- Operational dashboard/alerts identify gateway, account and connection state without exposing credentials.

**Dependencies:** accepted 01A/01B data contract and host evidence; lease/fencing design. Multi-host failover is not a precondition for the first single-host demo pilot.

### MT5-MVP-03 — Optional historical data capability

**Scope**

- Decide customer-visible historical scope: deal history, historical orders, bars, or a bounded subset.
- Extend the official Python provider, versioned bridge schema, Rust validation, persistence and tenant-scoped API as one compatible change. Add per-query time range/row limits, deduplication IDs, pagination and rate limiting.
- Treat broker retention/permissions as variable; return explicit unsupported/partial history states.

**Acceptance criteria**

- Contract tests cover boundaries, duplicate/replay, partial broker response, timezone/source timestamps, payload limits and tenant isolation.
- Real demo-host evidence shows matching provider records and persisted records with no synthetic history.

**Dependencies:** 01A data identity and 01B read API. This milestone can remain deferred if first release only requires current state and ticks.

## Recommended end-to-end order

1. Restore QOS-020 test identity and guard; run only isolated, guarded database tests after verification.
2. Freeze the read-only identity/data contract and one-account binding model.
3. Execute 01A on one managed Windows DEMO terminal; do not wait for I2–I5 blanket completion, but do resolve I1 and tenant controls before customer exposure.
4. Complete 01B API/UI and cross-tenant tests.
5. Review and implement 01C secure credential intake before customers submit credentials themselves. Until then use an explicitly approved operator-assisted enrollment path.
6. Add 02 operations/fencing before multiple gateways, duplicate host deployment, or automatic failover.
7. Add 03 only if history is an MVP requirement. Be transparent that the existing adapter has no MT5 deal-history or rates/candles support.
8. Keep execution work separate. QOS-021A E1/E2/E3, QOS-013 and the QOS-015 target-host release evidence gate remain required before any later live-execution program.

## Runtime architecture target

```text
Customer browser
  └─ EdgeNest web/API (session + current organization membership/entitlement)
       ├─ Broker/server catalogue + account enrollment API
       ├─ Secret manager (per-account credential scope; no credential in DB/UI logs)
       ├─ Account/binding/gateway registry (verified binding + terminal/workload identity)
       └─ Tenant-scoped read API/dashboard
             ▲
             │ PostgreSQL: broker binding, gateway lease, connection status,
             │ account/reconciliation snapshots, latest market snapshot
             │
Managed Windows gateway (isolated service identity; one terminal/account slot)
  ├─ Official MT5 terminal + MetaTrader5 Python package
  ├─ Read-only Python worker (credential from secret manager; dedicated mTLS identity)
  └─ Rust bridge daemon (loopback listener, PostgreSQL connection, bound account scope)

No customer EA/VPS. No broker order execution. No unfenced automatic failover.
```

## Operational and security release gates

- Windows host build, patching, service identity, interactive/headless terminal behavior and terminal auto-update policy are documented and tested.
- Only DEMO servers are selectable in the pilot; provider cannot silently switch to a live server. Worker and daemon independently check the expected server/environment.
- Passwords and client private keys are separately protected. Key files are ACL-restricted to the service identity; rotation/revocation is rehearsed with old-identity rejection.
- mTLS authenticates a workload, not a customer account owner. Registry binding and tenant entitlements are resolved server-side on every API read.
- Freshness timestamp is the broker tick timestamp, not the database write time. Stale/unavailable/invalid is visible and cannot be presented as live.
- Reconnection always includes a complete account/open-order/position snapshot; sequence gaps require reconciliation. Duplicate terminal publication is fenced.
- Migration applies only to a specifically approved, backed-up isolated/pilot database after fresh-install and supported-upgrade rehearsal; no staging/production is implied by this roadmap.
- Customer-facing connection state includes recovery status and last successful data time, with alerts on prolonged disconnection.
- MT5 worker and daemon remain read-only; verify no path allows order submission/cancel/modify. Trading remains disabled.

## Exclusions

- Live accounts, broker order execution, order modification/cancellation/close, signal-driven trading or risk-authorized sending.
- Customer-installed EA, customer-run terminal or customer-maintained VPS.
- Automatic gateway failover before lease/fencing is implemented.
- Treating account-number entry or successful credential login as privileged ownership verification.
- Historical deals/bars until MT5 provider/protocol/persistence/API support is designed and accepted.
- Production migration, deployment, credential use, terminal installation or host change by this roadmap document.

## Exact next action

After review and separate implementation authorization, start with **MT5-MVP-00**, because the last authorized QOS-020 guard attempt failed to inspect the exact container ID in its unchanged manifest. Do not run database tests until that guard passes. Then implement and run **MT5-MVP-01A** on a single managed Windows DEMO host. Do not start broad UI/self-service credential intake before 01A proves the official provider, identity mapping, persistence and restart path.
