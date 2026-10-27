# MT5 Demo Conformance Evidence Record

> Store the completed record and raw logs in the approved protected operations location. Do not commit secrets, private keys, passwords, full database URLs, or unredacted broker diagnostics.

## Run metadata
- Date/time (UTC):
- Operator / reviewer:
- Repository commit:
- Windows host identifier (non-sensitive):
- Windows / PowerShell / Python versions:
- MT5 terminal build:
- Worker service/process name:
- Daemon service/process name:
- Demo broker server:
- Masked demo login:
- Expected account currency:
- Configured exact symbols:
- Protected evidence location:

## First smoke pass
- Result: PASS / FAIL
- Start/end UTC:
- Script timeout/poll settings:
- Market-data sequence values observed:
- Market-data snapshot timestamp:
- Reconciliation sequence/timestamp:
- Quote and instrument metadata checks:
- Redacted output/log reference:
- Failure IDs and remediation:

## Worker restart and recovery
- Worker stopped at (UTC):
- Daemon remained running: YES / NO
- Daemon connection-close evidence:
- Worker restarted at (UTC):
- Reconnect/authentication evidence:
- Last pre-stop market-data sequence:
- First post-restart market-data sequence:
- Sequence advanced: PASS / FAIL
- Fresh post-restart market data persisted: PASS / FAIL
- Fresh post-restart reconciliation persisted: PASS / FAIL
- Second smoke pass result: PASS / FAIL
- Redacted output/log reference:
- Failure IDs and remediation:

## Certificate lifecycle
- Client certificate subject / issuer:
- Client certificate validity window:
- Client leaf DER SHA-256 fingerprint:
- Configured pin matches: PASS / FAIL
- Server certificate subject / issuer:
- Server certificate validity window:
- Server certificate fingerprint:
- Certificate/key pair loading: PASS / FAIL
- CA trust-chain reviewed by / date:
- Revocation method and enforcement evidence:
- Rotation owner / due date:
- Rotation overlap test evidence:
- Rollback test evidence:
- Old/untrusted identity rejection test evidence:
- Client private-key ACL review: PASS / FAIL
- Server private-key ACL review: PASS / FAIL
- Parent-directory effective-access review: PASS / FAIL
- Unreviewed ACL identities / remediation:

## Final decision
- All acceptance checks PASS: YES / NO
- Unresolved failures or UNKNOWN checks:
- Operator recommendation: PASS / BLOCKED
- Independent reviewer / date:
- Explicit statement: **No venue execution was enabled as part of this verification.**
