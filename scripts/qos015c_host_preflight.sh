#!/usr/bin/env bash
#
# QOS-015C read-only host preflight.
#
# Safe to invoke from zsh or bash:
#   bash scripts/qos015c_host_preflight.sh
#
# This script does not start/stop/recreate containers, modify volumes,
# run SQL, run migrations, change files, or print container environment values.
#
set -u
set -o pipefail

PROJECT="edgenest-mt5-bridge-v2-staging"
VOLUME="edgenest_mt5_bridge_v2_staging_data"
PORTS=(15432 15433 15543 18765 19443 8765 9443)

section() {
  printf '\n========== %s ==========\n' "$1"
}

if ! command -v docker >/dev/null 2>&1; then
  section "DOCKER CLI"
  echo "BLOCKED: docker executable is not on PATH."
  exit 2
fi

section "DOCKER CLIENT AND DAEMON"
docker --version || true
if docker info --format 'Server={{.ServerVersion}} OSType={{.OSType}} Architecture={{.Architecture}}' 2>&1; then
  echo "Docker daemon query completed."
else
  echo "BLOCKED: Docker daemon is unreachable from the selected context."
fi
printf 'Context='
docker context show 2>&1 || true

section "HOST DISK CAPACITY"
df -h / 2>&1 || true

section "EXPECTED STAGING PROJECT CONTAINERS"
docker ps -a \
  --filter "label=com.docker.compose.project=$PROJECT" \
  --format 'table {{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}' 2>&1 || true

section "STAGING CONTAINER PROVENANCE AND MOUNTS"
IDS="$(docker ps -aq --filter "label=com.docker.compose.project=$PROJECT" 2>/dev/null || true)"
if [ -n "$IDS" ]; then
  while IFS= read -r ID; do
    [ -z "$ID" ] && continue
    docker inspect --format \
'Name={{.Name}}
Image={{.Config.Image}}
State={{.State.Status}}
Health={{if .State.Health}}{{.State.Health.Status}}{{else}}no-healthcheck{{end}}
ComposeProject={{index .Config.Labels "com.docker.compose.project"}}
ComposeService={{index .Config.Labels "com.docker.compose.service"}}
ComposeConfig={{index .Config.Labels "com.docker.compose.project.config_files"}}
ComposeWorkingDir={{index .Config.Labels "com.docker.compose.project.working_dir"}}
Mounts={{json .Mounts}}
Ports={{json .NetworkSettings.Ports}}
---' "$ID" 2>&1 || true
  done <<< "$IDS"
else
  echo "No containers currently carry the expected Compose project label."
fi

section "EXPECTED EXTERNAL POSTGRES VOLUME"
if docker volume inspect "$VOLUME" >/dev/null 2>&1; then
  docker volume inspect "$VOLUME" --format \
'Name={{.Name}}
Driver={{.Driver}}
Scope={{.Scope}}
Mountpoint={{.Mountpoint}}
Labels={{json .Labels}}' 2>&1 || true
else
  echo "NOT FOUND: $VOLUME"
fi

section "ALL POSTGRES / MT5 / BRIDGE CONTAINERS"
docker ps -a \
  --format 'table {{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}' \
  | grep -Ei 'postgres|mt5|bridge|NAMES' || true

section "HOST TCP LISTENERS"
for PORT in "${PORTS[@]}"; do
  printf '\n--- Port %s ---\n' "$PORT"
  if command -v lsof >/dev/null 2>&1; then
    lsof -nP -iTCP:"$PORT" -sTCP:LISTEN 2>/dev/null || echo "No host listener reported."
  else
    echo "lsof is unavailable; listener check skipped."
  fi
done

section "PREFLIGHT COMPLETE"
echo "Read-only inspection only. No Docker resources, volumes, databases, or repository files were changed."
