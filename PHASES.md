# Project Phases — The Synergy Build

<!-- META
created: 2026-08-14
repository: MerverliPy/agent-ecosystem
branch: main
generated_from:
  - Research: GitHub trending (2026-08-14) — agent harnesses, skills, local inference, AI code quality
  - Competitive scan: skills.sh, tonsofskills.com, ClawHub, tech-leads-club/agent-skills, iflytek/skillhub,
    Observal, agentguard, cc-haha, skills-manage, openclaw, qm, pi, Abu-Cowork, talkio
  - Local inference research: kimi-k3-in-c, sqliteai/warp, turbo-fieldfare, MiniMax-H3
  - Anti-slop research: dmmulroy/anti-slop, anthropics/claude-code-security-review, tagore
  - Conventions: phases-creator SKILL.md, phase-executor SKILL.md
locked_constraints: DEC-0001, DEC-0002, DEC-0003, DEC-0004, DEC-0005, DEC-0006, DEC-0007, DEC-0008, DEC-0009
active_milestone: Milestone 1 — Cross-project synergies (BenchKit, SkillHub, SlopGate, DeskAgent)
milestone_state: ACCEPTED
next_action: Phase 1 — Recon, scaffold, and lock activation
-->

# Lock Policy (READ FIRST)

This file is **content-locked**. Per DEC-0003:

- Agents may update task checkboxes (`[ ]` → `[x]`) and phase status comments (`PENDING` → `IN_PROGRESS` → `COMPLETE`).
- Agents may **NOT** add, remove, reorder, or reword any other content.
- Content changes require explicit human approval: run `scripts/plan-lock.sh propose "<reason>"` (appends a change request to `PROGRESS.md`), then the human edits this file and runs `scripts/plan-lock.sh approve "<reason>"`.
- `scripts/plan-lock.sh verify` must pass before and after every phase. If it fails, stop work immediately and report the drift.
- All execution status is tracked in `PROGRESS.md`, never in this file.

# Locked Constraints

| ID | Constraint |
|----|-----------|
| DEC-0001 | Monorepo layout: `apps/` (one dir per product), `shared/` (specs, schemas, datasets), `scripts/` (tooling). No new top-level app dirs without approval. |
| DEC-0002 | All code we publish is MIT or Apache-2.0. Third-party deps must be permissively licensed. No copyleft (GPL/AGPL) dependencies. |
| DEC-0003 | PHASES.md content is immutable once locked. Changes require the approval ceremony in the Lock Policy above. |
| DEC-0004 | Languages: CLI tooling in Rust (single static binary); web frontends in TypeScript/Next.js; desktop app is Tauri 2 + React. No new languages without approval. |
| DEC-0005 | Local-first & privacy: no mandatory telemetry in any product; cloud model calls are opt-in only. |
| DEC-0006 | BenchKit data integrity: every benchmark row must link to an attributable source (repo README, paper, or runner log). No paid placement, no editorial ranking by sponsor. |
| DEC-0007 | Build order: Phase N depends on Phase N-1. No scope creep beyond tasks listed below. A task may only be added by the approval ceremony. |
| DEC-0008 | The plan lock is non-negotiable: agents must never bypass `plan-lock.sh` (no manual `PLAN.lock` edits, no `--force`). |
| DEC-0009 | DeskAgent memory is local-first and user-owned: stored locally, encrypted at rest, exportable, deletable, and never silently written — every memory write requires an approval card. Reflection runs on local models by default; cloud reflection is per-session opt-in. |

# Build Order Rationale

BenchKit first: zero dependencies, 2-week win, produces the dataset the other products consume.
SkillHub second: the primary product, needs BenchKit's credibility data for its verified badges.
SlopGate third: shares static-analysis muscle with SkillHub's security scanner and reuses the same community.
DeskAgent last: heaviest, consumes BenchKit data (model picker) and SkillHub spec (marketplace). Reframed as a personal agent with a self-memory system (companion + project scopes) — the moat asset — so it ships after the data, skills, and scanning layers it builds on.

