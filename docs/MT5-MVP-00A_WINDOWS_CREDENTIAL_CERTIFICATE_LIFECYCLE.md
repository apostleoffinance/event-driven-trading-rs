# MT5-MVP-00A ADR-03 — Windows Credential and Certificate Lifecycle

**Status:** Provider-neutral design proposed; cloud/enterprise provider decision required. No credentials or certificates were created.

## Context

The Windows worker now reads `MT5_PASSWORD_DPAPI_FILE` and decrypts a CurrentUser DPAPI-protected UTF-8 secret under the same Windows identity. Plaintext `MT5_PASSWORD` is rejected. `scripts/protect-mt5-password.ps1` provisions the local protected file and restrictive ACLs. This reduces plaintext-at-rest exposure but is not a managed secret manager and does not provide centralized rotation, revocation, audit or workload-identity delivery. `mt5-certificate-acl-review.ps1` checks file ACLs, certificate/private-key pairing, pin match, validity and online chain/revocation verification where `certutil` supports it; it explicitly does not prove runtime revocation enforcement or exercised rotation. Rust `Mt5TransportConfig::from_pem_files` loads PEM private keys at process startup and pins exact client leaf fingerprint; registry-backed revocation refresh is not yet wired.

## Options considered and decision

| Option | Strength | Limitation | Decision |
|---|---|---|---|
| Cloud secret manager with VM workload identity | Central policy, audit, rotation and revocation; avoids static access keys when host supports workload identity | Requires choosing a cloud/host provider and validating Windows integration | **Preferred where supported by selected host provider** |
| Enterprise secret-management service / vault with machine identity | Provider-neutral operations, centralized audit/rotation, can integrate with enterprise PKI | Adds a service dependency, agent/connectivity and operational ownership | **Acceptable alternative** if no cloud-native workload identity is available |
| Windows Credential Manager / DPAPI protected storage | Windows-native protection scoped to user/machine; can reduce plaintext-at-rest exposure | Does not alone provide central lifecycle, rotation, approval, auditing, revocation or outage policy; DPAPI-encrypted file is not a full secret manager | **Not the authoritative lifecycle**; may be a protected cache only after explicit security approval |
| Plaintext long-lived environment variable or `.env` file | Simple and compatible with current code | Exposed through process/environment diagnostics, weak rotation/audit boundary | **Rejected for production** |

Use a narrow provider-neutral interface such as `BrokerCredentialProvider::retrieve(secret_ref, service_identity) -> short_lived_secret_handle` and a separate `WorkerKeyProvider`/certificate enrollment interface. Provider implementation is selected only after Windows host/cloud ownership is decided. The broker password and TLS private key are distinct secrets and may not share an access policy or rotation event.

## Broker credential lifecycle

- Dedicated non-interactive Windows service SID (or dedicated service identity) may request one named DEMO credential only. No human developer or general host administrator receives routine read access; break-glass access is separately approved, time-limited and audited.
- Obtain the credential at worker start over authenticated TLS using workload identity. Fail startup closed if the secret manager is unavailable or the credential version is revoked. Do not fall back to a local plaintext cache or an older version unless an explicit rotation rollback policy permits it.
- Current MetaTrader5 Python API takes a password string. Therefore the value will transiently exist in worker process memory. Keep lifetime minimal; do not serialize it, expose it through command arguments, crash dumps, logs, tracebacks, telemetry, environment dumps or support bundles. Use a zeroizing wrapper where possible, noting Python/native copies cannot be reliably zeroized. Restrict process dump access and disable unattended dump collection or redact it under host policy.
- Retrieve only at process startup; do not refresh mid-session unless the provider safely supports re-authentication. Rotate by adding a new credential at the broker, updating the secret-manager version, restarting the worker in a controlled window, verifying exact login/server and reconciliation, then revoking the old broker credential. Revoke immediately for suspected compromise and close the stream.
- Secret-manager outage before start blocks service. Outage after retrieval does not authorize indefinite continued use; apply the approved short credential lease/restart interval, alert, and stop before the lease expires. The exact lease requires broker/provider behavior and operator decision.
- Maintain audit events for secret reference/version, requesting service identity, timestamp, result category and approver without secret value. Human access requires ticket/reason, MFA, time limit and review.

## Worker certificate and server trust lifecycle

- Operate an approved enterprise/workload CA. Issue one client-auth certificate per worker installation with a unique URI SAN such as `spiffe://edgenest/mt5-gateway/<host-id>/<terminal-id>/<account-id>` plus controlled DNS SAN only where needed. Identity components are immutable for that cert. Exact URI/SAN-to-registry assignment must be checked, not inferred from a CN or file name.
- Require `clientAuth` EKU for worker and `serverAuth` for daemon; appropriate key usage; TLS 1.3 only; strict hostname/SAN validation; bounded certificate lifetime. Worker validates daemon CA/name; daemon validates chain and exact certificate principal. Current Python `tls_context` uses TLS 1.3, hostname validation and CA trust; Rust transport enforces TLS 1.3, CA chain and leaf SHA-256 fingerprint principal. The daemon currently has no dynamic registry lookup or demonstrated CRL/OCSP checking.
- Prefer non-exportable Windows CNG/HSM private keys bound to service SID where supported. Current Rust/Python code loads PEM files, so non-exportable-key support is not available without a future signer/keystore integration. If the selected provider cannot support non-exportability, use a generated per-host key in an ACL-restricted machine store, allow read only to the dedicated service identity, prohibit export where OS/key provider allows, and document the residual risk. Never reuse a certificate/key across hosts or accounts.
- Separate client private key, daemon private key, client CA trust bundle, server CA trust bundle, broker password, and ordinary non-secret configuration into distinct objects with separate ACLs, owners and rotation schedules.
- Set expiration alerting at 30/14/7 days and block startup when expired/not-yet-valid. Rotate with overlap: issue new cert, register/allow new fingerprint/SAN, deploy to worker, verify authenticated ingestion, then remove old fingerprint and revoke old cert. Old and new cert overlap is short and tracked.
- Revocation must take effect by both CA CRL/OCSP verification and an immediate registry denylist/assignment-state lookup. Loss/compromise of host: suspend workload assignment, deny fingerprint, revoke certificate, terminate active session, rotate broker password independently, preserve forensic evidence, and re-enroll a clean host under new identity. Re-enrollment never changes or transfers broker account ownership.
- Existing exact-fingerprint config is loaded at startup; changing a file alone does not affect a running daemon. Dynamic denylist/revalidation or a controlled daemon restart is required. 01A must define and test revocation propagation bound and ensure active connection closure.

## Separation, options and acceptance

Broker password authenticates the terminal to broker; client private key authenticates the worker to bridge; daemon private key authenticates bridge to worker; trust roots validate peer issuers; host/config identify non-secret endpoints and policy. Each has its own issuer/owner, ACL, audit trail, rotation and emergency revocation process.

Before 01A, name the host/cloud provider or enterprise vault, workload identity method, Windows service SID, secret manager SLA/outage behavior, broker-supported password rotation, CA owner, certificate revocation mechanism, CNG/PEM key handling decision, rotation operator and evidence location. None is currently selected; this is a blocker for 01A-2.

Acceptance: secret retrieval is authenticated and least-privileged; no plaintext env/file deployment; startup fails closed on secret service outage or revoked version; log/dump scanning finds no secret; distinct rotation and emergency revocation drills pass; new cert is accepted only after registry assignment; revoked cert and active stream are denied within the approved objective; ACL review passes under actual service identity; provider and certificate identities are never conflated with broker ownership.

