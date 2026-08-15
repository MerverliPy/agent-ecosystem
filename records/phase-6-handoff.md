# Phase 6 Handoff — DeskAgent: runtime, skills, sandbox

## Completion state

- Phase status: COMPLETE
- Tasks: 6/6 completed
- Phase validated: `bash scripts/plan-lock.sh verify` (exit 0) · `cd apps/deskagent && npm test` (18/18) · `cargo check` (exit 0)
- Checkpoint tag: `phase-6-start` (deleted after completion)

## FILES CHANGED

- `apps/deskagent/src-tauri/crates/deskagent-core/src/runtime/` — mod.rs (Backend trait, RuntimeError, test_server), ollama.rs, llama_cpp.rs, registry.rs
- `apps/deskagent/src-tauri/crates/deskagent-core/src/skills.rs` — SkillHub registry client + lockfile + procedural-memory surfacing
- `apps/deskagent/src-tauri/crates/deskagent-core/src/sandbox.rs` — risky-action approval cards + shared undo log
- `apps/deskagent/src-tauri/crates/deskagent-core/src/conversation.rs` — persona + retrieval context + citations
- `apps/deskagent/src-tauri/crates/deskagent-core/src/store.rs` — actions + undo_log tables
- `apps/deskagent/src-tauri/crates/deskagent-core/src/approvals.rs` — approved memory writes now record undo entries
- `apps/deskagent/src-tauri/crates/deskagent-core/src/lib.rs` — module + re-exports
- `apps/deskagent/src-tauri/crates/deskagent-core/Cargo.toml` — +ureq (json)
- `apps/deskagent/src-tauri/src/lib.rs` — 11 new commands (runtime_list_models, runtime_pick, chat_complete, skill_install/list/remove, action_propose/decide/list, undo_list/revert)
- `apps/deskagent/src/lib/` — picker.ts, benchkit-catalog.ts (generated), tasks.ts
- `apps/deskagent/src/components/` — ModelPicker.tsx, TasksPanel.tsx; ChatWindow.tsx (mic stub); App.tsx (Models/Tasks tabs)
- `apps/deskagent/src/styles.css`, `apps/deskagent/test/{picker,tasks}.test.ts`, `apps/deskagent/scripts/sync-catalog.mjs`
- `shared/lib/will-it-run.d.mts` — ambient types for the shared calculator
- `PHASES.md` — Phase 6 status → COMPLETE, 6 checkboxes (status-only; lock hash unchanged)
- `PROGRESS.md` — Phase 6 record

## VALIDATIONS ACTUALLY RUN

| Command | Exit |
|---|---|
| `bash scripts/plan-lock.sh verify` (pre/post-task, post-phase) | 0 |
| `cd apps/deskagent && npm test` (Phase VALIDATE) | 0 (18/18) |
| `cargo check` (workspace incl. Tauri shell) | 0 |
| `cargo test` (workspace) | 0 (53/53, 1 ignored live) |
| `cargo test -p deskagent-core -- --ignored ollama_live --nocapture` | 0 — **real local Ollama chat**: qwen2.5-coder:7b → "Hello! How can I help you today?" |
| `npm run build` (tsc --noEmit + vite build) | 0 |

## ACTUAL EXIT CODES

All validations as above. Fixes during execution: ureq needs the `json` feature for `into_json`/`send_json`; fastembed-free runtime (ureq chosen over reqwest to keep the core blocking/light); `BackendKind` re-export; catalog generator copied hardware incl. `os`; ambient declaration moved beside the module (relative module declarations don't apply).

## CI RESULTS

No CI workflows yet (Phase 7). Local validation only.

## UNRESOLVED GATES / FOLLOW-UPS

- **Skill invoke**: install/update/remove work and skills surface as procedural memory; *invoking* a skill (running its SKILL.md instructions) is the app layer's job — the runtime/skill bridge (tool → approval card → execute) is a Phase 6+/v2 item. The action sandbox provides the approval + undo substrate for it.
- **llama.cpp adapter** tested against a mock server only (no llama.cpp server running here); the Ollama path is live-verified. Metal (macOS) is a documented configuration hint, not exercised on Apple hardware.
- **Voice**: mic stub only (placeholder transcript); Whisper/WebRTC transcription is a follow-up as the task allows.
- **Scheduled tasks**: local placeholder with due/roll/done logic; no runner/executor yet.
- **Model picker**: fit estimates use the row's measured peak RAM when present, else will-it-run at the row's quant tier (dataset rows carry no param counts); hardware profile is a fixed 16 GB default (editable in code; user UI is a polish item).
- chat_complete falls back to a deterministic reply when the runtime is offline (DEC-0005) — the fallback text notes the runtime is offline.

## EXACT NEXT ACTION

Phase 7, Task 1: Create `scripts/run-all-checks.sh`: runs every product's test suite, lints, and schema/dataset validation; single exit code.

## MILESTONE ACCEPTANCE CLAIMED: NO
