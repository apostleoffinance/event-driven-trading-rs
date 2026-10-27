# QOS-015A — Windows MT5 Demo Conformance Runbook

**Status:** real-host evidence pending. CI and static tests do not establish conformance of the Windows VPS.

## Safety boundary

- Use only the designated MT5 demo account.
- Keep worker and daemon read-only. Do not add order submission/cancel calls or weaken TLS validation.
- Never clear the sequence checkpoint or database snapshots to force a pass.
- Never capture/publish broker passwords, private-key material, complete database URLs, or unredacted terminal diagnostics.
- Venue execution remains disabled.

## 1. Capture baseline

On the Windows VPS, record UTC start time, host identifier, Windows/PowerShell/Python versions, repository commit, terminal build, daemon and worker process/service names, and the demo account's masked login plus exact server/currency. Store sensitive operational details only in the protected operations record.

Confirm the terminal is connected to the expected demo server and the daemon and worker use the intended configuration. Do not echo environment dumps.

## 2. First smoke pass

In a separate PowerShell session on the same host, with smoke-test environment values configured but not echoed:

\`\`\`powershell
$PSVersionTable.PSVersion
python --version
.\adapters\mt5-python\scripts\mt5-demo-smoke.ps1 -TimeoutSeconds 240 -PollSeconds 10
\`\`\`

Save timestamped console output to the protected evidence location. The script must verify account/server/currency, exact symbols, fresh quote/specification data, client certificate fingerprint and certificate/key loading, daemon listener, migrated database tables, persisted market-data/reconciliation snapshots, and two distinct fresh market-data sequences.

If it fails, preserve the redacted error and timestamped daemon/worker logs, diagnose, fix, and rerun. Partial output is not a pass.

## 3. Prove reconnect and reconciliation after worker restart

Do this only after the first smoke pass. Keep the daemon running throughout.

1. Record UTC time and current worker PID/service status.
2. Stop the worker only using its normal service manager/documented stop command. Do not stop the daemon or terminal and do not remove the sequence checkpoint.
3. Verify the worker is stopped. Confirm daemon logs record connection closure/discard without secrets.
4. Start the worker using the same configuration and same MT5_SEQUENCE_FILE.
5. Observe the worker reconnect. Do not reset the sequence file or delete database rows.
6. Rerun the smoke harness. It must pass with fresh market-data and reconciliation snapshots and two distinct market-data sequences.
7. Confirm from protected logs/database evidence that the post-restart market-data sequence is greater than the pre-stop sequence and reconciliation was persisted after reconnect. Record exact values and UTC timestamps.
8. If reconnect fails, sequence regresses, reconciliation does not refresh, or snapshots remain stale, mark FAIL/BLOCKED. Do not proceed to execution.

The smoke harness proves repeated sequences advance during its observation window. The operator must explicitly compare before-stop and after-restart values to prove restart continuity.

## 4. Certificate lifecycle and private-key ACL review

Run from an authorized PowerShell session, substituting the exact Windows service identity running the worker/daemon. Do not use a broad group as the expected service identity.

\`\`\`powershell
.\adapters\mt5-python\scripts\mt5-certificate-acl-review.ps1 -ExpectedServiceIdentity 'DOMAIN\QuantOS-MT5'
\`\`\`

The script verifies configured files exist, loads both certificate/key pairs without exporting or printing key material, prints certificate subject/issuer/validity/fingerprint, confirms the client leaf fingerprint matches MT5_BRIDGE_PRINCIPAL_SHA256, reports key ACL entries, fails on broad-principal read grants, and warns near expiry.

Review ACL output including inherited ACEs and warnings for other identities against actual service-account policy. Script success is necessary but not sufficient; perform a manual effective-access review.

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

Copy docs/MT5_DEMO_CONFORMANCE_EVIDENCE_TEMPLATE.md into the protected release record. Complete every field and attach redacted console output and timestamped logs. Link the protected evidence record from the PR/release review without committing credentials or sensitive operational details.

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

Any FAIL, UNKNOWN, or unrecorded check means the gate is BLOCKED. Do not enable venue execution.
