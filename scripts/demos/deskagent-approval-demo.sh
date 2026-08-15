#!/usr/bin/env bash
# DeskAgent demo — skill install + memory approval flow, driven by the core's own
# tests (deterministic, offline). Shows: capture → extract → propose → approve →
# retrieve-with-citation, plus the risky-action sandbox and shared undo log.
# Usage: bash scripts/demos/deskagent-approval-demo.sh
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MANIFEST="$ROOT/apps/deskagent/Cargo.toml"

echo "== DeskAgent demo: skill install + approval flow =="

echo
echo "-- capture pipeline + extraction pass (fixture conversation) --"
cargo test --manifest-path "$MANIFEST" -p deskagent-core capture_stores_episodes_and_extracts_proposals --quiet 2>&1 | tail -2

echo "-- approvals: propose → approve (+0.1) / reject (−0.1) + shared undo log --"
cargo test --manifest-path "$MANIFEST" -p deskagent-core approve_applies_learning_signal --quiet 2>&1 | tail -2

echo "-- action sandbox: risky shell/network blocked until click-to-approve; undo log --"
cargo test --manifest-path "$MANIFEST" -p deskagent-core risky_action_lifecycle_with_undo --quiet 2>&1 | tail -2

echo "-- skill install from a SkillHub registry → procedural memory proposal --"
cargo test --manifest-path "$MANIFEST" -p deskagent-core installs_skill_and_creates_memory_proposal --quiet 2>&1 | tail -2

echo "-- memory into conversation: persona + scoped retrieval + citations --"
cargo test --manifest-path "$MANIFEST" -p deskagent-core context_includes_persona_memories_and_history --quiet 2>&1 | tail -2

echo
echo "DeskAgent demo done. Run the live model path with:"
echo "  cargo test -p deskagent-core -- --ignored ollama_live   (requires a running Ollama)"
