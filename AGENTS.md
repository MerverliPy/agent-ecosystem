# AGENTS.md — Constitution for executing agents

This repository runs a **content-locked build plan**. Read this file completely before touching anything.
The plan file (`PHASES.md`) is immutable; the lock is enforced by scripts and git hooks, and this constitution
is the behavioral contract. Both layers apply.

## 1. Source-of-truth hierarchy

| Rank | File | Role | Writable by agents? |
|------|------|------|---------------------|
| 1 | `PLAN.lock` | Lock manifest (hash, token hash, history) | **NEVER** |
| 2 | `PHASES.md` | The build plan (content) | Content: **NO**. Checkboxes `[ ]`→`[x]` and status comments: **YES** |
| 3 | `AGENTS.md` | This constitution | No (fixes require human review) |
| 4 | `PROGRESS.md` | Execution status, notes, change requests | **YES — your only free-form channel** |
| 5 | `README.md` | Ecosystem overview | No |

## 2. Lock policy (DEC-0003, DEC-0008)

- Run `bash scripts/plan-lock.sh verify` **before and after every phase** and **after every task** that touches
  any file. If it fails: STOP. Do not continue, do not edit `PLAN.lock`, do not commit.
- You may flip task checkboxes (`- [ ]` → `- [x]`) and phase status comments (`PENDING` → `IN_PROGRESS` →
  `COMPLETE`, or `BLOCKED` with reason in PROGRESS.md). These are normalized away before hashing, so they
  never break the lock. Everything else in `PHASES.md` — adding/removing/reordering/rephrasing tasks or phases —
  is a **content change** and is forbidden.
- **PROHIBITED (zero-tolerance):**
  - Modify `PLAN.lock`, ever.
  - Run `scripts/plan-lock.sh init` or `approve`.
  - Read `~/.config/agent-ecosystem/plan.key`.
  - Set or use `PLAN_APPROVAL_TOKEN`.
  - Bypass the hooks (`git commit --no-verify`, manual hook edits, deleting `.git/hooks/pre-commit`).
  - Force-push, rebase history, or amend a commit that touched `PHASES.md`/`PLAN.lock`.
- **To request a change** (new task, reorder, scope change): `bash scripts/plan-lock.sh propose "<reason>"`.
  This appends a `CHANGE REQUEST` to `PROGRESS.md` and stops your current task. The human reviews; if approved,
  the human edits `PHASES.md` and re-locks with `approve`. Only then may you proceed. Never implement
  unapproved scope.

## 3. Guardrails (locked constraints from PHASES.md META)

- `DEC-0001` Monorepo: `apps/<product>`, `shared/`, `scripts/`. No new top-level dirs.
- `DEC-0002` MIT/Apache-2.0 only; no copyleft dependencies.
- `DEC-0004` Rust for CLIs, TypeScript/Next.js for web, Tauri 2 + React for desktop.
- `DEC-0005` No mandatory telemetry; cloud calls opt-in.
- `DEC-0006` BenchKit rows must carry `source_url`; no paid ranking.
- `DEC-0007` Strict phase order; no scope creep.

## 4. Execution protocol (per phase-executor conventions, adapted)

1. **Boot:** `verify` the lock → read `PROGRESS.md` → find first phase not COMPLETE with unchecked tasks.
2. **Pre-phase:** branch must be `main`; run `bash scripts/verify-env.sh`; create checkpoint tag
   `git tag phase-{N}-start`.
3. **Pre-task:** run `verify`; snapshot `git status --porcelain`.
4. **Execute** the task. When done: update its checkbox in `PHASES.md` **immediately**.
5. **Post-task:** `verify` again; append a one-line diff summary to `PROGRESS.md` under the phase section.
6. **Post-phase:** run the phase's `<!-- VALIDATE: ... -->` command. If it fails, retry up to 3×, then mark the
   phase `BLOCKED`, record the reason in `PROGRESS.md`, roll back to `phase-{N}-start`, and STOP.
7. **Handoff:** write `records/phase-{N}-handoff.md` with: completion state, FILES CHANGED, VALIDATIONS RUN +
   exit codes, UNRESOLVED GATES, EXACT NEXT ACTION (first task of next phase).
8. Mark the phase `COMPLETE`, delete the checkpoint tag, update `PROGRESS.md`, and mirror the next phase's
   tasks into `PROGRESS.md` before starting.

## 5. Required post-task template (append to PROGRESS.md after every task)

```markdown
### Task done: <task text>
- FILES CHANGED: <paths + insertions/deletions>
- VALIDATIONS RUN: <commands + exit codes>
- EXIT CODES: <map>
- Lock verify: PASS/FAIL
```

## 6. Prohibited actions (beyond the lock)

- No work outside the current phase's tasks; no unplanned features.
- No deletion or renaming of `PHASES.md`, `PLAN.lock`, `PROGRESS.md`, `AGENTS.md`.
- No telemetry by default (DEC-0005); no license changes (DEC-0002).
- Do not claim milestone acceptance unless the final handoff says `MILESTONE ACCEPTANCE CLAIMED: YES` and all
  Definition of Done items in PHASES.md pass.

## 7. If anything is ambiguous

STOP and ask the human. Do not guess. An ambiguous instruction is never a license to edit the plan.
