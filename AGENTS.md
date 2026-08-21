# AGENTS.md — agent-ecosystem (content-locked build plan)

This repo runs a content-locked plan. Read this file and PHASES.md "Lock Policy (READ
FIRST)" before touching anything. PHASES.md is the normative source for the lock and the
DEC constraints; this file is the operational contract for agents.

## Source of truth
`PLAN.lock` (never) > `PHASES.md` (checkboxes/status comments only) > `AGENTS.md` (no — fixes
require human review) > `PROGRESS.md` (yes — the only free-form channel) > `README.md` (no).

## Canonical commands (all verified in-repo)
- **Lock:** `bash scripts/plan-lock.sh {verify|status|propose}` — `verify` before/after every
  task & phase (must exit 0); `propose "<reason>"` is the ONLY plan-change channel;
  `init`/`approve` are humans-only (git hooks call `check-staged`/`check-push` internally).
- **Env:** `bash scripts/verify-env.sh` — must print `ENV-OK`.
- **Validate:** `bash scripts/run-all-checks.sh` — 21 checks, must print `RUN-ALL-CHECKS-OK`;
  authoritative per-product suite (bench-site, slopgate-action, slopgate-dash, skillhub-web,
  skillhub-cli/registry, deskagent, deskagent-cli) — mirror its exact commands.
- **Root tests:** `npm test`. **Hooks:** `hooks/install-hooks.sh` (never bypass). **Demos:**
  `bash scripts/demos/*.sh`.

## Lock rules (PHASES.md "Lock Policy" is normative; summary)
- `verify` before/after every task and phase. On FAIL: STOP, do not edit `PLAN.lock`, do not commit.
- `PHASES.md` edits: checkbox flips and status comments only (normalized away before hashing).
- PROHIBITED: edit `PLAN.lock`; run `init`/`approve`; read `~/.config/agent-ecosystem/plan.key`;
  set or use `PLAN_APPROVAL_TOKEN`; `git commit --no-verify`, hook edits, force-push, rebase, or
  amend any commit touching `PHASES.md`/`PLAN.lock`.
- Change requests: `propose "<reason>"` → human edits → human runs `approve`. Never implement
  unapproved scope.

## Guardrails
Follow the DEC-0001…DEC-0009 constraints in PHASES.md "Locked Constraints" — that table is the
single source (this file does not restate them). Layout: `apps/` (one dir per product),
`shared/`, `scripts/`; `hooks/`, `records/`, `data/` are plan-sanctioned top-level dirs.
DEC-0009 (local-first, approval-gated memory) applies to all DeskAgent work.

## Execution
Run per the phase-executor skill, with these repo deltas: branch must be `main`; per phase:
`verify` → `bash scripts/verify-env.sh` → `git tag phase-{N}-start`; execute tasks, ticking
checkboxes in `PHASES.md` immediately; run the phase's `<!-- VALIDATE: … -->` command (retry
3×, then mark `BLOCKED`, record the reason in `PROGRESS.md`, roll back to `phase-{N}-start`,
STOP); write `records/phase-{N}-handoff.md`; mark `COMPLETE`, delete the tag, mirror the next
phase's tasks into `PROGRESS.md` before starting.

## Required post-task template (append to PROGRESS.md)
```markdown
### Task done: <task text>
- FILES CHANGED: <paths + insertions/deletions>
- VALIDATIONS RUN: <commands + exit codes>
- EXIT CODES: <map>
- Lock verify: PASS/FAIL
```
Also available as `.pi/prompts/post-task.md`.

## Safety, mobile, release
- **Secrets:** never commit, log, or echo `*.db`, `.env`, seed tokens, signing keys, or plan.key
  material; Phase 9 adversarial fixtures must stay inert (no real credentials).
- **Mobile (DeskAgent TUI):** keep narrow (40–60 col) rendering, min-size guard, keyboard-only
  fallback; run the TestBackend suite at 40/50/60/120 cols; status bar must not truncate.
- **Release (Phase 10):** `scripts/release-gate.sh` is release-only; never loosen `run-all-checks.sh`.

## Handoff & acceptance
Phase handoffs `records/phase-{N}-handoff.md` carry: completion state, FILES CHANGED,
VALIDATIONS RUN + exit codes, UNRESOLVED GATES, EXACT NEXT ACTION. Final phase:
`records/final-handoff.md` claims `MILESTONE ACCEPTANCE CLAIMED: YES` only when every
Definition-of-Done item passes.

## Ambiguity
Stop and ask the human. An ambiguous instruction is never a license to edit the plan.
