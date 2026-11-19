# MT5-MVP-02B — Gateway and terminal-slot registry

**Status:** repository implementation proposal. No Windows host or MT5 terminal is provisioned by this change.

## Purpose

Represent EdgeNest-owned gateway hosts and isolated MT5 terminal slots as durable records. A host has an explicit terminal capacity. Each slot has a unique terminal ID, protected profile reference, OS service-identity reference and mTLS worker principal. A slot is created only for an exact VERIFIED binding. A worker assignment can be linked to it only when its organization, Quant account, binding, external account, worker, broker and server all match a READY slot.

Customers remain outside the infrastructure lifecycle: EdgeNest provisions, monitors, patches and replaces gateway hosts. This registry is an internal control-plane concern.

## Data model

- `mt5_gateway_hosts`: opaque host reference, provider/region metadata, lifecycle state and bounded capacity.
- `mt5_terminal_slots`: gateway association, exact organization/Quant account/verified broker-binding scope, broker/server association, unique terminal/profile/service/worker identities, lifecycle state and isolation schema version.
- `broker_worker_assignments.terminal_slot_id`: optional compatibility link. Existing assignments are not backfilled. New linked assignments are checked at insert time against a READY slot.

The database prevents duplicate terminal IDs on a host, duplicate profile/service/worker identities, active assignments sharing a slot, cross-account slot reuse, unverified binding enrollment, over-capacity host registration, identity mutation and deletion of registry history.

## Isolation rules

1. One external broker account per terminal slot.
2. A slot never shares its MT5 profile/data directory, OS service identity, worker principal, client certificate, sequence/checkpoint state or broker secret with another slot.
3. Worker identity is unique across slots. A worker assignment must match the slot's organization, Quant account, binding, external broker account, worker, broker and server.
4. Host capacity defaults to one. Increasing it is an explicit operator decision after CPU/RAM/disk/network and broker-connection measurements. The schema maximum is a guardrail, not a performance promise.
5. Host replacement or slot reassignment requires a new reviewed registry revision and fresh fencing/lease acceptance. Do not repoint an active slot silently.
6. Gateway/slot registry status is not itself an admission permit. Runtime ingestion must be upgraded to read and enforce slot status/identity before registry-backed slots are used in production.

## Broker/server configuration contract

The checked-in TenTrade example is intentionally disabled. It records MT5/Python connector family, DEMO environment, operator-assisted enrollment, read-only behavior and explicit deny flags for all order operations. The canonical server identifier is blank until copied from the user's TenTrade DEMO terminal/portal and verified. No credentials belong in this file.

Supported catalogue entries must have:
- immutable broker ID and broker-scoped server ID;
- exact provider-observed canonical server identifier and versioned normalization;
- connector family and provider type;
- explicit DEMO/LIVE/UNKNOWN environment and ACTIVE/UNVERIFIED/DISABLED lifecycle;
- source reference and review metadata;
- an explicit capability set. MT5 support does not imply MT4, cTrader, FIX or proprietary API support.

## Security and compatibility boundary

This migration adds nullable slot linkage so current single-slot acceptance fixtures and existing assignments remain compatible. It does not backfill records or change the current daemon's admission behavior. The trigger validates slot-backed assignment creation, but existing unlinked assignments continue to work during the transition. Do not activate a second terminal until the daemon admission query, supervisor, lease fencing and recovery path all enforce the slot registry.

No credential intake, secret manager integration, Windows provisioning, RDP, live account support or order execution is introduced.

## Required follow-up before using a second slot

- Update the authenticated worker principal/admission query to require an ACTIVE assignment and READY slot with matching gateway, terminal, broker/server and worker identities.
- Bind the daemon's configured terminal ID to the registry slot; reject client/heartbeat substitution.
- Ensure the worker lease key and persistence scope cannot collide across tenants or slots.
- Add PostgreSQL integration tests for capacity races, duplicate identities, cross-broker mismatch, stale/revoked slot, and concurrent assignment attempts.
- Run target-Windows capacity/isolation tests. Do not infer safe terminal density from the VPS plan.
