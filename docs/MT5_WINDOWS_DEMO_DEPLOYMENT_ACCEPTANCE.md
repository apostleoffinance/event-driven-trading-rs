# MT5 Windows DEMO deployment and end-to-end acceptance

**Purpose:** close the real-host deployment and acceptance gap between the repository implementation and a verified, read-only MT5 DEMO → Quant OS PAPER data path.

**Roadmap naming:** the canonical docs/EDGENEST_MT5_MVP_IMPLEMENTATION_ROADMAP.md reserves MT5-MVP-01B for tenant-scoped read APIs and the connection dashboard. This document therefore defines the Windows deployment/acceptance work as the operational completion gate for 01A, without renumbering or replacing the canonical 01B product milestone.

**Current state: BLOCKED / NOT ACCEPTED.** No Windows host, MT5 terminal, broker DEMO account, approved secret manager, CA, isolated database or service has been accessed by this repository change. Do not infer acceptance from CI or a successful preflight.

## Release boundary

- One managed Windows host, one terminal/profile, one official Windows MetaTrader5 Python worker, one Rust bridge daemon, one verified DEMO account binding and one isolated database.
- Read-only account, symbol, tick, open-order and open-position telemetry into Quant OS PAPER.
- No live account, no customer credential intake, no multi-account multiplexing, no automatic failover, no history/bars, and no submit/modify/cancel/close operations.
- The official Windows provider is the deployment target. The macOS/EA path is not a substitute for Windows acceptance.

## What this change adds

The script at adapters/mt5-python/scripts/mt5-windows-deployment-preflight.ps1 is a read-only host preflight. It checks the approved Windows build, 64-bit architecture, pinned terminal/daemon/Python artifacts, official MetaTrader5 package version, service installation and configured service identities, optional running-state/single-terminal checks, required configuration presence, DPAPI secret-file presence and certificate/key file presence. It never prints environment values, reads broker credentials or private-key contents, starts/stops services, connects to MT5, or connects to PostgreSQL.

A PASS_PREFLIGHT_ONLY result means only that the checks in this script passed. It is not an acceptance result and must never be used to mark any of the 21 tests PASS.

## Mandatory decisions and external prerequisites

These must be approved and recorded before host provisioning; do not invent provider-specific settings:

1. **Host/provider:** named cloud/VPS provider, Windows Server edition/build, region, host owner, patch/reboot owner, backup/encryption policy, firewall and private network route.
2. **Workload identity:** dedicated non-admin worker and daemon service identities (which may be distinct), exact SIDs, logon rights, service configuration owner and terminal interactive-session model. The MT5 terminal must run in a supported interactive session; do not assume Session 0 works.
3. **Supervisor:** select and test a supervisor compatible with the terminal session. SCM may supervise the Rust daemon, but do not install the terminal/worker as arbitrary Windows services until session behavior is proven. No automatic logon without a separate security decision.
4. **Secrets:** approved secret-manager product/owner and workload authentication. The existing CurrentUser DPAPI file is a protected local cache only, not a managed secret manager. Approve delivery, ACLs, rotation, revocation, audit and incident response. Never set plaintext MT5_PASSWORD.
5. **PKI:** approved CA, server/client issuance, trust bundle, SAN/identity mapping, key storage, revocation, renewal, rotation/rollback owner and evidence. Existing PEM-based code requires file access; do not claim non-exportable key support.
6. **Broker:** one DEMO login and exact server delivered through an approved secure channel; independent operator verification of login/server/environment; verified binding and current evidence/authority; exact account currency and symbol allowlist.
7. **Database:** exact disposable QOS-020 resource, unchanged identity guard pass, approved migrations and cleanup proof. No staging or production DB. Do not run database-capable acceptance tests until this gate passes.
8. **Governance:** resolve required R5 approvals and QOS-020 recovery/verification; independent reviewer must not be the operator who ran the tests.

## Deployment sequence

