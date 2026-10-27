# QOS-015A — Windows MT5 Demo Conformance Runbook

**Status:** real-host evidence pending. CI and static tests do not establish conformance of the Windows VPS.

## Safety boundary

- Use only the designated MT5 demo account.
- Keep worker and daemon read-only. Do not add order submission/cancel calls or weaken TLS validation.
- Never clear the sequence checkpoint or database snapshots to force a pass.
- Never capture/publish broker passwords, private-key material, complete database URLs, or unredacted terminal diagnostics.
- Venue execution remains disabled.

## 1. Capture baseline

On the Windows VPS, record UTC start time, host identifier, Windows/PowerShell/Python versions, repository commit, terminal build, daemon and worker service names, and the demo account's masked login plus exact server/currency. Store sensitive operational details only in the protected operations record.

Confirm the terminal is connected to the expected demo server and the daemon and worker use the intended configuration. Do not echo environment dumps.

## 2. First smoke pass

In a PowerShell session on the same host, with the smoke-test environment values configured but not echoed:

```powershell
$PSVersionTable.PSVersion
python --version
.\adapters\mt5-python\scripts\mt5-demo-smoke.ps1 -TimeoutSeconds 240 -PollSeconds 10
```

Save timestamped console output to the protected evidence location. The script must verify account/server/currency, exact symbols, fresh quote/specification data, client certificate fingerprint and certificate/key loading, daemon listener, migrated database tables, persisted market-data/reconciliation snapshots, and two distinct fresh market-data sequences.

If it fails, preserve the redacted error and timestamped daemon/worker logs, diagnose, fix, and rerun. Partial output is not a pass.

## 3. Automated worker restart and recovery

The restart verifier runs the initial smoke pass itself, then performs the worker-only restart and second pass. Run in an authorized PowerShell session with the same required environment values as the smoke harness. Substitute the actual Windows service names; do not guess them.

```powershell
.\adapters\mt5-python\scripts\mt5-demo-restart-verify.ps1 -WorkerServiceName 'QuantOS-MT5-Worker' -DaemonServiceName 'QuantOS-MT5-Bridge' -TimeoutSeconds 240 -PollSeconds 5 -EvidencePath 'C:\QuantOS\evidence\mt5-restart-evidence.json'
```

The script requires both services to be running before it starts. It runs the first smoke pass, records the latest persisted market-data sequence, stops and restarts only the worker, confirms the daemon remains running, waits for market-data sequence advancement and fresh post-stop reconciliation, runs the smoke harness again, and writes a JSON evidence record in `finally`, including failure details when possible. It never clears database rows or the worker sequence checkpoint.

Review the evidence record and logs. The record is not a replacement for daemon authentication logs or independent review. If it fails, diagnose and rerun after fixing the cause; a failed/partial record is not a pass.

For manual verification instead, Keep the daemon running, stop only the worker with its normal service manager, confirm the daemon records connection closure, restart the worker with the same `MT5_SEQUENCE_FILE`, rerun the smoke test, and compare exact pre-stop and post-restart sequence values. Do not reset the sequence file or delete database rows.

## 4. Certificate lifecycle and private-key ACL review

Run from an authorized PowerShell session, substituting the exact Windows service identity running the worker/daemon. Do not use a broad group as the expected service identity.

```powershell
.\adapters\mt5-python\scripts\mt5-certificate-acl-review.ps1 -ExpectedServiceIdentity 'DOMAIN\QuantOS-MT5'
```

The script verifies configured files exist, loads both certificate/key pairs without exporting or printing key material, prints certificate subject/issuer/validity/fingerprint, confirms the client leaf fingerprint matches MT5_BRIDGE_PRINCIPAL_SHA256, reports key ACL entries, fails on broad-principal read grants, and warns near expiry.

Review ACL output including inherited ACEs and warnings for other identities against actual service-account policy. Script success is necessary but not sufficient; perform a manual effective-access review including parent directories.

Record and test all of the following in the protected operations record:

- [ ] Issuing CA and trust-chain owner for client/server certificates.
- [ ] Revocation method and evidence the deployed daemon/worker enforce it. The script does not query OCSP/CRL.
- [ ] Rotation owner, expiry alert owner, and rotation schedule.
- [ ] New certificate fingerprint pin-update procedure and safe overlap of old/new certificates.
- [ ] Tested roll-forward and rollback, including negative test that an old/untrusted identity is rejected when expected.
- [ ] Private-key ACLs on both key files and parent directories; only service identity and approved administrators should have necessary access.
- [ ] No private key or secret copied into source, CI artifacts, logs, or evidence attachments.

Do not test rejection by disabling certificate verification or weakening TLS.

## 5. Evidence record

Copy `docs/MT5_DEMO_CONFORMANCE_EVIDENCE_TEMPLATE.md` into the protected release record. Complete every field and attach redacted console output and timestamped logs. Link the protected evidence record from the PR/release review without committing credentials or sensitive operational details.

## Acceptance gate

All items must be PASS before any venue-execution task:

- [ ] First smoke pass succeeded on the actual Windows VPS.
- [ ] Correct demo identity, server, currency, exact symbols and fresh quotes/specifications verified.
- [ ] Fresh market-data and reconciliation snapshots persisted for the same account/terminal.
- [ ] At least two distinct market-data sequences observed.
- [ ] Worker-only restart succeeded with daemon left running.
- [ ] Post-restart market-data sequence advanced beyond pre-stop sequence.
- [ ] Fresh post-restart reconciliation persisted.
- [ ] Client/server certificate expiry, fingerprint pin, CA trust and revocation policy reviewed.
- [ ] Private-key ACL and parent-directory effective access reviewed.
- [ ] Rotation and rollback procedure tested and recorded.
- [ ] Redacted evidence attached and independently reviewed.

Any FAIL, UNKNOWN, or unrecorded check means the gate is BLOCKED. Do not proceed to execution. Do not enable venue execution.
