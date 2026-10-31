# QOS-015 macOS developer and conformance support

**Current status: a native macOS MQL5 socket provider and terminal-side read-only EA are implemented and compiled; real macOS MT5 terminal conformance remains BLOCKED pending EA reload, fresh demo data, persistence/restart checks, and the remaining release gates.**

This document separates operating-system support from terminal API support. The Rust bridge and protocol checks can run natively on macOS. The existing MetaTraderTerminal implementation in adapters/mt5-python/mt5_adapter/worker.py uses the official MetaTrader5 Python package and intentionally rejects non-Windows platforms. Installing the MetaTrader 5 macOS application alone does not make that Windows Python API available to native macOS Python.

## Supported on macOS in this iteration

- Run the portable Python adapter contract tests.
- Run Rust format, workspace check, test, and Clippy checks.
- Inspect certificate/key pairs, the pinned client-certificate SHA-256 fingerprint, certificate validity, CA-chain verification, optional CRL verification, and local key-file/parent-directory permissions.
- Produce local diagnostic output without promoting it to Windows or release evidence.

## Not yet a real-terminal PASS

The macOS preflight reports the real terminal integration as NOT_RUN unless a provider is configured, and still blocks release approval when --require-real-terminal is used. A provider variable or passing unit tests are not proof that the EA works. The native provider uses `adapters/mt5-python/mql5/QuantOSReadOnlyProvider.mq5` plus `mt5_adapter.macos_socket_provider`; follow `adapters/mt5-python/README.md` for terminal setup. A real implementation must establish all of the following against the designated demo account:

1. Authenticated terminal login matches the configured account and exact server.
2. Account currency, exact broker symbols, bid/ask, timestamps, and instrument specifications come from the actual terminal.
3. The adapter emits the same versioned heartbeat, market-data, and reconciliation contracts as the Windows adapter.
4. The Rust bridge accepts the expected mTLS identity and persists the snapshots to PostgreSQL.
5. A worker-only restart advances the persisted sequence and results in fresh post-restart market-data and reconciliation snapshots while the daemon remains running.
6. Certificate trust, fingerprint pinning, revocation enforcement, key permissions, rotation, rollback, and rejection of a retired/untrusted identity are separately verified.
7. Evidence is redacted, tied to the exact repository commit, and independently reviewed.

Do not use fabricated or simulated data to satisfy these requirements.

## MT5 quote clock and freshness

`TimeCurrent()` is the last quote time and can remain frozen when no new quote arrives. The native EA therefore sends `TimeTradeServer()` as its current server-time reference. The adapter still compares the quote timestamp against that advancing reference and fails closed on a stale/mismatched quote; do not loosen these checks to make weekend or otherwise stale quotes pass.

The 2026-10-31 Mac diagnostic showed a frozen quote clock and an advancing `TimeTradeServer()`, while host NTP offset was approximately 0.45 seconds. The EA was compiled with zero errors/warnings and its source/binary were installed in the active terminal's Experts folder after backing up the previous files. Replacing files does not establish that an already attached EA reloaded the binary: detach and re-attach the read-only EA, then verify the worker rejects the known stale quote as stale/mismatched and accepts only genuinely fresh quote data. Until that is demonstrated and persisted market-data plus reconciliation snapshots are verified, the runtime stream is not healthy and QOS-015 remains blocked.

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

A restart test must stop/restart only the worker and must not reset or delete the durable sequence checkpoint or database snapshots. It must verify the daemon remained running, the market-data sequence advanced, and fresh market-data plus reconciliation snapshots were persisted after the worker stopped. The native provider, read-only MQL5 EA, smoke tooling, and LaunchAgent restart verifier are present. Until the EA is reloaded and verified against fresh data from a real terminal, these commands cannot establish a real-terminal PASS.

## Capturing macOS EA/provider reconnect diagnostics

When the EA reconnects or reports a socket read failure, capture Toolbox output together with timestamped worker and daemon logs for the same reproduction window. Use separate Terminal windows, create a protected log directory, and do not enable shell tracing or print environment variables:

    umask 077
    mkdir -p "$HOME/mt5-diagnostics"
    cd /path/to/event-driven-trading-rs
    RUST_LOG=info cargo run -p intent-bridge --bin mt5-bridge-daemon 2>&1 | while IFS= read -r line; do printf '%s %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$line"; done | tee "$HOME/mt5-diagnostics/daemon.log"

In another Terminal window, from the adapter directory and with the protected runtime configuration already loaded:

    umask 077
    cd /path/to/event-driven-trading-rs/adapters/mt5-python
    python3 -u -m mt5_adapter.worker 2>&1 | while IFS= read -r line; do printf '%s %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$line"; done | tee "$HOME/mt5-diagnostics/worker.log"

Start the daemon first, then the worker, attach/keep the read-only EA on the demo chart, reproduce the reconnect, and stop both processes after a short shared interval. Save the Toolbox Expert output for that same interval. The current-checkout worker emits its own UTC timestamp plus a fixed stage (`provider_connect`, `tls_context`, `bridge_tcp_connect`, `bridge_tls_handshake`, or `snapshot_poll`); OS/TLS error text is intentionally omitted, while a numeric errno may be included. A supervised worker from a separate older checkout may not include these stage labels. Redact account/login identifiers, broker/server and host details, local paths, certificate fingerprints, tokens, and other infrastructure identifiers before sharing any output.

The supplied Toolbox excerpt proves the EA sent its handshake and received PING commands, but it does not log the PING responses. In the provider source, the worker accepts the `HELLO` token before issuing its first PING, and its request/response calls are synchronous. Repeated PINGs strongly suggest the preceding PING/PONG exchanges completed, but the EA log does not independently show the responses. The worker's next stages are TLS-context creation and the Rust daemon TCP/TLS connection; snapshot commands follow only after that succeeds. Thus PING-only output points to a failure before snapshot polling, but does not identify which subsequent stage failed.

The EA reports `SocketRead` returning `-1`, error `5273`, and `SocketIsConnected=false`; its current `DrainSocket` logic then closes its local socket. This establishes that the peer was observed disconnected at that check, but neither the error code nor the screenshot establishes which process initiated the original close. The worker's reconnect path calls the macOS provider's `shutdown()` before accepting the next EA connection, so a worker retry can subsequently close the old session. Correlate timestamps from all three sources before attributing the initial disconnect or changing socket behavior.

## Release policy

- Local macOS tests can prove portable code contracts only.
- macOS native certificate/service checks cannot substitute for Windows ACL/service evidence when the target deployment is Windows.
- Windows deployment remains subject to docs/MT5_DEMO_SMOKE_TEST.md and the consolidated QOS-015 release manifest.
- Keep venue_execution_enabled explicitly false. No order submission or cancellation work is authorized by this developer tooling.
