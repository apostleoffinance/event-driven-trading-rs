# QOS-015C — Runtime and deployment reconciliation

**Status: IN PROGRESS — repository inspection complete; host-side inventory still required.**
**Safety: read-only. No services, terminals, certificates, credentials, databases, or volumes are modified.**

## Objective

Produce one evidence-backed inventory that ties together:

1. The authoritative checkout, branch, HEAD, tracked/untracked modifications, and locally known remote refs.
2. The committed build manifest and exact worker/provider/EA/daemon artifact hashes.
3. The active v1 macOS LaunchAgents and listening ports.
4. The v2 staging Docker context, container state, Compose project/service labels, Compose source path, mounts, published ports, external volume identity, and container health.
5. A clean deployment decision that explicitly blocks unsafe replacement, shared configuration, unverified artifacts, and destructive cleanup.

The collector is scripts/qos015c_runtime_inventory.py. It uses Python's standard library and read-only Git, Docker, LaunchAgent, and lsof queries. It never invokes Docker Compose lifecycle commands, database migrations, or terminal commands.

## Repository facts established from GitHub

Inspection on 2026-11-09 established:

| Item | Current observation |
|---|---|
| Repository default branch | main |
| main HEAD | cc2fc680c998681b3240cc9da5db5af18607d5f6 |
| release/mt5-bridge-v2-rc1 HEAD | a2f0bbeafe7f2d4c1a3c60048e43e7f87e6dfb49 |
| Relationship | main is 596 commits ahead of the release branch and 0 behind |
| Reviewed v2 implementation commit in existing manifest | 76006c1601ad017002e64907712872bcb78de128 |
| Existing evidence commit in manifest | 7bfd4358a3dcf840829a809a38108c09220f5d41 |
| Existing build-manifest snapshot | updated 2026-11-01; historical, not proof of current deployment |
| Staging Compose project | edgenest-mt5-bridge-v2-staging |
| Staging DB endpoint | 127.0.0.1:15543, database mt5_v2_staging |
| Staging volume | external edgenest_mt5_bridge_v2_staging_data |
| Development Compose DB | separate service in docker-compose.yml, host port 15432 |
| Recorded v1 DB endpoint | 127.0.0.1:15433/trading; last recorded connection attempt was refused |
| Deployment state in existing plan | not approved or executed |

**Important source/artifact finding:** the current main tree has advanced substantially beyond the RC evidence snapshot. The existing manifest explicitly says the daemon artifact was built from commit 76006c1; it must not be treated as an artifact built from current main. The manifest also records the EX5 as not accepted pending an approved runtime load and a PING reporting EA build 2.00 / provider protocol 1. Rebuild and re-hash from one reviewed immutable source revision before any v2 deployment.

main is the repository's latest integration branch, but this fact alone does not make it the deployment source. The selected source commit must be pinned in a new reviewed build manifest, and worker, daemon, provider and EA must be proven compatible at that exact revision.

## Run the local read-only collector

From the intended repository checkout on the Mac:

