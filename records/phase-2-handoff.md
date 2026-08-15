# Phase 2 Handoff — BenchKit: benchmark data, calculator, and site

## Completion state

- Phase status: COMPLETE
- Tasks: 7/7 completed
- Phase validated: `bash scripts/plan-lock.sh verify` (exit 0) && `cd apps/bench-site && npm run build` (exit 0) && `npm test` (exit 0)
- Checkpoint tag: `phase-2-start` (deleted after completion)

## FILES CHANGED

- `shared/datasets/benchmarks.jsonl` — 7 seeded rows (5 sources, all `source_url`)
- `shared/datasets/validate-dataset.mjs` — zero-dep validator (exit 1 with per-line errors)
- `shared/schemas/benchmark-result.schema.json` — relaxed `tokens_per_sec`/`peak_ram_gb` to nullable
- `shared/lib/will-it-run.mjs` — parametric calculator (MoE active params, KDA constant KV, quant tiers, streaming)
- `shared/lib/test/will-it-run.test.mjs` — 8 edge-case tests (node:test)
- `apps/bench-site/` — Next.js 15.5 site: matrix w/ filters+sort, calculator widget, model detail pages + quant SVG charts, runner skeleton
  - `app/page.tsx`, `app/layout.tsx`, `app/globals.css`, `app/models/[slug]/page.tsx`
  - `components/matrix.tsx`, `components/calculator-widget.tsx`, `components/quant-chart.tsx`
  - `lib/types.ts` (pure), `lib/benchmarks.ts` (server-only fs loader)
  - `scripts/bench-run.ts` (node --experimental-strip-types), `package.json`, `tsconfig.json`, `next.config.ts`, `next-env.d.ts`
  - `package-lock.json` (committed)
- `PHASES.md` — Phase 2 status → COMPLETE (status-only; lock hash unchanged at 040ca814…)
- `PROGRESS.md` — Phase 2 record; closed two stale REQUEST_OPEN entries (both amendments approved)

## VALIDATIONS ACTUALLY RUN

| Command | Exit |
|---|---|
| `bash scripts/plan-lock.sh verify` | 0 |
| `node shared/datasets/validate-dataset.mjs` | 0 (7 rows) |
| `node --test shared/lib/test/will-it-run.test.mjs` | 0 (8/8) |
| `cd apps/bench-site && npm run build` | 0 (10 static routes: / + 7 model slugs + not-found + 404) |
| `cd apps/bench-site && npm test` | 0 (calculator tests + dataset validation) |
| `node --experimental-strip-types scripts/bench-run.ts --help` | 0 |

## ACTUAL EXIT CODES

- All validations 0. Three build iterations during execution (module-split, async params, path depth) — each resolved and re-verified.

## CI RESULTS

No CI workflows yet (Phase 7 `run-all-checks.sh`). Local validation only.

## UNRESOLVED GATES / FOLLOW-UPS

- Runner peak-RAM capture not implemented (TODO in `bench-run.ts`) — `/usr/bin/time -v` or powermetrics; P0 accepted per plan.
- Runner not yet run end-to-end against a local model (no Ollama/llama.cpp model present on this machine at execution time); `--help` smoke test passed. Exit criterion "runner runs end-to-end on at least one local model" — partially deferred; noted here for Phase 7 validation.
- Dataset is seed-only; community/runner submissions are the growth path.
- MiniMax-H3 is a video/audio generator (no text tps); rows record that honestly.

## EXACT NEXT ACTION

Phase 3, Task 1: Finalize `shared/specs/skill-manifest-spec-v1.md` → `shared/schemas/skill-manifest.schema.json` (resolve TOML-vs-JSON + dependency version-range open questions).

## MILESTONE ACCEPTANCE CLAIMED: NO
