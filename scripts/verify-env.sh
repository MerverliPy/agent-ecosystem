#!/usr/bin/env bash
# Verify the toolchains the plan requires. Exit 0 if all present, non-zero otherwise.
set -uo pipefail

missing=0
check() {
  local name="$1"; shift
  if command -v "$1" >/dev/null 2>&1; then
    printf 'ok     %-12s ' "$name"
    "$@" --version 2>&1 | head -n1
  else
    printf 'MISSING %-12s %s\n' "$name" "$1"
    missing=1
  fi
}

check git git
check jq jq
check openssl openssl
check node node
check npm npm
check cargo cargo
check rustc rustc

# Desktop app toolchain (Phase 5) — warn only, not fatal for early phases.
for t in rustup tauri; do
  command -v "$t" >/dev/null 2>&1 || echo "warn   (optional until Phase 5): $t not found"
done

if [ "$missing" -eq 1 ]; then
  echo "ENV-FAIL: install missing tools above before continuing." >&2
  exit 1
fi
echo "ENV-OK: required toolchains present."