\`\`\`bash
python3 scripts/qos015c_runtime_inventory.py \\
  --repo "$PWD" \\
  --daemon-binary "/absolute/path/to/the/candidate/mt5-bridge-daemon" \\
  --ea-source "/absolute/path/to/installed/QuantOSReadOnlyProvider.mq5" \\
  --ea-binary "/absolute/path/to/installed/QuantOSReadOnlyProvider.ex5" \\
  --output "$HOME/qos015c-runtime-inventory.json"
\`\`\`

If the candidate daemon or installed EA paths are not known, omit those flags. The report will mark those identities unknown; do not guess paths. Repeat --ea-source / --ea-binary if multiple candidate files need to be compared.

The output file is created with mode 0600 where supported. Review it before sharing. It deliberately excludes environment variables, DATABASE_URL, passwords, private keys, tokens, Docker Config.Env, and file contents. It reports container mounts and Compose labels because those are needed to establish deployment provenance.

The collector does not fetch remote Git refs. Its origin/* values are whatever refs are already present in that checkout. If the local checkout has not fetched recently, record that limitation; do not run a fetch as part of this read-only inventory.

## Required operator checks not inferable from GitHub

The collector is evidence gathering, not automatic approval. Reconcile its JSON with the following:

- Confirm the actual git status and branch; preserve every dirty and untracked file.
- Identify the process executable paths for the active worker and daemon. Confirm whether the active LaunchAgents still use the v1 checkout and shared environment file described in the historical plan.
- Confirm the v1 and v2 workers use distinct environment files, account/terminal identities, bridge ports, certificate identities, and database URLs. Do not print or paste secret values. Compare secret-file paths and non-secret account/terminal identifiers only.
- Confirm the v2 container is the intended Postgres service, its health check is healthy, the Compose project is correct, and its mount references the exact external volume. Confirm the Compose config-file label: if it points to a Copilot/session-state file rather than the repository file, do not run Compose lifecycle commands from the repository file.
- A green Postgres container health check proves only the configured health probe passed. Confirm the expected database and app role with an authenticated, read-only SELECT 1 using the existing app secret without printing it. Confirm the expected migration set/checksums and table/sequence counts. Do not run migrations or role initializers during QOS-015C.
- Compare actual worker, provider, daemon, EA source, and EA binary SHA-256 values with one manifest from the exact pinned source revision. A missing path, hash mismatch, stale manifest, or unknown artifact provenance is a blocker.
- Confirm the EA chart-loaded binary only through the terminal's own runtime PING/build marker after a separately approved reload. A file hash on disk does not prove which EX5 the chart loaded.
- Confirm current free disk space and backup state before any later deployment proposal. Do not delete build outputs, Docker images, containers, volumes, logs, or backups to make room.

## Isolation rules

The following must remain separate:

| Concern | v1 development | v2 staging |
|---|---|---|
| Checkout | Existing local development checkout | Immutable, pinned staging checkout |
| Worker/daemon | Existing LaunchAgents and their current config | Separate supervised processes/services |
| Bridge listener | Existing v1 port(s) | Dedicated staging port 19443 |
| EA provider listener | Existing local listener, if retained | Dedicated 18765 only if macOS provider testing is explicitly approved |
| PostgreSQL | Existing v1 database endpoint | 127.0.0.1:15543/mt5_v2_staging |
| Docker project/volume | Development Compose project and its own data | edgenest-mt5-bridge-v2-staging / external edgenest_mt5_bridge_v2_staging_data |
| Credentials | Existing v1 secret store | Separate v2 secret files and certificate identity |

Do not point v2 at the v1 trading database. The daemon has no automatic database isolation; the database URL is the isolation boundary. Do not run two workers against the same MT5 terminal/account identity.

## Deployment decision criteria

### GO to a separately approved deployment plan only when all are true

- One exact source commit and immutable checkout are named.
- The build manifest is regenerated for that commit.
- Worker/provider/daemon/EA source hashes match the manifest.
- The daemon binary is rebuilt from that same source commit and its hash/size recorded.
- The EX5 is accepted by compiler evidence and then by a separately approved runtime PING reporting build 2.00 and provider protocol 1.
- v1 and v2 service configs, ports, account/terminal IDs, certificate identities and database URLs are demonstrably isolated.
- The intended v2 Postgres container is healthy, authenticated as mt5_v2_app, migrated to the reviewed schema, and backed by the exact external volume.
- Backup/restore and disk-capacity checks are complete.
- The deployment/rollback plan has explicit operator approval.

### NO-GO conditions

Any unknown or mismatch above means NO-GO. Do not recreate the existing staging container just to make its Compose labels match the repository file. Do not start, stop, restart, replace, migrate, rotate, delete, prune, or reconfigure anything during QOS-015C.

## Current decision

**NO-GO for deployment; GO only for read-only inventory collection.**

The repository source is inspectable, but the latest host-side process/container/volume/database state has not been collected in this task. The historical manifest is stale relative to current main; the daemon must be rebuilt and all artifacts re-fingerprinted from one pinned revision before v2 can be deployed. The existing v1 runtime and staging database are to remain untouched.

## Acceptance record

QOS-015C can be marked complete only after the operator-generated JSON and reviewed supporting evidence establish every inventory item above, a reviewer signs the exact inventory digest, and the deployment decision is recorded. Until then this task remains IN PROGRESS / HOST EVIDENCE REQUIRED.
