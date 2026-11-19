# MT5-MVP-00A ADR-02 — Verified Broker-Binding Admission

**Status:** Proposed integration design; not implemented.  
**Authority:** Existing EdgeNest broker registry and organization/account entitlement records remain the sole authority for binding and verification.

## Context and existing boundary

The schema in `migrations/20261113000001_broker_account_registry.sql` separates `brokers`, `broker_servers`, `external_broker_accounts`, HMAC identifier tokens, `trading_account_broker_bindings`, evidence, verification decisions and binding events. `migrations/20261112000002_organization_tenant_foundation.sql` provides organizations, memberships, account entitlements and Quant account environment verification. The external login is stored as keyed HMAC tokens rather than plaintext.

`crates/journal-api/src/broker_registry.rs::register` currently lets an authorized organization owner register a pending binding. `crates/persistence/src/repos/broker_registry.rs::register_pending_binding` checks active organization/owner and active entitlement and writes pending records transactionally. The API lifecycle transition accepts only SUSPENDED or REVOKED; its source comment says there is no platform authority to set VERIFIED. The schema can record verification evidence and decisions, but an approval workflow is not implemented. `crates/intent-bridge/src/bin/mt5-bridge-daemon.rs::main` currently builds a single principal from environment configuration; it does not query the registry. Its mTLS fingerprint is not broker ownership proof.

## Options considered

1. Keep the static daemon account allowlist as authority. Rejected: it can drift from entitlement, verification, environment and binding revisions.
2. Add an independent MT5 authorization registry. Rejected: it would conflict with the existing broker registry and create two authorities.
3. Add a narrow admission adapter backed by the existing registry database/service and revalidate active streams. Recommended. The current registry remains authoritative; the bridge receives a typed allow/deny result and accepted revision tuple.

## Proposed admission decision

Admission is allowed only if one consistent registry read establishes all of the following at one current revision:

1. Organization is ACTIVE.
2. Organization account entitlement is ACTIVE and belongs to that organization and Quant account.
3. Quant account is ACTIVE, its environment is verified, and the versioned environment policy allows the pair.
4. External broker account is ACTIVE and verification status is VERIFIED.
5. A current binding for the exact organization, Quant account and external account is VERIFIED and not suspended, revoked, superseded or stale.
6. Verification decision references an existing authenticated evidence record for the same broker, exact broker server, observed-login HMAC and provider type; it records an authorized actor, authority and durable evidence reference/digest.
7. The observed broker login matches an HMAC token for the exact broker/server under one of the currently retained identifier-key versions; compare in constant time.
8. The broker identity and exact server catalog identity match the observed provider values. No fuzzy server aliasing at admission.
9. Broker environment and server revision match the approved evidence and policy. UNKNOWN is denied.
10. Assigned terminal identity and host identity match the approved binding.
11. The mTLS principal fingerprint maps to the workload assignment for this account/terminal and is not revoked.
12. Binding, external-account verification, entitlement, account, broker/server and environment-policy revisions match the current revisions.

No ordinary data frame can establish its own authority. Before the first v2 telemetry frame is persisted, the authenticated TLS connection must complete a separate, bounded admission exchange. The exchange carries the configured Quant account ID, terminal ID, exact server identifier, provider login observation and protocol/build identity. It occurs inside TLS; raw login is handled only in memory by the admission boundary, transformed to the registry's HMAC using retained key versions, and never logged or stored. The registry's existing HMAC keys must not be copied into the worker. This control message is not silently inserted into the strict v2 market-data frames. Its framing/version must be explicitly specified and pinned. If the worker cannot securely provide a trustworthy observation, admission denies; the independent operator evidence remains necessary.

## Lifecycle model

The lifecycle uses existing persisted statuses where possible. `ACTIVE` is derived runtime eligibility, not a new binding status:

```text
UNBOUND (no row)
  → PENDING_EVIDENCE (binding + account registration created)
  → PENDING_APPROVAL (authenticated evidence submitted and reviewed)
  → VERIFIED (platform verification decision; immutable revision)
  → ACTIVE (derived only while every entitlement, account, broker, environment,
            binding and workload predicate remains current)
```

Other transitions:

- **REJECTED:** evidence or identity fails; append decision/event; no admission. Corrected evidence starts a new verification revision; do not rewrite history.
- **SUSPENDED:** temporary risk, entitlement, environment, host or certificate concern; immediately revoke active stream and require explicit re-review.
- **REVOKED:** terminal binding removal; release reserved identity only according to registry policy. Reassignment creates a new binding and auditable reason, never mutates identity in place.
- **EXPIRED:** no binding/evidence expiration column currently exists. Recommend short-lived provider observation evidence (maximum 5 minutes at admission) and a policy-defined periodic re-attestation. On evidence, certificate, catalog, or policy expiry, suspend admission; do not add an implicit expiry to an existing row. Persistent ownership approval is re-reviewed on changes and on a separately approved periodic schedule.
- **Reassignment:** old binding first revoked; new active entitlement, HMAC identity, evidence, independent approval and binding revision required. Never transfer a prior verification decision.