### 0. Freeze and verify source

- Use a clean checkout at the approved main commit; record the 40-character commit SHA.
- Build worker and daemon from that exact commit in the approved build environment; pin SHA-256 hashes and versions.
- Run Rust format, Clippy, workspace/unit tests, Python adapter tests, evidence-validator tests and all repository CI required checks.
- Do not deploy a PR branch or unreviewed artifact. Do not claim hosted CI validates Windows/MT5 behavior.

### 1. Provision the Windows host

- Enroll the host in the approved provider/asset inventory.
- Apply supported Windows Server baseline, disk encryption, patch policy, clock synchronization, firewall allowlist and remote-access policy.
- Create dedicated non-admin identities and restricted directories for terminal profile/data, Python venv, worker state/sequence, daemon state, logs and certificates.
- Ensure the terminal can run in the approved interactive session under its intended identity.
- Install the exact terminal build, 64-bit Python, pinned official MetaTrader5 package, worker package and daemon binary.
- Configure service/supervisor recovery with bounded retries and a crash-loop circuit breaker. Require process-handle exit before replacement. Do not enable multi-host failover.

### 2. Provision credentials and certificates

- Retrieve the broker password from the approved secret manager using the worker workload identity. If the only available mechanism is the existing CurrentUser DPAPI file, treat this as a pilot-only local cache and obtain explicit security approval; it does not satisfy the managed-secret-manager gate.
- Issue unique client/server certificates under the approved CA. Bind the worker leaf fingerprint to the worker principal, assignment, account, terminal and verified broker binding.
- Verify certificate/key pairing, trust chain, hostname/SAN, validity, revocation and exact fingerprint. Run mt5-certificate-acl-review.ps1 under the authorized review process.
- Exercise certificate rotation and rollback on the isolated DEMO host; separately rotate the broker password and prove the old credential is rejected.
- Do not include private keys, passwords, connection strings, raw login or token values in evidence.

### 3. Registry and database admission

- Run the unchanged QOS-020 disposable-database identity guard and record its result plus cleanup verification.
- Apply only approved migrations to that disposable database.
- Provision organization membership, active entitlement, external account, verified binding/evidence, verifier authority, service-principal membership, worker assignment and READY session exactly as defined in MT5-MVP-01A_WINDOWS_DEMO_ACCEPTANCE_PLAN.md.
- Ensure the canonical broker/server registry matches the observed server. The configured Quant OS account ID is not the numeric MT5 login.
- Confirm revocation and mismatch fail closed before starting the real worker.

### 4. Run the read-only host preflight

Run in the approved operations context. Supply the values approved in the host manifest; do not copy secrets into command-line arguments.

    .\adapters\mt5-python\scripts\mt5-windows-deployment-preflight.ps1 `
      -WorkerServiceName 'QuantOS-MT5-Worker' `
      -DaemonServiceName 'QuantOS-MT5-Bridge' `
      -ExpectedWorkerIdentity 'DOMAIN\QuantOS-MT5-Worker' `
      -ExpectedDaemonIdentity 'DOMAIN\QuantOS-MT5-Bridge' `
      -TerminalPath 'C:\Program Files\MetaTrader 5\terminal64.exe' `
      -DaemonArtifactPath 'C:\QuantOS\bin\mt5-bridge-daemon.exe' `
      -PythonPath 'C:\QuantOS\venv\Scripts\python.exe' `
      -ExpectedPythonVersion '<approved-version>' `
      -ExpectedMetaTrader5Version '<approved-package-version>' `
      -ExpectedTerminalFileVersion '<approved-file-version>' `
      -ExpectedWindowsBuild '<approved-build>' `
      -RequireRunningServices

The service names, identities, paths and versions above are examples/placeholders, not selected production values. Replace them only with approved host inventory values. The script checks presence and metadata; it does not verify the database, broker identity, certificate chain/revocation or secret-manager integration.

