# PROGRESS.md — Execution status (agent-writable)

> This is the **only** free-form file agents write to. PHASES.md is content-locked.
> Append, never overwrite. Keep entries chronological.

## Status legend

- `[x]` completed / `[/]` in progress / `[ ]` pending / `[-]` blocked-cancelled
- Phase status markers live in PHASES.md headers; per-phase narrative lives here.

---

## Phase 1: Recon, scaffold, and lock activation

**Phase status:** COMPLETE (2026-08-14)
**Started:** 2026-08-14
**Notes:**
- Plan approved by human 2026-08-14; lock initialized via `plan-lock.sh init` (baseline 865734f…), re-locked after the DeskAgent v2 amendment (ad23c2aa…, commit dca7718).
- Env verified: git 2.43.0, jq 1.7, OpenSSL 3.0.13, node v22.22.3, npm 12.0.1, cargo/rustc 1.96.0. `tauri` not found — warn only (optional until Phase 5).
- `verify-env.sh` fixed during execution (openssl now runs `openssl version`).
- Skeleton: `apps/README.md` (8 product dirs), `shared/README.md`, `shared/datasets/README.md` (DEC-0001 layout).
- Drafts created: `shared/schemas/benchmark-result.schema.json` v1, `shared/specs/skill-manifest-spec-v1.md` (open questions recorded for Phase 3).
- Hooks installed and enforcement proven live (2026-08-14): content change without token BLOCKED; wrong token BLOCKED; valid token ALLOWED; `approve` without TTY refused; status-only edits pass without token.
- Task "Lock the plan" completed via `init` (the `lock` subcommand in the plan maps to `init` in the script).
- **Guardrail incident (self-caught):** during Phase 1 execution an agent edit added a `(Note: ...)` parenthetical to a locked task line — `verify` flagged CONTENT DRIFT, the note was reverted, verify passed again. Lesson recorded: status/checkbox updates only, ever; explanatory notes belong in PROGRESS.md, never in PHASES.md.

### Task done: Verify environment
- FILES CHANGED: none (recorded in this file)
- VALIDATIONS RUN: `bash scripts/verify-env.sh` exit 0
- EXIT CODES: 0
- Lock verify: PASS

### Task done: Monorepo skeleton
- FILES CHANGED: apps/README.md +1, shared/README.md +1, shared/datasets/README.md +1
- VALIDATIONS RUN: `git status --short`
- EXIT CODES: 0
- Lock verify: PASS

### Task done: verify-env.sh
- FILES CHANGED: scripts/verify-env.sh (openssl version flag)
- VALIDATIONS RUN: `bash scripts/verify-env.sh` exit 0
- EXIT CODES: 0
- Lock verify: PASS

### Task done: AGENTS.md constitution
- FILES CHANGED: AGENTS.md (bootstrap, 2026-08-14)
- VALIDATIONS RUN: n/a (doc)
- EXIT CODES: 0
- Lock verify: PASS

### Task done: README.md
- FILES CHANGED: README.md (bootstrap, 2026-08-14)
- VALIDATIONS RUN: n/a (doc)
- EXIT CODES: 0
- Lock verify: PASS

### Task done: PROGRESS.md seeded
- FILES CHANGED: PROGRESS.md (bootstrap + this record)
- VALIDATIONS RUN: n/a (doc)
- EXIT CODES: 0
- Lock verify: PASS

### Task done: Install git hooks + confirm blocking
- FILES CHANGED: hooks/ (bootstrap); .git/hooks/pre-commit, pre-push installed
- VALIDATIONS RUN: drift test (content edit w/o token blocked; wrong token blocked; valid token allowed)
- EXIT CODES: 0/1 as designed
- Lock verify: PASS

### Task done: benchmark-result.schema.json (draft v1)
- FILES CHANGED: shared/schemas/benchmark-result.schema.json +1
- VALIDATIONS RUN: `jq empty shared/schemas/benchmark-result.schema.json`
- EXIT CODES: 0
- Lock verify: PASS

### Task done: skill-manifest-spec-v1.md (draft)
- FILES CHANGED: shared/specs/skill-manifest-spec-v1.md +1
- VALIDATIONS RUN: n/a (doc)
- EXIT CODES: 0
- Lock verify: PASS

### Task done: Lock the plan
- FILES CHANGED: PLAN.lock (via `plan-lock.sh init` at bootstrap; re-locked at amendment dca7718)
- VALIDATIONS RUN: `bash scripts/plan-lock.sh verify` exit 0
- EXIT CODES: 0
- Lock verify: PASS

