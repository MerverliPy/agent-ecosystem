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

echo "== 3. release artifacts present (per-target) =="
[ -d "$DIR" ] || { echo "FAIL: release dir missing: $DIR"; FAILURES=$((FAILURES+1)); }
# the 4 supported targets (linux amd64/arm64 + macOS amd64/arm64), each CLIs named per-target
TARGETS="x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin"
for t in $TARGETS; do
  for bin in skillhub deskagent; do
    if [ -f "$DIR/${bin}-${t}" ]; then
      printf '  ok   %s-%-28s\n' "$bin" "$t"
    else
      echo "  FAIL missing artifact $DIR/${bin}-${t}"
      FAILURES=$((FAILURES+1))
    fi
  done
done

# web static dists + action assembly are also part of the full inventory
for w in bench-site skillhub-web slopgate-dash; do
  [ -d "$DIR/web/$w" ] || { echo "  FAIL missing web dist $DIR/web/$w"; FAILURES=$((FAILURES+1)); }
done
[ -d "$DIR/slopgate-action" ] || { echo "  FAIL missing action assembly $DIR/slopgate-action"; FAILURES=$((FAILURES+1)); }

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

# signature: if a detached signature is present, verify it when gpg + the public key are available
if [ -f "$DIR/SHA256SUMS.sig" ]; then
  if command -v gpg >/dev/null 2>&1 && gpg --verify "$DIR/SHA256SUMS.sig" "$DIR/SHA256SUMS" >/dev/null 2>&1; then
    echo "  ok   SHA256SUMS.sig verifies"
  else
    echo "  WARN SHA256SUMS.sig present but not verified (no public key configured) — signature NOT confirmed"
  fi
else
  echo "  note no SHA256SUMS.sig (unsigned release)"
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
