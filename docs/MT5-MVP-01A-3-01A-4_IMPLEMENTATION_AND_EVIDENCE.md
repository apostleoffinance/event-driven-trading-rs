# MT5-MVP-01A-3/4 — Implementation and evidence guide

**Scope:** repository-side DEMO vertical-slice and recovery/conformance implementation.  
**Runtime status:** NOT ACCEPTED. This guide does not assert that a Windows host, MT5 terminal, broker DEMO account, or isolated PostgreSQL instance has been exercised.

## 01A-3 — Managed DEMO data vertical slice

The implementation uses the existing strict MT5 Bridge v2 read-only contract:

- Official Windows `MetaTrader5` Python provider; no parallel provider for the same account/terminal stream.
- TLS 1.3 mutual authentication and an exact worker certificate fingerprint allowlist.
- Registry-backed admission, verified binding/entitlement checks, worker assignment/session readiness, and account/terminal publisher lease.
- Account, symbol/quote, open-order and position snapshots; decimal strings and source/capture timestamps remain distinct.
- Durable sequence checkpointing and PostgreSQL market/reconciliation persistence.
- DEMO-only provider guard, expected account currency and exact symbol allowlist.
- Execution capabilities remain false; no submit/modify/cancel/close operation is introduced.

Do not extend schema v2 with unapproved fields. The contract document remains authoritative for unsupported provider fields and timestamp semantics.

## 01A-4 — Recovery and conformance

The Windows worker restart verifier now requires all of the following before reporting a pass:

1. Both worker and daemon are already running; the script does not restart the daemon.
2. The baseline market and reconciliation snapshots have the same sequence.
3. Windows Service Control Manager confirms the worker is stopped before the stop timestamp is recorded.
4. The worker restart timestamp is recorded only after SCM confirms Running.
5. Post-restart market and reconciliation snapshots are both captured after the stop event.
6. The market sequence advances beyond baseline and exactly matches the reconciliation sequence.
7. The worker passes the smoke check again.
8. The worker is restored to Running in the `finally` path, and restoration failure is recorded.

SQL account/terminal scope values are passed through psql variables for literal quoting; they are not interpolated into SQL text.

The repository's Rust/adapter test suites cover protocol validation, mTLS trust/fingerprint rejection, identity mismatch, freshness degradation/recovery, heartbeat expiry, sequence behavior, and deny-all execution. Portable tests are not substitutes for Windows service, terminal, certificate, broker or database evidence.

## Redacted acceptance evidence

The Windows acceptance plan defines the 21 required checks. After the target host and isolated database are separately provisioned and the acceptance plan has been executed, produce a JSON evidence record with:

- `evidence_schema_version: 1`, `milestone: MT5-MVP-01A`, and `protocol_schema_version: 2`.
- Broker environment `DEMO`, Quant environment `PAPER`, and `execution_enabled: false`.
- Immutable source commit and SHA-256 hashes for worker, daemon, terminal and worker public certificate.
- Worker/daemon versions, terminal build, Python architecture/version, official MetaTrader5 package version and config revision.
- Registry references and binding revision; store only a keyed HMAC-SHA-256 reference for the observed broker login, never the raw login or the HMAC key.
- Windows edition/build, architecture, service identity SID and clean source-tree status.
- Exactly one PASS evidence entry for each acceptance test ID 1–21, with UTC observation timestamp and a restricted evidence reference.

Do not include passwords, private keys, connection strings, tokens, raw broker login, environment dumps or unredacted exceptions. The validator rejects common secret-bearing field names.

Run the structural validator:

```powershell
python adapters/mt5-python/scripts/validate_demo_acceptance_evidence.py .\mt5-demo-acceptance-evidence.json
python adapters/mt5-python/scripts/validate_restart_evidence.py .\mt5-restart-evidence.json
```

A validator PASS means only that the submitted record is structurally complete and internally consistent. It does not authenticate evidence, prove broker ownership, prove independent review, or authorize deployment/execution.

## Required gates before real acceptance

- QOS-020 exact disposable database identity guard must pass unchanged; verify cleanup. Do not run database-capable acceptance tests against staging or production.
- Named Windows provider/host, supported OS/build, dedicated non-admin service identity and terminal session/supervisor model must be approved.
- Dedicated broker DEMO account and exact server must be delivered through the approved secure channel.
- Secret manager, workload identity, CA/PKI owner, private-key storage/ACL, rotation and emergency revocation must be approved and rehearsed.
- DEMO-to-PAPER policy and a verified broker binding with independent verifier evidence must exist.
- Run all 21 acceptance checks on the actual managed Windows DEMO host and isolated DB, retain redacted evidence, pin artifacts and obtain independent review.

Until those gates pass, 01A-3/4 remain repository-implemented but operationally BLOCKED. Live accounts and venue execution remain prohibited.