### Evidence and authority

- Organization owner may request/register a binding, provide a reason, and submit customer authorization material. A registered worker may submit only a signed/authenticated observation from its assigned stream.
- Approval of broker ownership and DEMO/LIVE classification is a platform verification function, not organization self-approval. Recommend two-person control for the 01A pilot: evidence submitter and verifier must be distinct identities. The current API has no platform approval authority; this is an implementation dependency.
- Persist evidence metadata, authenticated principal, observed time, HMAC token versions, attestation digest, immutable evidence ID, reviewer/authority/reference and decision reason. Store supporting documents in a separately approved access-controlled evidence repository and retain only opaque reference plus digest in registry. Never store passwords, investor credentials, or unnecessary raw account identifiers.
- Existing database records contain evidence digest and references but no dedicated object-storage integration or expiration field. Retention period, legal hold and deletion policy require a security/privacy owner decision.

### HMAC key rotation and revision invalidation

The current `BrokerIdentityHmac` in `crates/journal-api/src/broker_registry.rs` accepts a keyset and active key version; persistence writes tokens for configured versions. Rotation must overlap old and new keys for lookup. Because HMAC is irreversible and raw login is deliberately not stored, old account tokens cannot be re-derived from database values. During overlap, re-observe the login through the authenticated admission/evidence path and append the new key-version token. Do not retire an old key while any binding/evidence still depends on it. Removal requires a completeness report and creates a planned re-attestation/rejection if any identity remains unresolved.

Any relevant change increments its revision and emits append-only audit history. Current `broker_servers.environment` is mutable and the schema does not provide a dedicated change-event relation or demonstrated automatic revision increment for it; do not trust a changed value until that gap is addressed. A changed account, server, verification, entitlement, terminal assignment, certificate identity or eligibility-policy revision makes the prior admission stale and terminates the active stream.

## Daemon admission, cache and revocation behavior

1. Complete TLS 1.3 chain verification and exact client certificate fingerprint mapping.
2. Resolve certificate fingerprint to one assigned workload/host/terminal/account identity; reject missing, duplicate or revoked assignment.
3. Run the bounded admission exchange and ask the registry-backed adapter for one current eligibility decision.
4. Only after allow, create the stream session, accept v2 heartbeat, validate identity and sequence, and persist data.
5. Before every reconciliation confirmation, recheck the decision/revisions. Reconnect always performs full admission again.
6. For one-host 01A, poll current revisions every 5 seconds while connected (maximum staleness of authorization 5 seconds) and close the stream immediately on deny, revision change, or registry lookup error. The 5-second objective requires testing and must be adjusted only by an explicit security decision. No disconnected cache grants continued access.
7. Use a process-local single-instance lock plus a database-backed unique stream lease/advisory lock keyed by account/terminal so two workers or daemon replicas cannot own the same stream. Lease expiry and shutdown must fail closed. Current code lacks registry admission, leases and dynamic invalidation.
8. Registry outage at initial admission means reject. During a live session, inability to refresh authority within the 5-second bound means mark degraded, stop accepting frames, close the TLS connection, and retain stale data as stale. Re-admit only after the registry is healthy and all current revisions match.

This is a narrow adapter over the existing registry; it must not hold its own account ownership state. A read-only registry query/typed decision is preferable to duplicating records in daemon config. Privileged database credentials stay server-side; the MT5 worker receives only its own certificate and operational configuration.

## Consequences, risks and acceptance criteria

- Requires a separately authorized protocol admission preamble or equivalent mutually authenticated control-plane step; schema v2 telemetry frames stay unchanged.
- Requires a platform verification authority, independent approver policy, workload-certificate assignment, evidence retention mechanism, revision guarantees and active-stream invalidation. These are absent today.
- Registry lookup availability becomes part of gateway availability, intentionally fail-closed.
- mTLS identifies a workload, not a broker account. Independent account evidence remains mandatory.

Acceptance: negative tests for every predicate; verified admission before any persisted frame; stale revision/unknown environment/registry outage/certificate revocation closes the stream within the approved bound; duplicate stream ownership rejected; HMAC values and raw login never logged/stored; reconnect repeats all checks; event history is append-only; no order dispatch is introduced.