---

## Phase 1: Recon, scaffold, and lock activation <!-- COMPLETE -->
<!-- VALIDATE: bash scripts/plan-lock.sh verify && bash scripts/verify-env.sh -->
- [x] Verify environment: rustc/cargo, node ≥ 20, npm, git; record versions in `PROGRESS.md`.
- [x] Create monorepo skeleton: `apps/`, `shared/`, `scripts/` with placeholder READMEs per DEC-0001.
- [x] Create `scripts/verify-env.sh` that checks required toolchains and exits non-zero with a clear message on failure.
- [x] Create `AGENTS.md` constitution (lock policy, guardrails, pre/post-task templates).
- [x] Create `README.md` with ecosystem overview and links to the four products.
- [x] Create `PROGRESS.md` and seed with Phase 1 entries.
- [x] Install git hooks (`hooks/install-hooks.sh` → pre-commit + pre-push) and confirm they block a deliberate PHASES.md edit without token.
- [x] Create `shared/schemas/benchmark-result.schema.json` (draft v1: model, hardware, runtime, quant, tokens_per_sec, peak_ram_gb, disk_size_gb, quality_delta, source_url, submitted_at).
- [x] Create `shared/specs/skill-manifest-spec-v1.md` (draft: name, version, harnesses[], dependencies[], permissions[], repo, license).
- [x] Lock the plan: run `scripts/plan-lock.sh lock` (records baseline hash). Confirm `verify` passes.
- **Exit criteria:** repo green; hooks enforce; both draft schemas/specs exist; lock active.

## Phase 2: BenchKit — benchmark data, calculator, and site <!-- COMPLETE --> <!-- DEPENDS_ON: Phase 1 -->
<!-- VALIDATE: bash scripts/plan-lock.sh verify && cd apps/bench-site && npm run build && npm test -->
- [x] Seed `shared/datasets/benchmarks.jsonl` from published sources: kimi-k3-in-c (2.78T params, 3.7% active, 8.24GB RAM, 1.56TB disk, MXFP4, int8≈1% error / int4≈17% error), sqliteai/warp (expert streaming from NVMe), turbo-fieldfare (Gemma 4 26B-A4B, ~2GB RAM, M-series), MiniMax-H3. Every row has `source_url`.
- [x] Write `shared/datasets/validate-dataset.mjs`: parses and validates every row against the JSON schema; `npm test` fails on invalid rows.
- [x] Implement `shared/lib/will-it-run.mjs` calculator: RAM estimate = weights + KV cache + overhead; speed estimate from memory bandwidth × active params; returns fit verdict (fits/streams-needed/no-fit) with assumptions listed.
- [x] Scaffold `apps/bench-site` (Next.js + TypeScript). Implement searchable matrix page: filters for RAM, hardware, runtime, quantization; sortable columns; "Will it run?" calculator widget wired to the dataset.
- [x] Add model detail pages with quantization-quality chart data (error vs. budget) from the seeded study numbers.
- [x] Implement `apps/bench-site/scripts/bench-run.ts` runner skeleton: hardware detection (RAM, CPU, GPU, OS), measures tokens/sec and peak RAM against a local runtime (Ollama/llama.cpp), appends a `source:runner` row.
- [x] Add project-level tests for calculator edge cases (MoE active-param handling, quantization tiers, streaming).
- **Exit criteria:** dataset validated; calculator matches all seeded rows within tolerance; site builds and passes tests; runner runs end-to-end on at least one local model.

