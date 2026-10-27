# MT5 Bridge Transport Security Contract — QOS-014

Status: **mTLS transport boundary implemented; integration remains incomplete**. The Rust bridge now has a TLS 1.3 mutual-authentication/configuration boundary and certificate-fingerprint-to-principal allow-list. No MT5 Expert Advisor, broker mapping, order-dispatch wiring, or live execution is implemented.

## Goal and trust boundary

The MT5 v1 JSON protocol and `Mt5SessionGate` assume that a transport has authenticated its peer. The future transport must establish peer authenticity and confidentiality before it creates a session gate or accepts any heartbeat, submit outcome, order snapshot, or reconciliation response. A terminal-provided `account_id`, `terminal_id`, or `server` is an assertion, not proof of identity.

## Required transport properties

1. **Confidentiality and integrity in transit.** Use TLS 1.3 where the selected MT5 deployment can support it. Prefer mutual TLS with a host-managed client certificate for a native bridge agent. If an MT5 Expert Advisor cannot use that model, document the concrete alternative and threat model before implementation; do not silently downgrade to plaintext TCP.
2. **Peer authentication before protocol parsing.** Validate the server certificate chain and hostname on the client. On the host, authenticate the client certificate against a configured trust root and an explicit credential-to-terminal mapping. Do not trust a client-supplied account ID as an authentication credential.
3. **Allow-list binding.** After transport authentication, map the authenticated principal to an immutable configured terminal identity and permitted account IDs. Then require the first valid heartbeat to match that binding. The heartbeat must not add accounts or change terminal/server identity.
4. **No secrets in protocol frames.** Credentials, private keys, bearer tokens and shared secrets must never be placed in JSON frames, IDs, logs, errors, metrics, URLs, or frontend configuration.
5. **Bounded input and connection resources.** Retain the 64 KiB frame limit. Bound concurrent connections, handshake duration, frame buffering, heartbeat age, and per-connection request rate. Reject malformed or oversized input and close the connection on authentication or framing failure.
6. **Replay resistance.** Authentication must bind a fresh connection to a server-generated challenge or a TLS session, and application messages must retain schema version, message ID, correlation ID, account ID, timestamp and stream sequence. A replayed handshake must not establish a new trusted session. Durable order idempotency remains mandatory even with authenticated transport.
7. **Fail-closed reconnect.** A new TCP/TLS connection is a new unauthorised session until authentication and identity binding succeed. Reconnect never clears the durable reconciliation latch; external orders, positions and account state must be reconciled before new exposure.
8. **Credential lifecycle.** Support rotation and revocation without logging credential material. Revocation, certificate expiry, identity mismatch, failed authentication, or uncertain authentication state blocks new exposure and raises a redacted operational event.
9. **No unsafe bypass.** There must be no production option that disables certificate validation or authentication. Local tests may use deterministic test certificates only; test bypasses must not be reachable from production configuration.

## Handshake and readiness sequence

1. Host accepts a connection under bounded resource limits.
2. TLS handshake completes and peer authentication/authorization succeeds.
3. Host maps the authenticated principal to configured terminal identity and account allow-list.
4. A fresh `Mt5SessionGate` is created using only host configuration.
5. The first protocol heartbeat is validated and checked against the authenticated identity binding.
6. Stream sequence continuity and durable checkpoint are observed.
7. Host reconciles venue orders, positions, deals and account state; persists the checkpoint.
8. Only then may the session transition to ready. Risk authorization, OMS lifecycle validation and durable dispatch claim still precede each submit.

Any failure before step 8 must leave the session unable to submit or cancel. Cancel support is enabled only if the authenticated identity is bound and the terminal declares the required capability; a request acknowledgement is not proof of cancellation.

## Required implementation and test gates

Before implementing a real transport, the follow-up code change must define and test:

- Explicit host-side transport configuration with authentication required by default.
- Certificate/identity mapping and account allow-list enforcement.
- Authentication failure, unknown principal, expired/revoked credential, hostname mismatch and identity-switch rejection.
- No heartbeat/session creation before authentication completes.
- Reconnect creates a fresh session and preserves the reconciliation latch.
- Oversized, malformed, slow/incomplete and replayed handshake/frame handling.
- Secret redaction in errors/logs and proof that no credentials enter frames.
- Deterministic local integration tests with test-only certificates, plus an end-to-end restart/reconciliation test using the durable order ledger.

A green unit test or TLS handshake alone does not establish broker compatibility, terminal correctness, risk correctness, or production readiness.

## Explicit non-scope

This contract does not choose the final MT5 transport implementation, provision certificates, add a socket listener, implement an Expert Advisor, map broker symbols/lots/ticks, connect to a broker, enable live mode, deploy services, or move money. Those actions require a separately reviewed implementation and explicit deployment/account selection.


## Implementation status — authenticated transport boundary

The current Rust implementation in `crates/intent-bridge/src/mt5_transport.rs`:

- Loads a TLS 1.3 server certificate/key and a trusted client-CA bundle from configured file paths.
- Requires a valid client certificate, then requires the leaf certificate's SHA-256 fingerprint to match an explicit configured principal.
- Binds that principal to a terminal ID, server name and explicit allowed-account set.
- Applies configurable positive handshake and I/O timeouts, limits concurrent authenticated connections (default 64), and exposes a fail-closed heartbeat freshness watchdog (default 30 seconds). The host runtime must call `check_heartbeat_fresh` from its watchdog; a silent peer is not automatically monitored by a background task.
- Does not expose frames until mutual TLS and principal allow-list checks succeed.
- Requires the first application frame to be a heartbeat matching the authenticated terminal/server/account binding.
- Uses the bounded NDJSON decoder and fails closed on protocol identity/order violations.
- Treats successful socket writes as byte delivery only, never as venue order/fill/cancel confirmation.

**Still required before this is a usable bridge:** certificate-generation/rotation operations, configuration loading from a protected deployment secret/config source, listener resource limits and connection timeouts, authenticated client/EA compatibility tests, heartbeat timeout handling, durable sequence/checkpoint orchestration, dispatch coordinator wiring, order/position/account reconciliation, and full end-to-end restart tests. The current boundary must not be described as production-ready or connected to a live broker.
