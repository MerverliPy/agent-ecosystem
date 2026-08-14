#!/usr/bin/env bash
# PLAN LOCK — content immutability guard for PHASES.md
#
# Commands:
#   verify          Check PHASES.md content hash against PLAN.lock (exit 0/1). Safe for agents.
#   status          Show lock metadata, verify result, and open change requests. Safe for agents.
#   propose <reason> Append a change request to PROGRESS.md. Safe for agents (their ONLY channel).
#   init            Bootstrap: generate approval token, write PLAN.lock. Requires PLAN_BOOTSTRAP=1 or TTY.
#   approve <reason> Re-lock after an approved content change. Requires token + interactive TTY.
#   hash-stdin      Print normalized content hash of PHASES.md from stdin (used by hooks).
#   check-staged    Git pre-commit guard: block PHASES.md/PLAN.lock commits without valid token.
#   check-push <sha> Git pre-push guard: same check against a commit.
#
# Policy (DEC-0003/DEC-0008):
#   - Agents may update checkboxes and phase status comments in PHASES.md (normalized away before hashing).
#   - Agents must NOT change any other PHASES.md content, must NOT touch PLAN.lock, must NOT run init/approve,
#     must NOT read the key file, and must NOT export PLAN_APPROVAL_TOKEN.
#   - Content changes require: human edits PHASES.md -> human runs `plan-lock.sh approve "<reason>"`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLAN="$ROOT/PHASES.md"
LOCK="$ROOT/PLAN.lock"
PROGRESS="$ROOT/PROGRESS.md"
KEYFILE="${PLAN_KEY_FILE:-$HOME/.config/agent-ecosystem/plan.key}"

fail() { echo "LOCK-FAIL: $*" >&2; exit 1; }
info() { echo "LOCK: $*"; }

[ -x "$(command -v jq)" ] || fail "jq is required"
[ -x "$(command -v sha256sum)" ] || fail "sha256sum is required"

normalize() {
  # Canonicalize: checkbox markers -> plain, strip status/annotation comments, strip trailing spaces.
  sed -E \
    -e 's/^(\s*)- \[[ x\/-]\] /\1- [ ] /' \
    -e 's/<!--[[:space:]]*(PENDING|IN_PROGRESS|COMPLETE|BLOCKED)[[:space:]]*-->//g' \
    -e 's/<!--[[:space:]]*PHASE_TIME: [0-9]+s[[:space:]]*-->//g' \
    -e 's/<!--[[:space:]]*TIME: [0-9]+s[[:space:]]*-->//g' \
    -e 's/[[:space:]]+$//' \
    "$1"
}

content_hash() { normalize "$1" | sha256sum | cut -d' ' -f1; }
token_hash()  { printf '%s' "$1" | sha256sum | cut -d' ' -f1; }

have_tty() { [ -t 0 ] && [ -t 1 ]; }

lock_read() { jq -r "$1" "$LOCK"; }

# Token from environment ONLY (used by git hooks). Agents must never set this.
require_token_env() {
  local token="${PLAN_APPROVAL_TOKEN:-}"
  [ -z "$token" ] && fail "approval token missing: PLAN_APPROVAL_TOKEN must be set for plan-content commits"
  local expected actual
  expected="$(lock_read '.token_sha256 // ""')"
  [ -n "$expected" ] || fail "PLAN.lock has no token_sha256"
  actual="$(token_hash "$token")"
  [ "$actual" = "$expected" ] || fail "approval token mismatch — commit blocked"
}

# Token from env OR key file (used by approve). Agents must never use the key file.
require_token_any() {
  local token="${PLAN_APPROVAL_TOKEN:-}"
  if [ -z "$token" ] && [ -f "$KEYFILE" ]; then
    token="$(tr -d '\n' < "$KEYFILE")"
  fi
  [ -z "$token" ] && fail "approval token required (set PLAN_APPROVAL_TOKEN or create $KEYFILE)"
  local expected actual
  expected="$(lock_read '.token_sha256 // ""')"
  [ -n "$expected" ] || fail "PLAN.lock has no token_sha256"
  actual="$(token_hash "$token")"
  [ "$actual" = "$expected" ] || fail "approval token mismatch"
}

cmd_verify() {
  [ -f "$LOCK" ] || fail "plan is not locked (no PLAN.lock). Run init."
  [ -f "$PLAN" ] || fail "PHASES.md missing"
  local locked current
  locked="$(lock_read '.content_sha256')"
  current="$(content_hash "$PLAN")"
  if [ "$locked" = "$current" ]; then
    info "verify OK — PHASES.md content matches locked baseline $(printf '%.12s' "$locked")"
    exit 0
  fi
  fail "CONTENT DRIFT — PHASES.md hash $current does not match locked $locked. Stop work. Do not modify PLAN.lock. Run 'plan-lock.sh status' then 'propose' a change for review."
}

cmd_status() {
  [ -f "$LOCK" ] || { info "plan is not locked yet"; exit 0; }
  info "locked_at:    $(lock_read '.created_at')"
  info "approved_by:  $(lock_read '.approved_by')"
  info "content_sha256: $(lock_read '.content_sha256')"
  info "last change:  $(lock_read '.history[-1].reason // "n/a"') ($(lock_read '.history[-1].at // ""'))"
  echo "--- verify ---"
  cmd_verify || true
  echo "--- open change requests (PROGRESS.md) ---"
  if [ -f "$PROGRESS" ]; then
    grep -n "REQUEST_OPEN" "$PROGRESS" || echo "(none)"
  else
    echo "(PROGRESS.md missing)"
  fi
}

