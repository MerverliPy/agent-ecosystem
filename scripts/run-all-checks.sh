#!/usr/bin/env bash
# run-all-checks.sh — every product's test suite, lints, schema/dataset validation,
# and builds, in dependency order. Single exit code: 0 = everything green.
# Usage: bash scripts/run-all-checks.sh  (from the repo root)
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FAILURES=0
PASSES=0

note()  { printf '%-58s' "$1"; }
ok()    { printf 'OK (%s)\n' "${2:-done}"; PASSES=$((PASSES+1)); }
fail()  { printf 'FAIL (%s)\n' "${2:-failed}"; FAILURES=$((FAILURES+1)); }
run()   { # run <label> <cmd...>
  local label="$1"; shift
  note "$label"
  if "$@" >/tmp/run-all-checks.$$.log 2>&1; then
    ok "$(tail -1 /tmp/run-all-checks.$$.log | tr -d '\n')"
  else
    fail "see /tmp/run-all-checks.$$.log (kept for the failing step)"
    cp /tmp/run-all-checks.$$.log "/tmp/run-all-checks-failed.$$.log"
    tail -20 "/tmp/run-all-checks-failed.$$.log"
  fi
  rm -f /tmp/run-all-checks.$$.log
}

echo "== shared =="
run "artifact hygiene guard"                 bash "$ROOT/scripts/check-artifact-hygiene.sh"
run "memory-event schema tests"             node --test "$ROOT/shared/schemas/test/memory-event.test.mjs"
run "benchmark dataset validation"          node "$ROOT/shared/datasets/validate-dataset.mjs"
run "will-it-run calculator tests"          node --test "$ROOT/shared/lib/test/will-it-run.test.mjs"

echo "== slopgate =="
run "slopgate suite (root npm test)"        npm test --prefix "$ROOT"
run "slopgate typecheck"                    bash -c "cd '$ROOT/apps/slopgate' && npx tsc --noEmit -p tsconfig.json"

echo "== slopgate-action =="
run "slopgate-action tests"                 npm test --prefix "$ROOT/apps/slopgate-action"
run "slopgate-action build"                 npm run build --prefix "$ROOT/apps/slopgate-action"

echo "== slopgate-dash =="
run "slopgate-dash tests"                   npm test --prefix "$ROOT/apps/slopgate-dash"
run "slopgate-dash build"                   npm run build --prefix "$ROOT/apps/slopgate-dash"

echo "== bench-site =="
run "bench-site tests"                      npm test --prefix "$ROOT/apps/bench-site"
run "bench-site build"                      npm run build --prefix "$ROOT/apps/bench-site"

echo "== skillhub =="
run "skillhub-cli tests"                    bash -c "cd '$ROOT/apps/skillhub-cli' && cargo test --quiet"
run "skillhub-registry tests"               bash -c "cd '$ROOT/apps/skillhub-registry' && cargo test --quiet"
run "skillhub-web tests"                    npm test --prefix "$ROOT/apps/skillhub-web"
run "skillhub-web build"                    npm run build --prefix "$ROOT/apps/skillhub-web"

echo "== deskagent =="
run "deskagent frontend tests"              npm test --prefix "$ROOT/apps/deskagent"
run "deskagent cargo tests"                 bash -c "cd '$ROOT/apps/deskagent' && cargo test --quiet"
run "deskagent-cli build"                   bash -c "cd '$ROOT/apps/deskagent' && cargo build -p deskagent-cli --quiet"
run "deskagent cargo check (Tauri shell)"   bash -c "cd '$ROOT/apps/deskagent' && cargo check --quiet"
run "deskagent frontend build"              npm run build --prefix "$ROOT/apps/deskagent"

echo
echo "== summary =="
echo "passed: $PASSES   failed: $FAILURES"
[ "$FAILURES" -eq 0 ] && echo "RUN-ALL-CHECKS-OK" || { echo "RUN-ALL-CHECKS-FAILED"; exit 1; }
