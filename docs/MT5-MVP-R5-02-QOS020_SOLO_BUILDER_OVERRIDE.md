# MT5 MVP — R5-02 / QOS-020 Solo Builder Override

**Decision date:** 2026-11-14  
**Decision authority:** Project owner / solo builder  
**Decision type:** Explicit owner risk acceptance and bounded development authorization  
**Status:** RELEASED FOR BOUNDED DATABASE-BACKED IMPLEMENTATION ONLY

## Decision

The project owner explicitly releases the QOS-020 database-backed development gate for the next bounded R5-02 engineering phase and accepts the risks of proceeding as a solo builder without independent review.

This is an owner self-authorization. It is not an independent reviewer approval, security certification, production-readiness determination, or evidence that the admission policy has been independently validated.

## Authorized scope

The authorization covers implementation and local verification of a read-only PostgreSQL admission-facts repository that:

1. Loads broker and broker-server catalogue status and revisions.
2. Resolves the external broker account and its Quant-account binding.
3. Checks organization status, the authenticated actor's active membership, Quant-account status, and active entitlement.
4. Checks the latest verification/evidence record, explicit evidence expiry/revocation state, and a current platform verifier authority grant.
5. Resolves one active worker assignment and a current READY worker session scoped to the exact organization, Quant account, binding, external account, broker, and server.
6. Builds the existing domain admission facts and calls the pure `evaluate_broker_admission` function.
7. Returns an opaque, revision-bearing permit only when every required fact is present and consistent; otherwise it fails closed.
8. Adds additive database schema and automated tests for pending denial, successful admission from a complete test fixture, session-not-ready denial, and verifier-authority revocation.

## Explicit exclusions

This override does **not** authorize:

- Runtime or MT5 worker enforcement/integration.
- A public or organization-facing verification API.
- A provider evidence-ingestion endpoint or cryptographic signature-verification service.
- Authentication of worker certificates or provider workload identities inside this repository method. The caller must supply already-authenticated opaque actor and worker references.
- Execution, order placement, trading mutations, or access to LIVE trading.
- Production database migration execution, production credentials, production deployment, or service restarts.
- Treating broker Demo/Live environments as Quant PAPER/SHADOW/LIVE environments.
- Independent review, certification, or production approval.

## Fail-closed prerequisites

The database-backed evaluator intentionally does not promote existing registrations. Legacy evidence without explicit expiry and an authority-grant reference is ineligible. A successful decision requires:

- An active broker and server catalogue entry.
- A verified external account and verified binding.
- An active organization, active membership, active entitlement, and active Quant account.
- Authenticated evidence with a future expiry and no revocation.
- A latest VERIFIED decision linked to an active, unexpired `BROKER_ACCOUNT_VERIFY` authority grant whose revision still matches.
- Exactly one active, in-scope worker assignment and a READY session whose `ready_until` is in the future.
- Consistent revisions in the repeatable-read snapshot.

## Operational caution

The repository method is a point-in-time read-only admission check. Its permit must not be cached or treated as durable authorization. Any future runtime consumer must re-evaluate immediately before the protected read operation and must independently authenticate the actor/worker principals. The caller must fail closed on repository errors.

The current implementation does not cryptographically verify provider evidence. The persisted `AUTHENTICATED` state must only be written by a separately trusted evidence-verification process; until that process and its key/attestation trust model exist, this implementation is not sufficient for production admission.

## Governance effect

- **QOS-020:** RELEASED for the bounded database-backed implementation described above, under owner risk acceptance.
- **R5-02:** Remains PENDING as a substantive governance decision.
- **QOS-015C:** Remains NO-GO.
- **MT5-MVP-01A:** Remains NOT AUTHORIZED.
- Production use: NOT AUTHORIZED.

The owner may authorize further scopes separately. This override must not be interpreted as permission to cross any of the exclusions above.
