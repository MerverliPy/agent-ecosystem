#!/usr/bin/env bash
# sign-artifacts.sh — checksum + optional GPG-sign a directory of release artifacts.
#   bash scripts/sign-artifacts.sh [dir]
# Writes SHA256SUMS (and SHA256SUMS.sig when GPG_KEY_ID is set). Signing keys are secrets
# provided via env (e.g. CI secrets), never committed. Usage: release pipeline, pre-upload.
set -uo pipefail
DIR="${1:-dist/release}"
[ -d "$DIR" ] || { echo "no such dir: $DIR" >&2; exit 1; }
cd "$DIR"

find . -maxdepth 1 -type f ! -name SHA256SUMS ! -name '*.sig' ! -name 'SBOM.json' -print0 \
  | sort -z | xargs -0 sha256sum > SHA256SUMS
echo "wrote $DIR/SHA256SUMS"

if command -v gpg >/dev/null 2>&1 && [ -n "${GPG_KEY_ID:-}" ]; then
  gpg --batch --yes --armor --detach-sign --local-user "$GPG_KEY_ID" SHA256SUMS
  echo "wrote $DIR/SHA256SUMS.sig (GPG key ${GPG_KEY_ID})"
else
  echo "no GPG key configured (GPG_KEY_ID) — SHA256SUMS written unsigned"
fi
