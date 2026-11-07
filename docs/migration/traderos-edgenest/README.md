# TraderOS → EdgeNest migration hardening

This branch builds on the existing migration tool. It does not touch either production database.

## What is included
- Read-only, per-user extraction from TraderOS PostgreSQL.
- Exact decimal and timezone-preserving JSON snapshots.
- Screenshot binary extraction and SHA-256 checksums.
- Authenticated, owner-scoped EdgeNest Journal API imports.
- Stable source IDs for retry-safe trade imports.
- Explicit warnings for source facts not represented as first-class destination records.
- Dry-run planning without destination writes.

## Important boundaries
- No TraderOS users, password hashes, sessions, refresh tokens, MT5 credentials or secrets are migrated.
- No OMS order, fill, position, broker connection or execution state is created.
- EdgeNest remains the authority for identity, account ownership, MT5 connectivity and execution.
- Source data is read-only and must remain available for rollback/reconciliation.
- Production migration is blocked until backups, staging, target identity mapping and object-storage access are verified.

See [PHASE-4-5-MIGRATION.md](PHASE-4-5-MIGRATION.md) for operational procedure and [MIGRATION-COVERAGE.md](MIGRATION-COVERAGE.md) for parity boundaries.
