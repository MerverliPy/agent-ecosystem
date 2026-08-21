#!/usr/bin/env bash
# check-artifact-hygiene.sh — ensure runtime DBs, seed tokens, and signing secrets stay out of
# git and out of any build/release artifact. Single exit code: 0 = clean.
# Usage: bash scripts/check-artifact-hygiene.sh   (from the repo root)
#
# This is the guard Phase 9/10 rely on: a build step must never copy `*.db`, `.env`, or key
# material into a release artifact. It verifies (1) nothing forbidden is git-tracked, (2) any
# forbidden file present in the tree is git-ignored (so commits/archives skip it), and (3) no
# private-key material or seed tokens leak into tracked source.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
FAILURES=0

fail() { printf 'FAIL: %s\n' "$1"; FAILURES=$((FAILURES+1)); }

# forbidden filename patterns (gitignore already covers these; this asserts the invariant)
FORBIDDEN='(^|/)([^/]*\.db|.*\.db-wal|.*\.db-shm|[^/]*\.env|\.env|[^/]*\.key|.*plan\.key)$'

# 1) No forbidden artifacts may be tracked by git
TRACKED="$(git ls-files | grep -E "$FORBIDDEN" || true)"
if [ -n "$TRACKED" ]; then
  fail "forbidden artifacts are tracked by git:"
  echo "$TRACKED" | sed 's/^/    /'
fi

# 2) Forbidden files present in the working tree must be git-ignored, so a commit, archive, or
#    container build can never pick them up (this is the no-build-step-copies guard).
while IFS= read -r f; do
  [ -z "$f" ] && continue
  if ! git check-ignore -q "$f"; then
    fail "forbidden artifact is NOT git-ignored (would enter artifacts): $f"
  fi
done < <(find . \
  \( -path ./.git -o -path ./target -o -path '*/target' -o -path ./node_modules -o -path '*/node_modules' \) -prune -o \
  -type f \( -name '*.db' -o -name '*.db-wal' -o -name '*.db-shm' -o -name '*.env' -o -name '.env' -o -name '*.key' -o -name '*plan.key' \) -print)

# 3) No private-key material in tracked source (exclude the SkillHub scanner's own SEC-06 rule regex)
if git grep -nE -e '-----BEGIN (RSA |EC |OPENSSH |DSA |PGP )?PRIVATE KEY-----' \
    -- ':!apps/skillhub-cli/src/scan.rs' | grep -q .; then
  fail "private-key material found in tracked files"
fi

# 4) No obvious seed tokens / env secrets committed (SKILLHUB_REGISTRY_SECRET=..., <secret> literals)
if git grep -nE -e 'SKILLHUB_REGISTRY_SECRET=[A-Za-z0-9]|SKILLHUB_SIGNING_KEY=[A-Za-z0-9]' | grep -q .; then
  fail "hardcoded registry secret/signing-key literal found in tracked files"
fi

if [ "$FAILURES" -ne 0 ]; then
  echo "ARTIFACT-HYGIENE-FAILED"
  exit 1
fi
echo "ARTIFACT-HYGIENE-OK"