cmd_propose() {
  local reason="${1:-}"
  [ -n "$reason" ] || fail "usage: plan-lock.sh propose \"<reason>\""
  [ -f "$PROGRESS" ] || fail "PROGRESS.md missing"
  {
    echo ""
    echo "## CHANGE REQUEST <!-- REQUEST_OPEN -->"
    echo "- Proposed: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "- Reason: $reason"
    echo "- Status: pending human review. On approval: human edits PHASES.md, then runs 'scripts/plan-lock.sh approve \"$reason\"'."
  } >> "$PROGRESS"
  info "change request appended to PROGRESS.md"
}

cmd_init() {
  [ -f "$LOCK" ] && fail "already locked. Use approve after an approved content change."
  if ! have_tty && [ "${PLAN_BOOTSTRAP:-}" != "1" ]; then
    fail "init requires an interactive TTY or PLAN_BOOTSTRAP=1 set by the human. Agents must never run init."
  fi
  local token
  token="$(openssl rand -hex 24 2>/dev/null || od -An -N24 -tx1 /dev/urandom | tr -d ' \n')"
  [ -n "$token" ] || fail "failed to generate token"
  mkdir -p "$(dirname "$KEYFILE")"
  umask 077
  printf '%s\n' "$token" > "$KEYFILE"
  local hash
  hash="$(content_hash "$PLAN")"
  {
    echo "{"
    echo "  \"version\": 1,"
    echo "  \"policy\": \"content-locked\","
    echo "  \"created_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
    echo "  \"approved_by\": \"human\","
    echo "  \"content_sha256\": \"$hash\","
    echo "  \"token_sha256\": \"$(token_hash "$token")\","
    echo "  \"history\": ["
    echo "    {\"action\": \"init\", \"at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\", \"reason\": \"initial plan approval\"}"
    echo "  ]"
    echo "}"
  } > "$LOCK"
  info "locked. content_sha256=$hash"
  info "key file written: $KEYFILE (chmod 600)"
  info "NEXT STEP (human): add to your shell rc so executing agents never see it:"
  info "  export PLAN_APPROVAL_TOKEN='$token'"
  info "  (or rely on the key file for approve; git hooks require the env var)"
}

cmd_approve() {
  local reason="${1:-}"
  [ -n "$reason" ] || fail "usage: plan-lock.sh approve \"<reason>\""
  [ -f "$LOCK" ] || fail "not locked — run init first"
  require_token_any
  if ! have_tty; then
    fail "approve requires an interactive TTY — the human must run this. Agents cannot approve plan changes."
  fi
  echo "You are about to re-lock PHASES.md after this approved change:"
  echo "  $reason"
  read -r -p "Type APPROVE to confirm: " confirm
  [ "$confirm" = "APPROVE" ] || fail "confirmation mismatch — no change recorded"
  local hash
  hash="$(content_hash "$PLAN")"
  jq --arg h "$hash" --arg at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg r "$reason" \
     '.content_sha256 = $h | .history += [{"action": "approve", "at": $at, "reason": $r}]' \
     "$LOCK" > "$LOCK.tmp" && mv "$LOCK.tmp" "$LOCK"
  info "re-locked. content_sha256=$hash (reason: $reason)"
}

cmd_hash_stdin() {
  local tmp
  tmp="$(mktemp)"
  trap 'rm -f "$tmp"' EXIT
  cat > "$tmp"
  content_hash "$tmp"
}

cmd_check_staged() {
  # Git pre-commit: allow if neither file staged; allow bootstrap commit; allow status-only changes;
  # otherwise require a valid approval token from the environment.
  if ! git diff --cached --name-only | grep -qE '^(PHASES\.md|PLAN\.lock)$'; then
    exit 0
  fi
  if ! git cat-file -e HEAD:PLAN.lock 2>/dev/null; then
    info "bootstrap commit (PLAN.lock not yet in HEAD) — allowing"
    exit 0
  fi
  if git diff --cached --name-only | grep -qx 'PLAN.lock'; then
    require_token_env
  fi
  if git diff --cached --name-only | grep -qx 'PHASES.md'; then
    local staged_hash locked
    staged_hash="$(git show :PHASES.md | "$0" hash-stdin)"
    locked="$(lock_read '.content_sha256')"
    if [ "$staged_hash" != "$locked" ]; then
      require_token_env
    fi
  fi
  info "staged PHASES.md/PLAN.lock changes are within lock policy"
}

cmd_check_push() {
  # Git pre-push: verify the pushed commit's PHASES.md matches its own PLAN.lock (no content drift pushed).
  local sha="${1:-}"
  [ -n "$sha" ] || fail "usage: check-push <sha>"
  if ! git cat-file -e "$sha:PLAN.lock" 2>/dev/null; then
    info "push contains bootstrap (no PLAN.lock at $sha) — allowing"
    exit 0
  fi
  local pushed_plan pushed_lock locked
  pushed_plan="$(git show "$sha:PHASES.md" | "$0" hash-stdin)"
  pushed_lock="$(git show "$sha:PLAN.lock" | jq -r '.content_sha256')"
  if [ "$pushed_plan" != "$pushed_lock" ]; then
    require_token_env
  fi
  info "pushed plan state is consistent with its lock"
}

cmd="${1:-}"
case "$cmd" in
  verify)      cmd_verify ;;
  status)      cmd_status ;;
  propose)     shift; cmd_propose "${1:-}" ;;
  init)        shift; cmd_init ;;
  approve)     shift; cmd_approve "${1:-}" ;;
  hash-stdin)  cmd_hash_stdin ;;
  check-staged) cmd_check_staged ;;
  check-push)  shift; cmd_check_push "${1:-}" ;;
  *)
    echo "usage: plan-lock.sh {verify|status|propose|init|approve|hash-stdin|check-staged|check-push}" >&2
    exit 2 ;;
esac
