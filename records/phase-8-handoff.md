# Phase 8 Handoff — DeskAgent CLI (terminal UI)

## Completion state

- Phase status: COMPLETE
- Tasks: 8/8 completed
- Workspace: `apps/deskagent/src-tauri/crates/deskagent-cli` (third member, binary `deskagent`)
- Exit criteria: **all met** — CLI builds + tests pass; `run-all-checks.sh` green (20/20); live chat smoke succeeds over the core (not the GUI); Tauri GUI still compiles and is marked deferred.

## FILES CHANGED

- `apps/deskagent/Cargo.toml` — workspace `members` + `src-tauri/crates/deskagent-cli`
- `apps/deskagent/src-tauri/crates/deskagent-cli/` (new crate)
  - `Cargo.toml` — ratatui 0.29, crossterm 0.28, clap 4, serde_json, chrono; path dep on `deskagent-core`
  - `src/main.rs` — clap CLI: TUI (default) + chat / models / approvals / approve / reject / memory / persona / export / wipe
  - `src/data.rs` — data-dir resolution + encryption key resolution (mirrors Tauri shell `resolve_key`), 4 tests
  - `src/chat.rs` — chat engine (capture_turn → extraction pass → context → backend/fallback → citations), 8 tests
  - `src/app.rs` — TUI state: 4 tabs, input editing, inline Y/n approval flow, model pick, export, 9 tests
  - `src/ui.rs` — pure ratatui rendering (TestBackend-tested), 7 tests
  - `README.md` — usage, keys, encryption, live smoke
- `apps/deskagent/README.md` — CLI section; GUI marked deferred
- `scripts/run-all-checks.sh` — +`deskagent-cli build` check (19 → 20)
- `PHASES.md` — Phase 8 status → IN_PROGRESS → COMPLETE; 8 checkboxes `[x]` (status-only, lock-safe)
- `PROGRESS.md` — Phase 8 narrative + 8 task entries

## VALIDATIONS ACTUALLY RUN

| Command | Exit code |
|---|---|
| `bash scripts/plan-lock.sh verify` (pre/post phase + after checkbox marking) | 0 |
| `bash scripts/verify-env.sh` | 0 |
| `cargo build -p deskagent-cli` (from apps/deskagent) | 0 |
| `cargo test -p deskagent-cli` (27/27 + 1 ignored live) — run 3×, stable | 0 |
| `cargo test -p deskagent-cli -- --ignored deskagent_chat_live` (real Ollama) | 0 |
| `cargo test` (workspace: CLI 27 + core 54, 1 ignored each) | 0 |
| `cargo check` (workspace incl. Tauri shell `deskagent`) | 0 |
| `bash scripts/run-all-checks.sh` | 0 (20/20) |
| `deskagent chat "Hello, DeskAgent."` — live Ollama, auto-picked qwen2.5-coder:7b, citation rendered | 0 |
| `deskagent models / --pick` · `approvals` · `approve/reject` · `memory` · `persona` · `export` · `wipe` | 0 (wipe without --yes: 2 as designed) |
| TUI via PTY driver: typed message → real reply + citation → Tab → Memory pane → Esc quit | 0 |
| Encryption check: `deskagent.key` mode 0600; `strings deskagent.db` → memory content only as AES-GCM ciphertext | n/a |

## ACTUAL EXIT CODES

- All validation commands: 0
- `deskagent wipe` without `--yes`: 2 (guarded, designed)
- PTY-boot smoke via `timeout`: 124 (still running in event loop = no panic)

## CI RESULTS

- No `.github/workflows` exist yet (CI lands in Phase 10); `run-all-checks.sh` is the CI surrogate — 20/20 green.

## UNRESOLVED GATES

- None. Two test flake races found during validation (shared SQLite data dir; shared `DESKAGENT_PASSPHRASE` env) were fixed; suites stable across repeated runs.
- Known product limitation (tracked, not blocking): the Tasks pane is a placeholder mirroring the web stub — the core exposes no tasks table; the runner lands with the GUI milestone.
- Chat `--session <missing-id>` errors "session not found" by design (create by omitting `--session`).
- **PR status:** delivered as a direct push to `main` (`dd49afe`); the GitHub PR description is archived in `records/phase-8-pull-description.md` (GitHub cannot host an empty-diff PR since base == head).

## EXACT NEXT ACTION

Phase 9, Task 1: **SkillHub registry security (public multi-tenant)** — "MUST LAND FIRST": normalize the package identifier model to a canonical `owner/name` used identically by schema and handlers; reset the runtime registry DB (no migration code) per the DECIDED note. The read/write key-space mismatch is unresolved until this lands — every later auth task depends on it. Phase 9 depends on Phase 3 (COMPLETE), not Phase 8.

## MILESTONE ACCEPTANCE CLAIMED: NO

(Phase 8 of Milestone 2 is complete, but Phases 9–10 remain; milestone acceptance is only claimed in the Phase 10 handoff.)
