# QOS-015 macOS developer and conformance support

**Current status: portable macOS checks are supported by this change; real macOS MT5 terminal conformance remains BLOCKED until a supported terminal provider is implemented and independently verified.**

This document separates operating-system support from terminal API support. The Rust bridge and protocol checks can run natively on macOS. The existing MetaTraderTerminal implementation in adapters/mt5-python/mt5_adapter/worker.py uses the official MetaTrader5 Python package and intentionally rejects non-Windows platforms. Installing the MetaTrader 5 macOS application alone does not make that Windows Python API available to native macOS Python.

## Supported on macOS in this iteration

- Run the portable Python adapter contract tests.
- Run Rust format, workspace check, test, and Clippy checks.
- Inspect certificate/key pairs, the pinned client-certificate SHA-256 fingerprint, certificate validity, CA-chain verification, optional CRL verification, and local key-file/parent-directory permissions.
- Produce local diagnostic output without promoting it to Windows or release evidence.

## Not yet a real-terminal PASS

The macOS preflight reports the real terminal integration as NOT_RUN unless a provider is configured, and still blocks release approval when --require-real-terminal is used. A provider variable is not itself proof that a provider works. A real implementation must establish all of the following against the designated demo account:

1. Authenticated terminal login matches the configured account and exact server.
2. Account currency, exact broker symbols, bid/ask, timestamps, and instrument specifications come from the actual terminal.
3. The adapter emits the same versioned heartbeat, market-data, and reconciliation contracts as the Windows adapter.
4. The Rust bridge accepts the expected mTLS identity and persists the snapshots to PostgreSQL.
5. A worker-only restart advances the persisted sequence and results in fresh post-restart market-data and reconciliation snapshots while the daemon remains running.
6. Certificate trust, fingerprint pinning, revocation enforcement, key permissions, rotation, rollback, and rejection of a retired/untrusted identity are separately verified.
7. Evidence is redacted, tied to the exact repository commit, and independently reviewed.

Do not use fabricated or simulated data to satisfy these requirements.

## Local preflight

From the repository root on macOS, run:

    bash adapters/mt5-python/scripts/mt5-macos-preflight.sh

To run only Python adapter contract tests and skip Rust checks:

    bash adapters/mt5-python/scripts/mt5-macos-preflight.sh --skip-rust

To explicitly require real terminal support and fail closed while no verified provider is available:

    bash adapters/mt5-python/scripts/mt5-macos-preflight.sh --require-real-terminal

The preflight output is a local developer report, not a QOS-015 release manifest. Rust tests may need a reachable PostgreSQL test database, depending on the repository test configuration.

## Certificate review

Use the helper only with the protected demo/staging certificate files and a trusted CA bundle. The expected client fingerprint must be obtained from the approved deployment configuration; never copy private-key bytes or secrets into the command, repository, or evidence report.

    python3 adapters/mt5-python/scripts/mt5-macos-certificate-review.py --client-cert /protected/path/client-cert.pem --client-key /protected/path/client-key.pem --server-cert /protected/path/server-cert.pem --server-key /protected/path/server-key.pem --ca-file /protected/path/bridge-ca.pem --expected-client-sha256 YOUR_APPROVED_64_HEX_FINGERPRINT --crl-file /protected/path/current-ca.crl --evidence-out /protected/path/macos-certificate-review.json

Omit --crl-file only when you explicitly accept that the report will be blocked for missing revocation evidence. A CRL check is a point-in-time file-level check; it does not prove the running TLS stack enforces revocation. The helper never reports the overall release gate as PASS. Runtime negative tests and rotation/rollback evidence remain required.

## macOS services and restart recovery

macOS uses launchd, not Windows Services. Service supervision can be implemented with per-user LaunchAgents or system LaunchDaemons, but the service definitions must not contain broker passwords, private-key contents, or database credentials in plaintext. Use a protected configuration mechanism and ensure the service identity can read only the required key files.

A restart test must stop/restart only the worker and must not reset or delete the durable sequence checkpoint or database snapshots. It must verify the daemon remained running, the market-data sequence advanced, and fresh market-data plus reconciliation snapshots were persisted after the worker stopped. The provider-backed smoke and LaunchAgent restart scripts now exist, but no native macOS MT5 provider is bundled. Until a trusted provider is implemented and verified against a real terminal, these commands cannot establish a real-terminal PASS.

## Release policy

- Local macOS tests can prove portable code contracts only.
- macOS native certificate/service checks cannot substitute for Windows ACL/service evidence when the target deployment is Windows.
- Windows deployment remains subject to docs/MT5_DEMO_SMOKE_TEST.md and the consolidated QOS-015 release manifest.
- Keep venue_execution_enabled explicitly false. No order submission or cancellation work is authorized by this developer tooling.