### 5. Execute the 21 acceptance checks

Follow docs/MT5-MVP-01A_WINDOWS_DEMO_ACCEPTANCE_PLAN.md exactly. All 21 tests must be run on the same approved Windows host and isolated database. Retain timestamped, restricted, redacted evidence for each test, including evidence hash, source commit, artifact hashes, binding revision, sequence and persistence observations.

Required coverage:
- host, terminal and Python/package identity;
- terminal initialization and independent DEMO login/server verification;
- account, symbols/specification, fresh ticks, positions and pending orders;
- TLS 1.3/mTLS success and invalid-certificate rejection;
- binding mismatch and malformed protocol rejection;
- sequence ordering/duplicate handling;
- persistence/reconciliation;
- worker-only restart and terminal disconnect/reconnect;
- stale snapshot detection;
- log/credential redaction;
- execution remains disabled.

The evidence manifest collector intentionally initializes all 21 tests to NOT_RUN. Mark a test PASS only after its pass criterion is observed. Add the required evidence_sha256 for every test and use a restricted evidence reference. The validator must reject any missing, duplicate, non-PASS or incomplete test. A structural validator PASS does not authenticate the evidence.

### 6. Recovery and lifecycle acceptance

- Run the worker-only restart verifier; verify daemon remains running, sequence is monotonic, fresh post-restart market and reconciliation snapshots align, and the worker is restored.
- Exercise terminal disconnect/reconnect and prove identity is rechecked and freshness/reconciliation are re-established.
- Rotate broker password and certificates as separate events; verify old credentials/fingerprints cannot reconnect.
- Exercise revocation, entitlement suspension and binding/session mismatch; verify ingestion stops.
- Inspect service, terminal and worker logs for secret leakage. Verify alerting and crash-loop stop behavior.
- Confirm the account remains DEMO, Quant OS environment remains PAPER, and execution is disabled.

### 7. Independent sign-off

A reviewer other than the operator must inspect:
- exact deployed commit and worker/daemon/terminal hashes;
- all 21 evidence records and evidence artifact hashes;
- QOS-020 guard and cleanup evidence;
- identity/binding/entitlement/worker assignment state;
- service identities, host ACLs, certificate chain/revocation/rotation and secret custody;
- restart/reconnect/revocation outcomes;
- absence of execution capability.

Only after all gates pass may the operational 01A Windows DEMO slice be marked ACCEPTED. This does not authorize live accounts or order execution.

## Acceptance status ledger

| Gate | Current repository-known status | Required proof |
|---|---|---|
| Repository implementation | Implemented through 01A-1..4 | Exact-head green CI and reviewed merge |
| Windows host/provider selected | BLOCKED / not evidenced | Approved host inventory and provider decision |
| Service identities/supervisor | BLOCKED / not evidenced | Approved identity/SID and tested interactive-session supervisor |
| Managed secret manager | BLOCKED / not evidenced | Approved workload identity, secret access, rotation/revocation |
| CA/PKI lifecycle | BLOCKED / not evidenced | CA approval, ACL, chain/revocation and rotation evidence |
| QOS-020 isolated DB guard | BLOCKED / not evidenced here | Exact guard PASS and cleanup |
| Registry binding/admission | Code exists; live setup not evidenced | Verified DEMO binding and READY worker assignment/session |
| 21 Windows DEMO checks | NOT RUN / not evidenced | 21 PASS records with authenticated restricted evidence |
| Restart/reconnect/certificate rotation | Code and scripts exist; real-host result not evidenced | Actual Windows evidence |
| Independent review | NOT DONE / not evidenced | Signed reviewer decision |

## Final rule

A script PASS, GitHub CI PASS, source implementation, simulated test or complete-looking JSON file is not a real Windows DEMO acceptance. Keep the release state BLOCKED until the exact host, terminal, broker account, isolated database, credentials, certificates and independent evidence have all been verified. Live trading remains disabled.
