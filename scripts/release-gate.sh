#!/usr/bin/env bash
# release-gate.sh — RELEASE-ONLY gate. This is NOT part of run-all-checks.sh (which remains the
# general CI surrogate); it is run only before publishing a tagged release. It verifies:
#   1. version consistency (VERSION matches every product manifest)   via check-versions.sh
#   2. artifact hygiene (no *.db/.env/keys anywhere)                  via check-artifact-hygiene.sh
#   3. required release artifacts are present in dist/release/
#   4. SHA256SUMS is present and matches the artifacts
#   5. SBOM.json is present
#   6. release artifacts contain no forbidden runtime DB / secret material
# Upgrade path: CLIs support `--version`; re-running install.sh upgrades to the latest release,
# and cargo-binstall can install cargo-dist artifacts (`cargo binstall skillhub`).
# Usage: bash scripts/release-gate.sh [release-dir]   (default dist/release)
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
FAILURES=0
DIR="${1:-dist/release}"

echo "== 1. version consistency =="
bash scripts/check-versions.sh || FAILURES=$((FAILURES+1))

echo "== 2. artifact hygiene =="
bash scripts/check-artifact-hygiene.sh || FAILURES=$((FAILURES+1))

echo "== 3. release artifacts present =="
[ -d "$DIR" ] || { echo "FAIL: release dir missing: $DIR"; FAILURES=$((FAILURES+1)); }
for a in skillhub deskagent; do
  if [ -f "$DIR/$a" ]; then
    echo "  ok   $a"
  else
    echo "  FAIL missing artifact $DIR/$a"
    FAILURES=$((FAILURES+1))
  fi
done

echo "== 4. checksums =="
if [ -f "$DIR/SHA256SUMS" ]; then
  if (cd "$DIR" && sha256sum -c SHA256SUMS >/dev/null 2>&1); then
    echo "  ok   SHA256SUMS verifies"
  else
    echo "  FAIL SHA256SUMS does not match artifacts"
    FAILURES=$((FAILURES+1))
  fi
else
  echo "  FAIL missing $DIR/SHA256SUMS"
  FAILURES=$((FAILURES+1))
fi

echo "== 5. SBOM =="
[ -f "$DIR/SBOM.json" ] && echo "  ok   SBOM.json" \
  || { echo "  FAIL missing $DIR/SBOM.json"; FAILURES=$((FAILURES+1)); }

echo "== 6. no forbidden content in artifacts =="
if [ -d "$DIR" ]; then
  FORBIDDEN="$(find "$DIR" -maxdepth 1 \( -name '*.db' -o -name '*.db-wal' -o -name '*.db-shm' -o -name '*.env' -o -name '*.key' -o -name '*plan.key' \) -print)"
  if [ -n "$FORBIDDEN" ]; then
    echo "  FAIL forbidden content in release artifacts: $FORBIDDEN"
    FAILURES=$((FAILURES+1))
  else
    echo "  ok   no runtime DB / secret material in artifacts"
  fi
fi

if [ "$FAILURES" -ne 0 ]; then
  echo "RELEASE-GATE-FAILED"
  exit 1
fi
echo "RELEASE-GATE-OK"