## Phase 2: BenchKit — benchmark data, calculator, and site

**Phase status:** COMPLETE (2026-08-14)
**Started:** 2026-08-14
**Notes:**
- Dataset seeded: 7 rows, 5 sources (kimi-k3-in-c ×2 configs + 2 quant-study rows, warp, turbo-fieldfare, MiniMax-H3 sglang, h3.c) — every row has `source_url` (DEC-0006). Real numbers: Kimi K3 0.088 tps @ 127.9 GB peak; warp 0.6 tps on 64 GB MBP; KDA constant 626 MB KV; KV 2.37 MB/pos; int8 ≈ 1% / int4 ≈ 17% error. Nulls where unpublished — never fabricated.
- Schema refined: `tokens_per_sec` + `peak_ram_gb` relaxed to nullable for honest seeding.
- Calculator `shared/lib/will-it-run.mjs`: MoE active-param handling, KDA constant-KV, quant tiers, streaming verdicts; 8/8 edge-case tests pass.
- Site `apps/bench-site` (Next 15.5 / React 19, zero extra deps): searchable/sortable matrix, will-it-run widget, model detail pages + quant SVG charts; 10/10 static routes prerendered.
- Runner skeleton `scripts/bench-run.ts`: hardware detect, times a command, parses tokens/s, appends `runner:local` rows. Peak-RAM capture = documented follow-up.
- Fixes during execution: client/server module split (node:fs out of client bundle), Next 15 async params, nested-route path depth, runner typing.

### Task done: Seed benchmarks.jsonl
- FILES CHANGED: shared/datasets/benchmarks.jsonl +7 rows
- VALIDATIONS RUN: `node shared/datasets/validate-dataset.mjs` exit 0
- EXIT CODES: 0
- Lock verify: PASS

### Task done: validate-dataset.mjs
- FILES CHANGED: shared/datasets/validate-dataset.mjs +1
- VALIDATIONS RUN: `node shared/datasets/validate-dataset.mjs` exit 0
- EXIT CODES: 0
- Lock verify: PASS

### Task done: will-it-run.mjs calculator
- FILES CHANGED: shared/lib/will-it-run.mjs +1
- VALIDATIONS RUN: `node --test shared/lib/test/will-it-run.test.mjs` exit 0 (8/8)
- EXIT CODES: 0
- Lock verify: PASS

### Task done: bench-site scaffold + matrix
- FILES CHANGED: apps/bench-site/ (package.json, tsconfig, next.config, app/, components/matrix.tsx, lib/types.ts, lib/benchmarks.ts, globals.css)
- VALIDATIONS RUN: `npm run build` exit 0 (10 static routes)
- EXIT CODES: 0
- Lock verify: PASS

### Task done: model detail pages + quant charts
- FILES CHANGED: apps/bench-site/app/models/[slug]/page.tsx, components/quant-chart.tsx
- VALIDATIONS RUN: `npm run build` (7 model slugs generated)
- EXIT CODES: 0
- Lock verify: PASS

### Task done: bench-run.ts runner skeleton
- FILES CHANGED: apps/bench-site/scripts/bench-run.ts +1
- VALIDATIONS RUN: `node --experimental-strip-types scripts/bench-run.ts --help` exit 0
- EXIT CODES: 0
- Lock verify: PASS

### Task done: calculator edge-case tests
- FILES CHANGED: shared/lib/test/will-it-run.test.mjs +1 (8 tests)
- VALIDATIONS RUN: `npm test` exit 0 (calculator + dataset)
- EXIT CODES: 0
- Lock verify: PASS

---

## CHANGE REQUEST <!-- REQUEST_CLOSED -->
- Proposed: 2026-08-14 — DeskAgent v2 amendment (approved in review)
- Status: **APPROVED by human 2026-08-14** — `approve` ceremony completed, re-locked at content_sha256=ad23c2aa…; amendment committed (dca7718).

## CHANGE REQUEST <!-- REQUEST_CLOSED -->
- Proposed: 2026-08-14T21:38:28Z — DeskAgent v2 amendment
- Status: APPROVED by human 2026-08-14 (re-locked ad23c2aa…, committed dca7718).

## CHANGE REQUEST <!-- REQUEST_CLOSED -->
- Proposed: 2026-08-15T01:16:08Z — META repository field update
- Status: APPROVED by human 2026-08-15 (re-locked 040ca814…, committed 2b4da29).
