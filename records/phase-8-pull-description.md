# Phase 8 — DeskAgent CLI (terminal UI) · Pull-request description

> **Status:** Phase 8 was delivered as a direct push to `main` (`dd49afe`, plus the
> README follow-up `d547f36`). Because the work is already on `main`, GitHub cannot
> host a non-empty PR for it (base == head). This file preserves the PR description
> for review/archive purposes; `main`'s push IS the merged state.

## Summary

Adds `deskagent-cli` as a third DeskAgent workspace member (binary `deskagent`): a
ratatui + crossterm **four-pane terminal UI** (Chat / Memory+Approvals / Models /
Tasks) mirroring the web tabs, plus headless subcommands (`chat`, `models`,
`approvals`, `memory`, `persona`, `export`, `wipe`).

## Highlights

- **Zero core changes** — `deskagent-cli` depends on `deskagent-core` exactly as the
  Tauri shell does; all wiring lives in the CLI crate.
- **Chat loop** — `capture_turn` (raw episode) → scheduled extraction pass (every 5
  user turns, max 20 proposals) → persona + scoped retrieval with a strict injection
  budget → backend chat → assistant message with "I remember…" citations; **DEC-0005**
  deterministic fallback when the runtime is offline (never hangs, still stores the
  assistant turn).
- **Model picker** — backed by `runtime_list_models` / `remembered_choice`; a plain
  `deskagent chat "…"` auto-picks the first reachable model and persists it.
- **Inline approval cards** — `y`/`n` resolves the focused card through
  `approvals::decide`; headless `approve`/`reject <id>` with short-id matching.
- **Memory explorer + persona + export/wipe** — via `memory_list` / `persona_get` /
  `export_all` / `wipe_all`; wipe requires `--yes` (DEC-0009 delete).
- **Encryption parity (DEC-0009)** — identical key resolution to the shell
  (`DESKAGENT_PASSPHRASE` + persisted salt, else 0600 keyfile); verified 0600
  keyfile and AES-GCM ciphertext at rest.
- **GUI deferred** — the Tauri shell still compiles (`cargo check` green) and is
  explicitly marked deferred; the CLI is the supported desktop surface.

## Validation

- `cargo build -p deskagent-cli` — exit 0
- `cargo test -p deskagent-cli` — 27/27 (+1 ignored live Ollama smoke)
- Workspace `cargo test` — CLI 27 + core 54
- `cargo test -p deskagent-cli -- --ignored deskagent_chat_live` — exit 0
- `bash scripts/run-all-checks.sh` — **20/20** (added `deskagent-cli build` check)
- Live smoke over the core (not the GUI): `deskagent chat "Hello, DeskAgent."`
  against local Ollama auto-picked `qwen2.5-coder:7b` and replied with a real
  "I remember…" citation; TUI driven end-to-end through a PTY.
- `bash scripts/plan-lock.sh verify` — PASS before/after every task, pre/post phase,
  and post-push (`8d9253e4668c` baseline unchanged).

## Files changed

- `apps/deskagent/Cargo.toml` — workspace `members` + `src-tauri/crates/deskagent-cli`
- `apps/deskagent/src-tauri/crates/deskagent-cli/` (new crate: `Cargo.toml`,
  `src/{main,data,chat,app,ui}.rs`, `README.md`)
- `apps/deskagent/README.md` — CLI section; GUI marked deferred
- `scripts/run-all-checks.sh` — +`deskagent-cli build` (19 → 20 checks)
- `PHASES.md` — Phase 8 status/checkbox updates only (lock-safe)
- `PROGRESS.md` — Phase 8 narrative + 8 task entries
- `records/phase-8-handoff.md` — phase handoff

## Next action

Phase 9 — SkillHub registry security (public multi-tenant). Task 1 is
**MUST LAND FIRST**: normalize the package identifier to a canonical `owner/name`
used identically by schema and handlers; reset the runtime registry DB (no
migration code).