## Phase 3: SkillHub — manifest, CLI, registry, site <!-- PENDING --> <!-- DEPENDS_ON: Phase 2 -->
<!-- VALIDATE: bash scripts/plan-lock.sh verify && cargo test && cd apps/skillhub-web && npm run build && npm test -->
- [ ] Finalize `shared/specs/skill-manifest-spec-v1.md` → `shared/schemas/skill-manifest.schema.json` (validates the spec).
- [ ] Scaffold `apps/skillhub-cli` (Rust, clap). Implement `search`, `info`, `install` (writes skill into detected harness skills dir; writes `skillhub.lock.json`), `update`, `remove`.
- [ ] Implement harness detection: Claude Code, Codex, Cursor, Gemini CLI, Copilot, pi, OpenClaw (env vars + config paths per harness).
- [ ] Implement `verify` subcommand: downloads package, runs the security scanner, reports per-check results.
- [ ] Implement security scanner (`apps/skillhub-cli/src/scan/`): static checks — prompt-injection markers, dangerous shell/network calls, exfiltration URLs, encoded payloads, unexpected binary blobs. 24+ rules. Includes 3 seeded malicious test packages (fixtures) that must all be flagged.
- [ ] Scaffold `apps/skillhub-registry` (Rust, axum + SQLite): publish (from git repo + manifest), version listing, download counting, search API.
- [ ] Scaffold `apps/skillhub-web` (Next.js): search, skill pages (install command, per-harness compatibility badges, verified badge, install counts), publish instructions.
- [ ] End-to-end test: publish a fixture skill from a local git repo → search → install into a temp harness dir → verify.
- **Exit criteria:** CLI installs a real skill into a temp harness; scanner flags all 3 malicious fixtures; registry serves search; web site lists the published skill with badges.

## Phase 4: SlopGate — rules, score, CI action, dashboard <!-- PENDING --> <!-- DEPENDS_ON: Phase 3 -->
<!-- VALIDATE: bash scripts/plan-lock.sh verify && npm test && cd apps/slopgate-action && npm run build -->
- [ ] Scaffold `apps/slopgate` (TypeScript). Implement deterministic rule pack (TS/JS): dead abstractions, unused helpers, cargo-cult comments, generic naming, over-engineering patterns, boilerplate commits/PR text, "as an AI" phrasing. 30+ rules; each rule has fixture tests.
- [ ] Implement `slop` CLI: `slop scan <path>` (files), `slop score` (0–100 with per-rule breakdown), `slop lint` exit codes for CI.
- [ ] Implement LLM review layer (bring-your-own-key): scores prose/commit/PR-description slop using a pattern catalog; disabled when no key present (deterministic core always works).
- [ ] Scaffold `apps/slopgate-action` (GitHub Action): runs scan, posts inline comments + summary, supports `threshold` input, writes SARIF artifact, fails CI above threshold (input `block: true`).
- [ ] Seed 3 fixture repos (clean / mildly sloppy / heavily sloppy) in `apps/slopgate/fixtures/`; assert score ordering and threshold gating in tests.
- [ ] Scaffold `apps/slopgate-dash` (Next.js): per-repo score history + trend line (reads check artifacts; optional hosted API later).
- **Exit criteria:** rule pack passes fixture tests; sloppy fixture scores high and clean low; action fails CI at threshold; dashboard renders a trend from a recorded artifact.

## Phase 5: DeskAgent — self-memory core <!-- PENDING --> <!-- DEPENDS_ON: Phase 4 -->
<!-- VALIDATE: bash scripts/plan-lock.sh verify && cd apps/deskagent && npm test && cargo check -->
- [ ] Create `shared/schemas/memory-event.schema.json`: four memory kinds (episodic, semantic, procedural, working) with sources, confidence, timestamps, and project scope; include validation tests.
- [ ] Scaffold `apps/deskagent` (Tauri 2 + React + TypeScript). Window shell, chat UI, session persistence (SQLite).
- [ ] Implement memory store (SQLite + local embeddings via sqlite-vec/fastembed-rs): episodic log, semantic facts, procedural records, working context; encrypted at rest; export/delete APIs.
- [ ] Implement capture pipeline: every conversation appended as raw episodes; extraction pass distills facts/preferences every N turns (default 5, max 20 memories per pass).
- [ ] Implement consolidation & persona: regenerate the persona model every N new memories (default 50); dedupe + conflict detection + decay; reflection on local models by default, per-session opt-in cloud for heavy passes (DEC-0005, DEC-0009).
- [ ] Implement hybrid retrieval: keyword + embedding recall (RRF fusion) with strict injection budget; companion-level + per-project scoping (both scopes per DEC-0009).
- [ ] Implement propose-to-remember approval cards: every memory write routes through the sandbox approval system; approvals and rejections recorded as learning signal.
- [ ] Implement memory UX: memory explorer (timeline/facts/projects — browse, edit, pin, delete, export) and persona card view.
- **Exit criteria:** memory schema validated; store encrypted and deletable; pipeline extracts memories from a fixture conversation; persona regenerates; retrieval returns scoped hits; memory writes require approval; explorer and persona card render.

