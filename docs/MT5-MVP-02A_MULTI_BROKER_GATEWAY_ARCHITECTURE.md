# MT5-MVP-02A — Multi-Broker Terminal Fleet on EdgeNest-Managed Windows Gateways

**Status:** Architecture decision proposal for review. No host has been provisioned and no multi-terminal runtime implementation is claimed.
**Product requirement:** Customers must never need to purchase, configure, or operate a VPS, Windows machine, MT5 terminal, EA, Python worker, or gateway.
**Initial infrastructure direction:** Contabo Cloud VPS 6, Windows Server, USA region, subject to checkout verification and broker-latency qualification.
**Initial broker:** TenTrade DEMO.
**Execution boundary:** Read-only telemetry only. Order submission, modification, cancellation and close remain disabled.

## 1. Decision summary

EdgeNest will operate a centrally managed gateway fleet. The customer connects to EdgeNest through the product interface; EdgeNest owns and operates the Windows hosts, MT5 terminals, workers, bridge daemons, certificates, secrets, monitoring, upgrades and recovery.

The architecture will support multiple MT5 broker brands and broker server environments from the beginning. A broker is represented by a canonical broker/server catalogue entry; a running terminal is represented by an isolated terminal slot; and an external broker account is connected to a verified EdgeNest account binding.

A Windows host may eventually run multiple isolated terminal slots. It must not be assumed that a single terminal process, profile, worker identity, sequence file, certificate or daemon principal can safely serve multiple unrelated broker accounts.

**The first live-host acceptance remains deliberately narrow:** one TenTrade DEMO account on one terminal slot. The repository and provisioning model will be designed for additional broker slots, but additional terminals are activated only after the first slot passes host acceptance and the multi-slot capacity/isolation checks pass. This preserves the canonical MT5-MVP-01A acceptance boundary and does not silently redefine MT5-MVP-01B or MT5-MVP-02.

## 2. Product boundary: customers do not operate infrastructure

The customer-facing workflow is:

1. Authenticate to EdgeNest.
2. Select an EdgeNest-supported broker and server.
3. Select or enroll an account through the currently approved enrollment process.
4. View connection progress, current account state, market data and supported positions/orders.
5. Continue using EdgeNest even if EdgeNest replaces or relocates a gateway.

The customer is never instructed to:
- rent or configure a VPS;
- install Windows, MT5, an EA, Python or Rust;
- keep a desktop or laptop online;
- expose an RDP endpoint;
- manage certificates, terminal profiles, worker processes or gateway restarts.

For the initial pilot, account enrollment and gateway assignment remain operator-assisted. Do not collect customer broker passwords until the reviewed MT5-MVP-01C credential-intake and secret-custody gate is complete.

## 3. Supported broker scope

### 3.1 MT5 broker support

The first connector family is MetaTrader 5. Multiple broker brands can be represented when they provide a compatible MT5 server and the official Windows MetaTrader5 integration can connect to their terminal build and account environment.

Broker support is not inferred from the presence of the MT5 logo alone. Each broker/server combination requires a conformance record covering:
- official terminal download source, publisher and version/build;
- exact broker server identifier and environment;
- DEMO account availability and account type;
- login and read-only credential behavior;
- symbols, specifications, quote freshness and account-currency semantics;
- terminal launch/session behavior and restart/reconciliation;
- broker/prop-program terms governing automated access.

MT4, cTrader, FIX, proprietary REST/WebSocket APIs and other platforms are separate connector families. They are not automatically supported by the MT5 adapter.

### 3.2 Broker and server identity

Keep the canonical broker and server catalogue independent of terminal installation identity:
- broker ID: stable EdgeNest catalogue identity;
- broker server ID: stable identity for the exact broker server/environment;
- environment: DEMO, PAPER or LIVE, explicitly separated;
- terminal slot ID: immutable identity of one managed terminal installation/profile;
- worker principal: unique workload identity assigned to one terminal/account slot;
- external broker account: protected external identity, never a customer authorization token;
- verified binding: authoritative mapping from organization/EdgeNest account to external broker account, server and environment.

Do not use a terminal label, broker display name, numeric login, customer-supplied path ID or mTLS certificate by itself as proof of account ownership or tenant authorization.

## 4. Windows gateway and terminal-slot model

### 4.1 Host

A gateway host is a managed Windows Server asset with:
- provider, region, instance ID and private asset inventory record;
- supported Windows build, patch/reboot owner and encrypted storage;
- restricted remote administration and audited break-glass access;
- approved outbound network policy, clock synchronization and monitoring;
- one host supervisor/controller;
- a measured resource budget and a declared maximum active terminal-slot count.

