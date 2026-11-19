# QOS-020 test-lane review — 2026-11-19

## Current implementation reviewed

The current main-branch guard is materially stronger than the earlier failing attempt:

- `provision.py` creates a fresh run ID, Compose project, volume, network, PostgreSQL container, random credentials and protected runtime directory for every run.
- The manifest records the exact container ID/name, generated resource identities, loopback endpoint, pinned PostgreSQL 16 image digest and a unique instance-creation marker.
- `qos020_guard.py` re-inspects Docker container, volume, network, database marker, PostgreSQL major version, endpoint and restricted SQLx role; it checks the URL password against the protected secret file.
- The guard rejects protected ports/databases and non-loopback endpoints.
- CI invokes negative guard tests, provisions the isolated DB, wraps Rust checks/tests in the guard, and verifies SQLx-created databases were cleaned up.

## Previous blocker and current interpretation

The earlier failure was that the manifest's container ID could not be inspected. The current provisioner no longer relies on a pre-existing/static container ID: it obtains the exact ID from the newly started Compose project and writes it into a run-specific manifest. This removes the stale-manifest assumption that caused the reported failure mode.

However, no successful workflow run is currently evidenced for merge commit `859cf29bb09caa1841fe1b62d07d8ccd22c3bc60` (the GitHub Actions workflow-run query returned no runs). Therefore QOS-020 is **implemented but not yet proven green on this head**. Do not run local database-capable tests against any existing local/staging database.

## Safe next verification

1. Trigger CI on the new PR head; let the provisioner create its fresh disposable instance.
2. Require all negative guard tests to pass.
3. Require the guard-wrapped workspace checks and SQLx tests to pass.
4. Require cleanup verification to report no leftover SQLx databases.
5. Inspect failed job logs if any step fails; do not bypass the guard or edit its identity assertions to make a run pass.

A CI-created disposable DB is the approved test lane. No user's local database, staging, production or recovery dump is an acceptable substitute.
