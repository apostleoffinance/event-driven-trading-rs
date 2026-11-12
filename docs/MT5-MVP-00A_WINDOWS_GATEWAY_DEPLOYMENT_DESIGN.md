# MT5-MVP-00A ADR-05 — Managed Windows Gateway Deployment

**Status:** Provider-neutral target design proposed. Windows host/provider is not selected. No host or service was provisioned.

## Options and target

| Target | Advantages | Risks / controls | Recommendation |
|---|---|---|---|
| Dedicated Windows VPS/cloud VM | Clear ownership, narrow network, controlled patch/backup/monitoring, one terminal and service identity | Provider-specific workload identity, console/session, disk encryption, network and secret-manager integration must be validated | Preferred target class for 01A |
| User-operated Windows machine | Fast pilot, access to existing desktop | Shared user session, unplanned shutdown/reboot, broader software/users, weak separation and uncertain evidence | Not production target; only explicitly supervised development/conformance |
| Managed Windows server in cloud | Central enterprise controls, possible integrated identity/key manager, support contract | Product/provider may have session/GUI restrictions and cost constraints | Acceptable equivalent to dedicated VM only after terminal/UI and workload-identity qualification |

Use one dedicated managed Windows host, one MT5 terminal installation/profile, one official Python provider worker, one Rust daemon, one verified DEMO/PAPER binding. No automatic failover, multi-account multiplexing, or duplicate worker. The actual cloud/provider remains undecided; workload identity, secret manager, PKI, Windows edition and interactive-session supervision depend on that choice.

## Host isolation and installation

- Record and pin supported Windows edition/build, architecture, terminal version/build/publisher, install/data-folder identity, Python architecture/version, official `MetaTrader5` package version, worker artifact hash, daemon artifact hash and protocol/admission versions.
- Dedicated non-admin service identity/SID; deny interactive logon except a separately approved break-glass operator. Separate operator login from worker service identity. Restrict RDP and audit every session.
- Install terminal under a protected program directory and use a dedicated data/profile directory with ACLs limited to the service identity and approved administrators. Do not reuse a personal terminal profile.
- Python virtual environment and pinned dependencies under read-only install ACL for service identity; separate writable state/log/sequence directories. Prevent unreviewed package upgrades.
- Rust daemon installed as a versioned immutable artifact; service account has only database network access, listener access, certificate read, log/state write and required secret-manager retrieval.
- Encrypt disks/backups; exclude broker credentials/private keys from ordinary image snapshots. Host backup does not automatically authorize restoring a duplicated terminal/worker identity.
- No secrets in command line, committed files, general `.env`, service logs, Windows event text, crash dumps or ordinary environment dumps.

## Process supervision and lifecycle

Use a single host supervisor/controller that can manage a terminal in its required user session and its worker, plus Windows Service Control Manager (SCM) for the Rust daemon where compatible. Do not assume the MT5 terminal works in Windows Session 0. Verify the terminal's headless/session requirements with the selected terminal build before choosing a specific supervisor. Do not enable automatic logon merely to keep a desktop session alive without a separate security decision.

| Stage / failure | Supervisor action and readiness rule |
|---|---|
| Host startup | Validate supported build, disk encryption, service identity ACLs, clock sync, network/firewall, artifact hashes and single-instance lock. Alert and remain stopped on drift. |
| Secret/certificate availability | Retrieve short-lived broker credential through approved workload identity; validate cert/key presence, expiry, trust and assignment. Fail closed if service unavailable, expired or revoked. |
| Terminal startup | Start the designated terminal under dedicated profile/session; wait bounded time for terminal process, connection state and exact login/server. A process alone is not ready. Do not persist if identity mismatches. |
| Worker startup | Start exactly one worker under dedicated SID, with a stream-scoped sequence checkpoint and pinned version. Acquire OS mutex and DB-backed account/terminal lease. Reject second instance. |
| Bridge connection | Daemon SCM service starts listener only after config/DB setup. Worker negotiates TLS 1.3 mTLS and registry-backed admission before sending v2 frames. |
| Initial reconciliation | Persist complete account/open-order/position snapshot; verify identity, sequence, current binding revision and freshness. Set readiness only after reconciled snapshot and fresh market feed. |
| Running health | Report separate process, terminal-connected, TLS-authenticated, admission-current, market-fresh, reconciliation-ready, persistence and execution-disabled states. Heartbeat or TCP alone is never ready. |
| Worker crash | Supervisor captures exit category without dumping secrets, closes/ages stream, exponential backoff 1–60 sec with jitter; after 10 consecutive failures in 15 min, stop automatic restart and alert operator. Restart only after prior process handle is gone; then re-authenticate and reconcile. Thresholds are proposed defaults requiring ops approval. |
| Terminal disconnect | Worker stops claiming fresh snapshots, uses bounded reconnect and rechecks login/server. Quotes age naturally to stale. No connected/fresh state until new ticks and full reconciliation. |
| Daemon restart | SCM restarts under bounded policy; worker reconnects with same assigned identity/sequence checkpoint. Daemon rechecks certificate, registry, binding and entitlement; requires full reconciliation before readiness. |
| Binding/entitlement/certificate revocation | Registry update invalidates lease; stream is closed within 5-second proposed revalidation objective. No continued persistence. |
| Credential rotation | Update broker-side password and secret manager; controlled worker restart; verify login and reconciliation; revoke old password. Does not rotate certificates. |
| Certificate rotation | Issue/register new workload certificate while old remains briefly valid; update trust/assignment; reconnect and verify new fingerprint; revoke/remove old; close old stream. Does not change broker ownership or password. |
| Maintenance/shutdown | Stop admitting new stream, stop worker gracefully, mark stream degraded, terminate daemon after connections close, then terminal, then host. On interruption, next start performs complete admission/reconciliation. |

Proposed failure policy: never restart in a tight loop; alert on circuit-breaker, keep data stale, do not auto-failover to another host, and require operator recovery after repeated identity/authentication failures. Time bounds and windows above are design defaults, not measured production values.

## Network and workload boundaries

- Worker-to-daemon loopback (`127.0.0.1:9443`) on a colocated host where practical. If listener is remote, private network + host firewall + TLS 1.3 mTLS; no public listener. PostgreSQL access from daemon only via private network and restricted runtime credentials.
- Registry is authoritative. Daemon admission adapter must fail closed on registry outage and periodically revalidate revisions; no static account allowlist as sole authority.
- CA trust, workload certificate, broker credential, server private key and application settings are separate objects and ACLs. Windows service identity may retrieve only its own DEMO password and key.
- Expose operational status to a protected monitoring plane without login, prices, positions, password, private key, token, raw connection string or raw broker login. Use opaque IDs and normalized error categories.

## Outstanding host decisions and acceptance

Before implementation, decide host/provider, Windows edition/build, workload identity, secret manager, PKI provider, terminal interactive/session behavior, supervisor product, patch owner, RDP policy, firewall/network path, disk/key protection, backup exclusion, monitoring owner, incident response and approved retry/revalidation objectives.

Acceptance requires one host/terminal/worker/daemon instance, version/hash pinning, no duplicate process, verified operator-approved binding, secret and certificate lifecycle drills, explicit per-state health, stale-data correctness, restart and reconnect reconciliation, and continued deny-all execution behavior. No automatic failover is in scope.

