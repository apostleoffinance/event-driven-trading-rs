# TraderOS → EdgeNest migration (Phases 4–5)

This tool migrates historical journal records only. It never copies TraderOS users,
password hashes, session/refresh tokens, MT5 connection tokens, or credentials. Source
PostgreSQL is opened read-only. EdgeNest is written only through its authenticated Journal API.
Never run this against production until a full dry-run/reconciliation report is reviewed.

## Phase 4 — tooling and schema readiness

- Snapshot selected TraderOS user rows to JSON files (source tables: accounts, risk profiles,
  setups, trades, psychology, checklists/responses, screenshots).
- Preserve Decimal values as strings, timezone-aware timestamps, source UUIDs and screenshot
  bytes separately; secret-like fields are stripped from JSON snapshots.
- Explicit owner/account/setup mapping state, dry-run validation, idempotent source trade IDs,
  retryable API calls, and JSON audit report.
- The source is not modified and no target OMS orders/fills/positions are created.

## Phase 5 — historical journal migration

The CLI imports journal accounts, setups, trades and psychology using the API. Trade import
uses source_system=TraderOS and original trade ID as source_trade_id; duplicate retries
are safe. Closed trades with missing exit time/price and naive timestamps without a timezone
are rejected rather than guessed.

TraderOS-only facts that are not yet represented as first-class EdgeNest columns are retained
in the extraction snapshot (risk profiles, planned RR/percent, realized RR, stop/target pips,
MT5 IDs, instrument metadata, checklists and screenshot records). The CLI does not silently
discard these: its reconciliation report flags screenshot object transfer as incomplete.
Checklist migration into Journal UI/API and S3/R2 binary transfer must be completed/configured
before claiming full historical parity.

## Run

Install dependencies:

    python3 -m venv .venv
    source .venv/bin/activate
    pip install -r scripts/traderos-migration/requirements.txt

1. Configure the EdgeNest API and explicit source-user → EdgeNest JWT mapping:

    cp scripts/traderos-migration/config.example.json migration-config.json

Edit migration-config.json. Keep JWT out of Git and logs.

2. Export one TraderOS user from a read-only database credential:

    export TRADEROS_DATABASE_URL='postgresql://READ_ONLY_USER:...@.../traderos'
    python scripts/traderos-migration/migrate.py extract --user-id '<TraderOS UUID>' --out ./snapshot.json

3. Validate snapshot mapping:

    python scripts/traderos-migration/migrate.py plan --snapshot ./snapshot.json --config ./migration-config.json

4. Run dry-run first:

    python scripts/traderos-migration/migrate.py apply --snapshot ./snapshot.json --config ./migration-config.json --dry-run

5. Apply only after reviewing report:

    python scripts/traderos-migration/migrate.py apply --snapshot ./snapshot.json --config ./migration-config.json

## Important operational constraints

- A single source snapshot is per TraderOS user. Use a separate snapshot file per user.
- Source timezone must be explicit; do not reinterpret naive timestamps.
- Account balance is imported as the historical journal account's initial balance; TraderOS live
  account equity and prop-firm risk-profile fields are snapshot-only until a dedicated target
  schema/API is implemented.
- Binary screenshot bytes are extracted to a local directory and never embedded in JSON. Do not
  delete the source. S3/R2 transfer and attachment linking are a separate, explicit step.
- Import uses the current API batch limit of 10,000 trades. Checkpoints preserve account/setup IDs.
- Keep snapshot, state, reports and JWTs out of Git; snapshots may contain sensitive financial data.
- For production, run from a secure host with private DB networking, a least-privilege source
  DB account, target JWTs, and encrypted backups. Reconcile counts and values before cutover.

## Verification

    python -m unittest discover -s scripts/traderos-migration -p 'test_*.py' -v