Initial purchase target: Contabo Cloud VPS 6 Windows, USA. The vendor's current public offer describes a 6-vCPU, 12-GB RAM Cloud VPS 6 Windows/Meta Trading configuration. The checkout must confirm that the selected US region, Windows licence, storage type, renewal price, taxes and setup charges are included within the approved budget. US Central (St. Louis) is the first region to evaluate, not a permanent broker-latency decision.

### 4.2 Terminal slot

Each account-connected terminal slot has its own:
- broker/server/environment assignment;
- terminal executable and install/update provenance;
- terminal data directory and profile;
- dedicated non-admin Windows service identity/SID;
- interactive terminal session model validated for the selected MT5 build;
- Python virtual environment and pinned official MetaTrader5 package;
- worker process and bounded supervisor policy;
- account-scoped sequence checkpoint and local state directory;
- unique client certificate/private-key identity and server-side assignment;
- account binding, lease/fencing identity and readiness state;
- redacted logs, metrics and evidence record.

Never share a writable terminal data directory, sequence file, worker identity, private key or account secret between slots. If broker-branded terminal packages differ, install and pin them separately. Do not assume that a terminal can be safely reused across brokers simply because both expose MT5.

### 4.3 Host-level daemon and admission

The current bridge daemon is configured with a static principal/account/terminal/server allow-list. That is suitable for the current single-slot acceptance path; it is not proof of a dynamic multi-account or multi-broker gateway.

Before activating multiple slots on one host, implement and review one of these explicit models:
1. a host-level daemon with a registry-backed set of independently authenticated worker principals and per-stream account/terminal scopes; or
2. isolated daemon instances/listeners per slot, each with a separate principal and resource boundary.

Do not simply widen the current static account allow-list or reuse one certificate across brokers. The selected model must be supported by tests for cross-slot isolation, stale/duplicate publishers, revocation, lease expiry, reconnect and persistence identity.

The daemon must fail closed when broker binding, organization entitlement, worker assignment, certificate identity, terminal identity or registry state is missing, mismatched, revoked or stale.

## 5. Multi-broker topology

Target topology:

```text
EdgeNest customer dashboard / developer API
                 |
      Authenticated tenant-scoped APIs
                 |
     Broker/server catalogue + account registry
                 |
       Verified account-to-slot assignment
                 |
      EdgeNest-managed Windows gateway pool
                 |
       +---------+------------------+
       |                            |
  Terminal Slot A              Terminal Slot B
  TenTrade DEMO                Other MT5 broker DEMO
  Worker A + identity A        Worker B + identity B
  Isolated profile/state       Isolated profile/state
       |                            |
       +---------+------------------+
                 |
     Authenticated read-only ingestion
                 |
       Quant OS normalized data model
                 |
       Tenant-scoped EdgeNest APIs
```

A gateway host can contain multiple terminal slots only after measured capacity and isolation acceptance. The first pilot starts with one active TenTrade slot; a second broker slot is a controlled qualification exercise, not an assumption that the host can support unlimited terminals.

## 6. Capacity and placement policy

Do not publish a fixed supported terminals-per-host number from CPU/RAM specifications alone. MT5 resource use varies with broker terminal build, symbol subscriptions, market activity, charts, history cache, polling cadence and reconnect behavior.

Before enabling a second active terminal slot, measure at minimum:
- idle and steady-state CPU per slot;
- working-set and peak memory per slot;
- disk I/O and terminal data-directory growth;
- network throughput and reconnect burst behavior;
- quote and account snapshot processing latency;
- daemon/database connection usage;
- recovery behavior when one terminal crashes or disconnects;
- impact of one slot's failure or restart on another slot.

Define a conservative slot limit from the measured worst-case envelope with headroom. If the limit is reached, provision another EdgeNest-owned Windows gateway and assign accounts centrally. Customers remain unaware of host placement and never supply their own VPS.

## 7. Broker latency and US region selection

The requested deployment region is USA. Contabo advertises US East (New York), US West (Seattle) and US Central (St. Louis) locations; the Cloud VPS 6 US listing has shown US Central. Region availability and price must be confirmed in the live configurator at purchase.

Do not choose a region based only on customer geography or provider marketing. For TenTrade:
1. obtain the exact DEMO server name from the TenTrade MT5 terminal/client portal or broker support;
2. provision only after the Windows product/region/price is confirmed;
3. measure TCP/TLS connection latency and MT5 connection stability from the actual host to the broker server;
4. record median, p95 and packet loss over a representative period;
5. compare another available US region only if the broker route is materially poor.

The initial product is telemetry, not latency-sensitive order execution. Do not claim execution-quality latency from a ping test.

## 8. Security and safety invariants

