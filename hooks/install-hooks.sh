#!/usr/bin/env bash
# Install the plan-lock git hooks into .git/hooks/
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
for hook in pre-commit pre-push; do
  src="$ROOT/hooks/$hook"
  dst="$ROOT/.git/hooks/$hook"
  if [ -f "$dst" ]; then
    echo "skip: $hook already installed ($dst)"
  else
    cp "$src" "$dst"
    chmod +x "$dst"
    echo "installed: $dst"
  fi
done
echo "hooks installed. Verify with: bash scripts/plan-lock.sh verify"
