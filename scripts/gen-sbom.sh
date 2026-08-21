#!/usr/bin/env bash
# gen-sbom.sh — emit a lightweight SPDX-ish SBOM listing release artifacts (name, sha256,
# version) from a directory of artifacts + the canonical VERSION. Signing-key / secret material
# is never included. Usage: bash scripts/gen-sbom.sh [dir]
set -uo pipefail
DIR="${1:-dist/release}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VER="$(tr -d '[:space:]' < VERSION)"
[ -d "$DIR" ] || { echo "no such dir: $DIR" >&2; exit 1; }

out="$DIR/SBOM.json"
{
  echo "{"
  echo "  \"bomFormat\": \"SPDX\","
  echo "  \"spdxVersion\": \"SPDX-2.3\","
  echo "  \"name\": \"agent-ecosystem-release\","
  echo "  \"version\": \"$VER\","
  echo "  \"packages\": ["
  first=1
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    file="${line#*  }"
    hash="${line%%  *}"
    [ "$file" = "SBOM.json" ] && continue
    if [ "$first" -eq 1 ]; then first=0; else echo "    ,"; fi
    printf '    {"name": "%s", "version": "%s", "sha256": "%s"}' "$file" "$VER" "$hash"
  done < <(sha256sum "$DIR"/* 2>/dev/null | grep -v 'SHA256SUMS\|SBOM.json')
  echo ""
  echo "  ]"
  echo "}"
} > "$out"
echo "wrote $out"
