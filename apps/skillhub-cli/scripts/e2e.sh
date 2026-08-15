#!/usr/bin/env bash
# SkillHub end-to-end: registry + CLI publish / search / install / verify / remove.
# Also writes apps/skillhub-web/data/skills.json (the web snapshot).
# Usage: bash apps/skillhub-cli/scripts/e2e.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CLI_DIR="$ROOT/apps/skillhub-cli"
REG_DIR="$ROOT/apps/skillhub-registry"
CLI="$CLI_DIR/target/debug/skillhub"
REG="$REG_DIR/target/debug/skillhub-registry"
PORT="${SKILLHUB_E2E_PORT:-8791}"
BASE="http://127.0.0.1:$PORT"
TMP="$(mktemp -d)"
HARNESS_DIR="$TMP/harness"
PASS=0
FAIL=0

cleanup() {
  [ -n "${REG_PID:-}" ] && kill "$REG_PID" 2>/dev/null || true
  rm -rf "$TMP"
}
trap cleanup EXIT

step() { echo; echo "== $1"; }
ok()   { PASS=$((PASS+1)); echo "   PASS: $1"; }
bad()  { FAIL=$((FAIL+1)); echo "   FAIL: $1"; }

step "build both crates"
(cd "$CLI_DIR" && cargo build --quiet) || { echo "cli build failed"; exit 1; }
(cd "$REG_DIR" && cargo build --quiet) || { echo "registry build failed"; exit 1; }

step "start registry on :$PORT (temp db)"
SKILLHUB_REGISTRY_PORT="$PORT" SKILLHUB_REGISTRY_DB="$TMP/skillhub.db" "$REG" >"$TMP/reg.log" 2>&1 &
REG_PID=$!
for _ in $(seq 1 50); do
  curl -sf "$BASE/health" >/dev/null 2>&1 && break
  sleep 0.2
done
curl -sf "$BASE/health" >/dev/null 2>&1 && ok "registry healthy" || { bad "registry did not start"; cat "$TMP/reg.log"; exit 1; }

step "publish benign fixture"
out="$("$CLI" publish "$CLI_DIR/fixtures/benign-skill/skillhub.json" --files-dir "$CLI_DIR/fixtures/benign-skill" --registry "$BASE" 2>&1)"
echo "$out"
echo "$out" | grep -q "published demo/hello-skill v1.0.0 (verified: true)" && ok "benign published + verified" || bad "benign publish output unexpected"

step "publish 3 malicious fixtures (must be unverified)"
for f in exfil-shell-skill prompt-inject-skill secret-stealer-skill; do
  out="$("$CLI" publish "$CLI_DIR/fixtures/$f/skillhub.json" --files-dir "$CLI_DIR/fixtures/$f" --registry "$BASE" 2>&1)"
  echo "$out" | tail -1
  echo "$out" | grep -q "verified: false" && ok "$f published unverified" || bad "$f not flagged unverified"
done

step "search finds the benign skill"
"$CLI" search hello --registry "$BASE" | grep -q "demo/hello-skill" && ok "search finds demo/hello-skill" || bad "search missed demo/hello-skill"

step "install benign into temp harness dir"
"$CLI" install demo/hello-skill --dir "$HARNESS_DIR" --registry "$BASE" | tail -1
[ -f "$HARNESS_DIR/demo/hello-skill/SKILL.md" ] && ok "SKILL.md installed" || bad "SKILL.md missing"
[ -f "$HARNESS_DIR/skillhub.lock.json" ] && ok "lockfile written" || bad "lockfile missing"
"$CLI" info demo/hello-skill --registry "$BASE" | grep -q "demo/hello-skill" && ok "info works" || bad "info failed"

step "verify flags the malicious package (expect exit 1)"
set +e
ver="$("$CLI" verify malware/exfil-shell --registry "$BASE" 2>&1)"
rc=$?
set -e
echo "$ver" | head -3
[ "$rc" -ne 0 ] && ok "verify exited non-zero for malicious" || bad "verify should have failed"
echo "$ver" | grep -q "SHELL-02" && ok "exfil-shell flagged SHELL-02" || bad "SHELL-02 not reported"

step "remove the installed skill"
"$CLI" remove demo/hello-skill --dir "$HARNESS_DIR" | tail -1
[ ! -e "$HARNESS_DIR/demo/hello-skill" ] && ok "skill dir removed" || bad "skill dir still present"

step "write web snapshot (apps/skillhub-web/data/skills.json)"
WEB_DATA="$ROOT/apps/skillhub-web/data"
mkdir -p "$WEB_DATA"
curl -s "$BASE/api/search?q=" >"$TMP/pkgs.json"
while read -r n; do
  owner="${n%%/*}"
  pname="${n##*/}"
  curl -s "$BASE/api/packages/$owner/$pname"
done < <(jq -r '.[].name' "$TMP/pkgs.json") >"$TMP/details.jsonl"
jq -n --argjson arr "$(jq -s . "$TMP/details.jsonl")" '{updated_at: (now|todateiso8601), packages: $arr}' >"$WEB_DATA/skills.json"
jq -e '.packages | length >= 4' "$WEB_DATA/skills.json" >/dev/null 2>&1 && ok "snapshot has 4+ packages" || bad "snapshot incomplete"

step "inject SlopGate quality scores into the snapshot (Phase 7 Task 3)"
SLOP="$ROOT/apps/slopgate/src/cli.ts"
quality_of() { # quality_of <fixture-dir> -> score
  node --experimental-strip-types "$SLOP" score "$1" --json 2>/dev/null | jq -r '.score'
}
QUALITY_JSON=$(jq -n \
  --argjson hello "$(quality_of "$ROOT/apps/skillhub-cli/fixtures/benign-skill")" \
  --argjson exfil "$(quality_of "$ROOT/apps/skillhub-cli/fixtures/exfil-shell-skill")" \
  --argjson inject "$(quality_of "$ROOT/apps/skillhub-cli/fixtures/prompt-inject-skill")" \
  --argjson secret "$(quality_of "$ROOT/apps/skillhub-cli/fixtures/secret-stealer-skill")" \
  '{ "demo/hello-skill": $hello, "malware/exfil-shell": $exfil, "malware/prompt-inject": $inject, "malware/secret-stealer": $secret }')
jq --argjson q "$QUALITY_JSON" '(.packages[]) |= (.quality_score = ($q[.name] // null))' "$WEB_DATA/skills.json" >"$TMP/skills-q2.json"
mv "$TMP/skills-q2.json" "$WEB_DATA/skills.json"
jq -e '.packages[0].quality_score != null' "$WEB_DATA/skills.json" >/dev/null 2>&1 && ok "quality scores injected" || bad "quality injection failed"
echo "   snapshot: $WEB_DATA/skills.json"

echo
echo "================================"
echo "E2E RESULT: $PASS passed, $FAIL failed"
echo "================================"
[ "$FAIL" -eq 0 ]
