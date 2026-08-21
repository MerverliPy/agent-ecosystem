#!/usr/bin/env bash
# SkillHub demo — install a skill from the registry into a temp harness dir.
# Usage: bash scripts/demos/skillhub-install-demo.sh
# (starts the registry on an ephemeral port, publishes the benign fixture, installs it)
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CLI="$ROOT/apps/skillhub-cli/target/debug/skillhub"
REG="$ROOT/apps/skillhub-registry/target/debug/skillhub-registry"
PORT="${SKILLHUB_DEMO_PORT:-8792}"
TMP="$(mktemp -d)"
HARNESS="$TMP/harness"

echo "== SkillHub install-from-registry demo =="
SKILLHUB_REGISTRY_PORT="$PORT" SKILLHUB_REGISTRY_DB="$TMP/registry.db" "$REG" >"$TMP/reg.log" 2>&1 &
REG_PID=$!
trap 'kill $REG_PID 2>/dev/null || true; rm -rf "$TMP"' EXIT
for i in $(seq 1 40); do
  curl -sf "http://127.0.0.1:$PORT/health" >/dev/null 2>&1 && break
  sleep 0.25
done
curl -sf "http://127.0.0.1:$PORT/health" >/dev/null 2>&1 || { echo "registry did not start"; cat "$TMP/reg.log"; exit 1; }

echo "0) register owner 'demo' and mint its publish capability token"
DEMO_TOKEN="$(curl -sf -X POST "http://127.0.0.1:$PORT/api/owners/register" \
  -H 'content-type: application/json' -d '{"owner":"demo"}' | jq -r .token)"
export SKILLHUB_TOKEN="$DEMO_TOKEN"

echo "1) publish the benign fixture (runs the security scanner first)"
"$CLI" publish "$ROOT/apps/skillhub-cli/fixtures/benign-skill/skillhub.json" \
  --files-dir "$ROOT/apps/skillhub-cli/fixtures/benign-skill" --registry "http://127.0.0.1:$PORT"

echo "2) search"
"$CLI" search hello --registry "http://127.0.0.1:$PORT"

echo "3) install into a temp pi harness dir"
mkdir -p "$HARNESS"
"$CLI" install demo/hello-skill --harness pi --dir "$HARNESS" --registry "http://127.0.0.1:$PORT"

echo "4) installed files:"
find "$HARNESS" -type f | sed "s|$HARNESS/|   |"

echo "5) verify --quality (security + SlopGate quality score)"
"$CLI" verify demo/hello-skill --registry "http://127.0.0.1:$PORT" --quality

echo
echo "SkillHub demo done (temp harness at $HARNESS — removed on exit)."
