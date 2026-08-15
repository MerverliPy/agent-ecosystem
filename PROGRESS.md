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

## Phase 3: SkillHub — manifest, CLI, registry, site

**Phase status:** COMPLETE (2026-08-14)
**Started:** 2026-08-14
**Notes:**
- Spec finalized: JSON manifest (`skillhub.json`), draft open questions resolved (JSON over TOML; exact pins + ^-caret ranges; MCP deps deferred to v2). Machine-validated by `shared/schemas/skill-manifest.schema.json`.
- CLI (`apps/skillhub-cli`, Rust/clap): search, info, install (writes skill + `skillhub.lock.json`), update, remove, verify, scan, harnesses, publish. Harness detection for 7 harnesses (env vars + config paths), pi fallback. Path traversal guards on install.
- Security scanner: 27 rules (INJ×4, SHELL×8, NET×4, SEC×6, ENC×3, BIN×2), binary/archive detection + invalid-UTF8 ratio. 3 malicious fixtures all flagged; benign fixture clean (false-positive guard).
- Registry (`apps/skillhub-registry`, axum 0.8 + rusqlite bundled): publish (immutable versions, 409 on dup), search, detail, files+download counting, health. Fixed mid-phase: package names contain '/' so routes use `{owner}/{name}` segments (single `{name}` param cannot hold a slash).
- Web (`apps/skillhub-web`, Next.js 15): search grid, verified/high-risk/download badges, per-harness install commands, version tables; snapshot from registry via e2e (`data/skills.json`).
- E2E (`apps/skillhub-cli/scripts/e2e.sh`): 13/13 — publish benign (verified) + 3 malicious (unverified), search, install to temp harness (SKILL.md + lockfile), verify exits 1 with SHELL-02/NET-02 findings, remove, web snapshot.
- Implementation decisions: `publish` subcommand added to CLI (plan's registry task needed a client half); verified = zero high-severity scan findings.

### Task done: Finalize manifest spec + schema
- FILES CHANGED: shared/specs/skill-manifest-spec-v1.md (rewrite), shared/schemas/skill-manifest.schema.json (new)
- VALIDATIONS RUN: CLI manifest::tests (parse/validate) pass; `jq empty` on schema
- EXIT CODES: 0
- Lock verify: PASS

### Task done: CLI scaffold + commands
- FILES CHANGED: apps/skillhub-cli/src/{main,manifest,harness,lockfile,registry}.rs, Cargo.toml
- VALIDATIONS RUN: `cargo test` 9/9; `cargo build` clean
- EXIT CODES: 0
- Lock verify: PASS

### Task done: Harness detection
- FILES CHANGED: apps/skillhub-cli/src/harness.rs
- VALIDATIONS RUN: harness tests (pi known, unknown rejected)
- EXIT CODES: 0
- Lock verify: PASS

### Task done: verify subcommand
- FILES CHANGED: apps/skillhub-cli/src/main.rs (cmd_verify)
- VALIDATIONS RUN: e2e — verify malware/exfil-shell exits 1 with findings
- EXIT CODES: 1 (expected)
- Lock verify: PASS

### Task done: Security scanner (27 rules) + fixtures
- FILES CHANGED: apps/skillhub-cli/src/scan.rs, apps/skillhub-cli/fixtures/{benign,exfil-shell,prompt-inject,secret-stealer}-skill/
- VALIDATIONS RUN: scan tests — benign clean, 3 malicious flagged; e2e flags all 3 unverified
- EXIT CODES: 0
- Lock verify: PASS

### Task done: Registry API
- FILES CHANGED: apps/skillhub-registry/src/main.rs, Cargo.toml
- VALIDATIONS RUN: `cargo test` 4/4 (publish/search/409/roundtrip/downloads)
- EXIT CODES: 0
- Lock verify: PASS

### Task done: Web site
- FILES CHANGED: apps/skillhub-web/ (app/, components/browse.tsx, lib/, test/, data/skills.json)
- VALIDATIONS RUN: `npm run build` (8 static routes), `npm test` 3/3
- EXIT CODES: 0
- Lock verify: PASS

### Task done: End-to-end test
- FILES CHANGED: apps/skillhub-cli/scripts/e2e.sh (new)
- VALIDATIONS RUN: `bash apps/skillhub-cli/scripts/e2e.sh` — 13 passed / 0 failed
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

---

## Phase 4: SlopGate — rules, score, CI action, dashboard

**Phase status:** IN_PROGRESS (2026-08-15)
**Started:** 2026-08-15
**Notes:**
- Deterministic rule pack in `apps/slopgate` (TypeScript, zero runtime deps, runs via `node --experimental-strip-types`): 46 rules — DEAD×6, UNUSED×5, COMM×6, NAME×5, OVER×9, COMMIT×11, AI×14. Every rule has inline fixture tests (positive + negative).
- Cross-file analysis (unreferenced exports/interfaces/local classes, pass-through wrappers) via regex symbol table; per-file rules via brace-aware scanning. Heuristics documented; a rule never crashes a scan.
- `slop` CLI: scan (text/JSON/SARIF), score (0–100, higher = worse), lint (threshold gate, exit 1 above with `--block`), check-text, llm-review, rules, version. SARIF 2.1.0 output.
- Scoring: severity weights high 10 / med 5 / low 2, density bonus for files > 5 findings, cap 100.
- Fixtures seeded: clean → 0, mild → 29, heavy → 100 (53 findings, 36 distinct rules). Ordering + threshold gating asserted in tests.
- LLM review layer (bring-your-own-key): deterministic pattern catalog always runs; LLM pass disabled without SLOPGATE_LLM_KEY/OPENAI_API_KEY (DEC-0005). Mock-fetch tests only — no network.
- Root `package.json` added (required by Phase 4 VALIDATE `npm test`); test runner uses glob discovery for `*.test.ts`.

### Task done: Scaffold apps/slopgate + deterministic rule pack
- FILES CHANGED: apps/slopgate/ (package.json, tsconfig, bin/slop.mjs, src/{types,analysis,scanner,score,report,llm,cli}.ts, src/rules/{dead,unused,comments,naming,over,commit,ai,index}.ts, README.md), package.json (root, new)
- VALIDATIONS RUN: `npm test --prefix .` exit 0 (80/80); `npx --prefix apps/slopgate tsc --noEmit -p apps/slopgate/tsconfig.json` exit 0
- EXIT CODES: 0 / 0
- Lock verify: PASS

### Task done: slop CLI (scan / score / lint)
- FILES CHANGED: apps/slopgate/src/cli.ts, report.ts, score.ts (scan+score+lint+check-text+llm-review+rules+version; JSON/SARIF output)
- VALIDATIONS RUN: CLI integration tests in test/cli.test.ts (13 tests, all pass); SARIF artifact written and parsed
- EXIT CODES: 0
- Lock verify: PASS

### Task done: LLM review layer (BYOK)
- FILES CHANGED: apps/slopgate/src/llm.ts, test/llm.test.ts (llmConfig, patternCatalog, catalogReview, parseLlmJson, mergeReviews, reviewProseWithLlm w/ mock fetch)
- VALIDATIONS RUN: `npm test` llm tests 10/10; CLI `llm-review` without key returns enabled:false + catalog findings (exit 0)
- EXIT CODES: 0
- Lock verify: PASS

### Task done: Fixture repos + ordering/gating tests
- FILES CHANGED: apps/slopgate/fixtures/{clean,mild,heavy}/ (package.json, src/*, README.md)
- VALIDATIONS RUN: score ordering clean(0) < mild(29) < heavy(100); `slop lint --threshold 50` fails heavy (exit 1), passes clean (exit 0); test/score.test.ts 7/7
- EXIT CODES: 0 (and 1 as designed for the gate)
- Lock verify: PASS

### Task done: slopgate-action (GitHub Action)
- FILES CHANGED: apps/slopgate-action/ (action.yml, package.json, main.mjs, lib/core.mjs, test/core.test.mjs, README.md)
- VALIDATIONS RUN: `npm test --prefix apps/slopgate-action` exit 0 (9/9: inputs, gate pass/warn/fail, comment+summary builders, event-payload parsing, real CLI integration via runSlop); `npm run build --prefix apps/slopgate-action` exit 0
- EXIT CODES: 0 / 0
- Lock verify: PASS

### Task done: slopgate-dash (Next.js)
- FILES CHANGED: apps/slopgate-dash/ (package.json, tsconfig, next.config.ts, app/{layout,page,globals.css}.tsx, app/repos/[repo]/page.tsx, components/trend-chart.tsx, lib/{types,history}.ts, scripts/record-run.mjs, data/history.json, test/history.test.mjs, README.md)
- VALIDATIONS RUN: `npm test --prefix apps/slopgate-dash` exit 0 (5/5 data contract); `npm run build --prefix apps/slopgate-dash` exit 0 (7 static pages, 3 repo pages); artifact recorded from real scans (clean 0 / mild 29 / heavy 100); rendered HTML verified (stats + polyline points)
- EXIT CODES: 0 / 0
- Lock verify: PASS

## Phase 4 post-phase

**Phase status:** COMPLETE (2026-08-15)
- VALIDATE hook: `bash scripts/plan-lock.sh verify` exit 0 · `npm test` exit 0 (80/80) · `cd apps/slopgate-action && npm run build` exit 0
- Exit criteria met: rule pack passes fixture tests (80/80); sloppy fixture scores high (100) and clean low (0); action gates CI at threshold (decideGate fail + lint exit 1 on heavy); dashboard renders a trend from the recorded artifact (7 static pages, polyline verified in HTML)
- Handoff: `records/phase-4-handoff.md` — 6/6 tasks, no blocked gates; CI not triggered (no `.github/workflows` yet — Phase 7)

## Phase 5: DeskAgent — self-memory core

**Phase status:** PENDING
**Mirrored tasks (from PHASES.md; checkboxes live in PHASES.md):**
- [ ] Create `shared/schemas/memory-event.schema.json`: four memory kinds (episodic, semantic, procedural, working) with sources, confidence, timestamps, and project scope; include validation tests.
- [ ] Scaffold `apps/deskagent` (Tauri 2 + React + TypeScript). Window shell, chat UI, session persistence (SQLite).
- [ ] Implement memory store (SQLite + local embeddings via sqlite-vec/fastembed-rs): episodic log, semantic facts, procedural records, working context; encrypted at rest; export/delete APIs.
- [ ] Implement capture pipeline: every conversation appended as raw episodes; extraction pass distills facts/preferences every N turns (default 5, max 20 memories per pass).
- [ ] Implement consolidation & persona: regenerate the persona model every N new memories (default 50); dedupe + conflict detection + decay; reflection on local models by default, per-session opt-in cloud for heavy passes (DEC-0005, DEC-0009).
- [ ] Implement hybrid retrieval: keyword + embedding recall (RRF fusion) with strict injection budget; companion-level + per-project scoping (both scopes per DEC-0009).
- [ ] Implement propose-to-remember approval cards: every memory write routes through the sandbox approval system; approvals and rejections recorded as learning signal.
- [ ] Implement memory UX: memory explorer (timeline/facts/projects — browse, edit, pin, delete, export) and persona card view.
- Exit criteria: memory schema validated; store encrypted and deletable; pipeline extracts memories from a fixture conversation; persona regenerates; retrieval returns scoped hits; memory writes require approval; explorer and persona card render.
