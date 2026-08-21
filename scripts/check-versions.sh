#!/usr/bin/env bash
# check-versions.sh — unified versioning: assert every product manifest matches the canonical
# `VERSION` file. This is the single semver source per product; release-gate.sh calls it so a
# tagged release can't ship with a manifest that drifted from the tag.
# Usage: bash scripts/check-versions.sh   (from the repo root)
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
FAILURES=0

VER="$(tr -d '[:space:]' < VERSION)"
[ -n "$VER" ] || { echo "FAIL: VERSION file is empty"; FAILURES=1; }
echo "canonical VERSION: $VER"

expect() { # expect <label> <actual>
  if [ "$2" = "$VER" ]; then
    printf '  ok   %-28s %s\n' "$1" "$2"
  else
    printf '  FAIL %-28s %s (want %s)\n' "$1" "$2" "$VER"
    FAILURES=1
  fi
}

cargo_ver() { sed -n 's/^version = "\(.*\)"/\1/p' "$1" | head -1; }
json_ver()  { sed -n 's/^  "version": "\(.*\)",/\1/p' "$1" | head -1; }
tauri_ver() { sed -n 's/^[[:space:]]*"version": "\([^"]*\)".*/\1/p' "$1" | head -1; }

echo "== Rust (Cargo) =="
expect "skillhub-cli"                 "$(cargo_ver apps/skillhub-cli/Cargo.toml)"
expect "skillhub-registry"            "$(cargo_ver apps/skillhub-registry/Cargo.toml)"
expect "deskagent workspace"          "$(cargo_ver apps/deskagent/Cargo.toml)"
expect "deskagent tauri.conf"         "$(tauri_ver apps/deskagent/src-tauri/tauri.conf.json)"

echo "== Node (package.json) =="
for p in slopgate slopgate-action slopgate-dash bench-site skillhub-web deskagent; do
  expect "$p" "$(json_ver "apps/$p/package.json")"
done
expect "root" "$(json_ver package.json)"

# tag-to-version consistency: ONLY when the run is triggered by a release tag (GITHUB_REF_TYPE
# == tag). A manually-dispatched run on a branch (e.g. main) must not be subject to the tag check.
# The tag must equal "v$VER", so a wrongly named tag can't publish a version mismatch.
if [ "${GITHUB_REF_TYPE:-}" = "tag" ]; then
  if [ "$GITHUB_REF_NAME" = "v$VER" ]; then
    echo "  ok   tag $GITHUB_REF_NAME matches v$VER"
  else
    echo "  FAIL tag $GITHUB_REF_NAME does not match v$VER"
    FAILURES=1
  fi
fi

if [ "$FAILURES" -ne 0 ]; then
  echo "VERSION-CHECK-FAILED"
  exit 1
fi
echo "VERSION-CHECK-OK"