## Phase 6: DeskAgent — runtime, skills, sandbox <!-- PENDING --> <!-- DEPENDS_ON: Phase 5 -->
<!-- VALIDATE: bash scripts/plan-lock.sh verify && cd apps/deskagent && npm test && cargo check -->
- [ ] Implement runtime layer: Ollama/llama.cpp backend adapter + Metal path on Apple Silicon (TurboFieldfare-compatible); model registry.
- [ ] Implement model picker consuming BenchKit data (`shared/lib/will-it-run.mjs`): shows "runs on your machine" per model; offline fallback to bundled dataset.
- [ ] Implement skill integration: install/update skills from SkillHub registry in-app (manifest spec + lockfile format); skills surface as procedural memory.
- [ ] Implement action sandbox: tool calls render as approval cards; risky actions (shell, file writes, network) require click-to-approve; full undo log (shared with memory-write approvals).
- [ ] Wire memory into conversation: persona + scoped memories injected into chat context; "I remember…" citations with sources.
- [ ] Add voice input path (Whisper/WebRTC stub acceptable at P0) and a scheduled-tasks placeholder.
- **Exit criteria:** app chats with a local model; picker reflects BenchKit data; agent recalls facts/preferences across sessions with citations; skills install and invoke; risky actions and memory writes blocked until approved; undo log records actions.

## Phase 7: Synergies, validation, and launch <!-- PENDING --> <!-- DEPENDS_ON: Phase 6 -->
<!-- VALIDATE: bash scripts/plan-lock.sh verify && bash scripts/run-all-checks.sh && bash scripts/plan-lock.sh status -->
- [ ] Create `scripts/run-all-checks.sh`: runs every product's test suite, lints, and schema/dataset validation; single exit code.
- [ ] Wire BenchKit API into DeskAgent model picker (live fetch with cached fallback).
- [ ] Wire SlopGate scanner into SkillHub `verify` as an optional quality check; add quality score to skill pages.
- [ ] Add cross-links and "ecosystem" landing section in each product's README.
- [ ] Write demo scripts: BenchKit dataset + calculator demo; SkillHub install-from-registry demo; SlopGate PR-gate demo; DeskAgent skill-install + approval demo.
- [ ] Full validation pass: run `run-all-checks.sh`, fix all failures, re-run.
- [ ] Write final handoff: `records/final-handoff.md` with completion state, validations run, residual risks, and next actions for each product.
- **Exit criteria:** all checks green on a clean clone (`git clone` → `run-all-checks.sh`); four demos runnable from README; handoff written; milestone acceptance claimed.

---

# Definition of Done (Milestone 1)

- All phases `COMPLETE`; zero tasks cancelled without documented reason in PROGRESS.md.
- `run-all-checks.sh` green from a fresh clone.
- BenchKit: ≥ 4 seeded hardware/model configurations, calculator matches them.
- SkillHub: install/search/update/verify work end-to-end; scanner flags all malicious fixtures.
- SlopGate: fixtures ordered correctly; action gates CI at threshold.
- DeskAgent: launches and chats locally with memory — recalls facts/preferences across sessions; persona card reviewable/correctable; memory browsable/exportable/deletable; memory writes approval-gated; installs a skill; enforces approvals.
- `plan-lock.sh verify` passes at every checkpoint; PROGRESS.md contains per-phase handoffs.