- EdgeNest owns the gateway infrastructure; no customer-operated VPS or terminal is part of the supported product workflow.
- Broker credentials are never committed, logged, included in evidence, placed in browser storage or copied into CI.
- Customer credential intake remains blocked until MT5-MVP-01C is reviewed and accepted.
- Each worker identity is unique to its slot; certificate identity authenticates a workload, not a customer or account owner.
- Every persisted frame must be bound to the currently verified broker/server/environment/account/terminal/worker assignment.
- Tenant authorization is resolved server-side from authenticated user, active organization membership, active entitlement and verified binding. Client-supplied tenant or account IDs are selectors only.
- Registry outage, identity mismatch, stale authorization, revoked binding, stale market data or failed reconciliation degrades/rejects the stream; never report it as healthy.
- One active publisher per external account; leases/fencing must reject duplicate or stale publishers.
- No automatic failover until the target lease/fencing and recovery acceptance is complete.
- Order submit/modify/cancel/close remain denied by code and configuration. The first multi-broker milestone is read-only.
- DEMO, PAPER and LIVE environments remain explicitly separated; the pilot must reject LIVE and unknown environments.

## 9. Implementation sequence

This ADR does not renumber canonical milestones in `docs/EDGENEST_MT5_MVP_IMPLEMENTATION_ROADMAP.md`.

| Sequence | Work | Gate |
|---|---|---|
| A | Confirm Contabo Windows Cloud VPS 6 USA checkout details; obtain TenTrade exact DEMO server name | No payment until region, Windows licence, renewal and total are verified |
| B | Complete MT5-MVP-00: current unchanged QOS-020 identity/cleanup guard and read-only data contract freeze | No database-capable tests until the exact guard passes in the authorized environment |
| C | Complete MT5-MVP-01A on one TenTrade DEMO terminal slot | Actual host-based 21-check acceptance, evidence and independent review |
| D | Implement registry-backed gateway/terminal-slot assignment and multi-principal admission | Contract, tenant, duplicate publisher, revocation and failure-isolation tests |
| E | Add a second broker DEMO terminal slot and run capacity/isolation tests | Demonstrate that slot B failure/restart cannot corrupt or impersonate slot A |
| F | Complete MT5-MVP-01B tenant-scoped read APIs and connection dashboard | Cross-tenant tests; show real accepted DEMO state only |
| G | Complete MT5-MVP-01C secure credential intake before self-service credential submission | Independent security review and secret lifecycle evidence |
| H | Scale to additional EdgeNest-owned gateways when measured capacity requires it | Gateway operations, leases/fencing, monitoring, recovery and incident runbooks |

MT5-MVP-02 remains the canonical gateway operations/scale-out milestone. This ADR prepares its multi-terminal target and identifies the dependencies; it does not declare multi-terminal operations implemented.

## 10. Acceptance tests for multi-terminal readiness

Before declaring a host multi-broker capable, tests must demonstrate:
1. two distinct broker/server DEMO identities are independently catalogued;
2. each slot starts only its assigned terminal executable and profile;
3. worker A cannot authenticate as slot B and vice versa;
4. a frame with a mismatched broker, server, environment, account, terminal or worker principal is rejected before persistence;
5. sequence checkpoints and leases are independent per account/slot;
6. duplicate and stale worker publishers are rejected;
7. revoking one slot does not revoke or expose another slot's data;
8. restarting or crashing one terminal/worker does not stop or corrupt another slot;
9. host reboot restores only approved desired-state slots and each slot reconciles before readiness;
10. resource limits prevent over-scheduling and produce an explicit degraded/unscheduled state;
11. organization A cannot read organization B's snapshots, even when IDs are guessed;
12. no execution command is accepted or emitted.

## 11. Explicit non-goals

- Customer-provided VPS, terminal, EA, desktop or worker.
- Automatic customer credential intake before 01C.
- Arbitrary "any broker" claims; support is limited to individually qualified connector/server combinations.
- Unlimited terminals per Windows host.
- Automatic multi-host failover before fencing and recovery acceptance.
- Live-account access or broker order execution.
- Treating CI, mocks, a successful RDP login, a terminal process, or a successful preflight as real broker conformance.

## 12. Purchase and provisioning evidence to retain

Record non-secret evidence only:
- provider order/instance ID and region;
- selected Windows Server product/licence and renewal terms;
- CPU/RAM/storage/network specification;
- host OS build, architecture and patch baseline;
- static IP/private networking and firewall policy;
- remote administration owner and break-glass procedure;
- host encryption and backup policy;
- terminal publisher/build and package hashes;
- broker/server/environment identifiers (mask account login in broadly accessible evidence);
- measured broker connectivity and host resource baseline;
- approved service identities, certificate fingerprints and secret references (never private key or password values).

A purchase receipt or preflight is not a DEMO acceptance result.
