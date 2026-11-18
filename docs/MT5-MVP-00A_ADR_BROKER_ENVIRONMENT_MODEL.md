# MT5-MVP-00A ADR-01 — Broker and Quant Environment Model

**Status:** Proposed decision for review; not implemented.  
**Scope:** One read-only MT5 DEMO stream for 01A.

## Context

The repository already has two separate environment dimensions, but they are not yet joined by a verified eligibility decision:

- `broker_servers.environment` in `migrations/20261113000001_broker_account_registry.sql`: `DEMO`, `LIVE`, `UNKNOWN`.
- `accounts.environment` in `migrations/20261107000002_dashboard_account_access.sql` and `migrations/20261111000002_account_environment_provenance.sql`: `PAPER`, `SHADOW`, `LIVE`, with independent verification provenance.

These values describe different facts. Broker environment is a fact about the external account/server. Quant environment describes the platform account's operating mode. The external broker server catalog is not proof of a particular account's environment. A server name substring or user-supplied label must never decide it.

## Options considered

1. Collapse both concepts into one environment enum. Rejected: it loses whether a value describes external broker reality or platform policy and makes `DEMO == PAPER` appear automatic.
2. Preserve independent enums and define an explicit, versioned eligibility matrix. Recommended: each authority owns its own fact and policy maps an approved pair to an allowed operation set.
3. Defer all environment eligibility. Safe in the short term but prevents even the read-only DEMO pilot; use as fail-closed behavior until the policy is approved and enforced.

## Decision

Recommend two independent dimensions and an explicit policy version. For the initial read-only 01A policy, recommend:

| Broker environment | Quant environment | Read-only eligibility | Execution eligibility |
|---|---|---|---|
| DEMO | PAPER | Permit only for explicitly verified 01A ingestion | Prohibited |
| DEMO | SHADOW | Deny by default | Prohibited |
| DEMO | LIVE | Prohibited | Prohibited |
| LIVE | SHADOW | Permit only with separate explicit read-only observation authorization | Prohibited by this ADR |
| LIVE | LIVE | Deny until separate live-account authorization and execution governance | Separate authorization required |
| LIVE | PAPER | Deny by default | Prohibited |
| UNKNOWN | Any | Prohibited | Prohibited |

All unlisted pairs are denied. Broker environment authority is the platform-maintained broker/server catalog plus account-specific evidence and an authorized verification decision. Quant environment authority is the Quant account record plus its verification revision/actor/reason. The eligibility policy is a separate platform-owned, versioned policy; it does not rewrite either source value.

The first 01A stream is exactly one broker `DEMO` account bound to one Quant `PAPER` account, read-only, with a current verified binding. The DEMO label must be confirmed by independent operator evidence for that account and server, then compared to the provider observation. Neither mTLS nor a server-name convention establishes ownership or environment.

If the observed account environment becomes `UNKNOWN`, or conflicts with the approved classification, suspend admission immediately and terminate an active stream. A broker server rename or identity change invalidates the prior server revision and binding for new connections; create a new catalog identity/revision, gather new evidence, and require a new approval. Do not silently edit an immutable canonical identity. Any environment or Quant-account environment change increments its revision, appends an audit event, invalidates admission cache, and requires policy re-evaluation. A prior session cannot survive a relevant revision mismatch.

## Rationale and schema implications

The external broker environment and internal Quant account environment are separate dimensions, but the current schema does not fully express both facts at the correct granularity. `broker_servers.environment` classifies a server; `external_broker_accounts` has no broker-environment or account-environment verification field, and its `account_category` must not be repurposed as one. The schema also has no explicit `DEMO` Quant environment (which is intentional under this model) and no binding column for the policy version or account/server revisions accepted at admission. Quant accounts have verification provenance and an account revision.

Before implementation, add (through a separately authorized forward-only migration) explicit per-external-account `broker_environment` (`DEMO/LIVE/UNKNOWN`), verification state/revision, verified time/actor and evidence reference, with append-only environment-change events. Do not infer this value from `broker_servers.environment`; require both server catalog classification and account-level evidence to agree. Preserve existing rows as unverified/unknown until evidence is reviewed. Enforce revision increments and audit events for server environment changes as well. Record the accepted policy version on an admission/binding event and bind it to Quant account revision, entitlement revision, broker server revision, external-account environment/verification revision, binding revision, terminal identity and worker identity. Do not overload either existing environment column.

## Consequences and risks

- Existing legacy/unverified Quant account environment values remain ineligible until verified; no auto-promotion.
- `DEMO → PAPER` is an explicit policy exception for read-only data observation, not semantic equivalence.
- LIVE/SHADOW observation may expose sensitive account data and requires explicit customer and platform authorization.
- Broker catalog environment changes need revision/event enforcement before their values are trusted at runtime.
- Changing classifications may interrupt existing streams. This is intentional fail-closed behavior.

## Acceptance criteria

1. Separate enums and authorities remain visible in APIs, records and audit events.
2. Eligibility matrix is versioned, reviewed by the authorized owner and evaluated fail-closed.
3. No decision uses server-name text or a worker configuration label as environment evidence.
4. 01A DEMO/PAPER eligibility requires current verified broker evidence, account environment verification and active entitlement.
5. Environment, server, or policy revision changes revoke cached and live admission within the approved revocation objective.
6. No execution permission is granted by this read-only policy.

