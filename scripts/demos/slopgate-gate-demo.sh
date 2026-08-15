#!/usr/bin/env bash
# SlopGate PR-gate demo — score the three fixture repos and show the CI gate.
# Usage: bash scripts/demos/slopgate-gate-demo.sh
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SLOP="$ROOT/apps/slopgate/src/cli.ts"

echo "== SlopGate PR-gate demo =="
for name in clean mild heavy; do
  score="$(node --experimental-strip-types "$SLOP" score "$ROOT/apps/slopgate/fixtures/$name" --json | jq -r '.score')"
  echo "fixtures/$name -> slop score $score/100"
done

echo
echo "-- CI gate at threshold 50 (exit codes are the CI signal) --"
for name in clean mild heavy; do
  node --experimental-strip-types "$SLOP" lint "$ROOT/apps/slopgate/fixtures/$name" --threshold 50 >/dev/null 2>&1
  rc=$?
  echo "fixtures/$name: lint exit $rc ($([ "$rc" -eq 0 ] && echo PASS || echo BLOCKED))"
done

echo
echo "Try it yourself:  slop lint <repo> --threshold 50"
echo "GitHub Action:    apps/slopgate-action (posts PR comments, writes SARIF)"
