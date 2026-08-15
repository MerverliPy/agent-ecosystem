#!/usr/bin/env bash
# Inject SlopGate quality scores into the skillhub-web snapshot (Phase 7 Task 3).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
SNAP="$ROOT/apps/skillhub-web/data/skills.json"
SLOP="$ROOT/apps/slopgate/src/cli.ts"

quality_of() {
  node --experimental-strip-types "$SLOP" score "$1" --json 2>/dev/null | jq -r '.score'
}

Q=$(jq -n \
  --argjson hello "$(quality_of "$ROOT/apps/skillhub-cli/fixtures/benign-skill")" \
  --argjson exfil "$(quality_of "$ROOT/apps/skillhub-cli/fixtures/exfil-shell-skill")" \
  --argjson inject "$(quality_of "$ROOT/apps/skillhub-cli/fixtures/prompt-inject-skill")" \
  --argjson secret "$(quality_of "$ROOT/apps/skillhub-cli/fixtures/secret-stealer-skill")" \
  '{ "demo/hello-skill": $hello, "malware/exfil-shell": $exfil, "malware/prompt-inject": $inject, "malware/secret-stealer": $secret }')

jq --argjson q "$Q" '(.packages[]) |= (.quality_score = ($q[.name] // null))' "$SNAP" > "$SNAP.q"
mv "$SNAP.q" "$SNAP"
jq -e '.packages[0].quality_score != null' "$SNAP" >/dev/null
echo "quality scores injected into $SNAP"
