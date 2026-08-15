# BenchKit

Local-inference benchmark data, a "will it run?" calculator, and a searchable matrix site.

- **Dataset** — `../shared/datasets/benchmarks.jsonl` (7 rows, every row carries a `source_url`, DEC-0006).
- **Calculator** — `../shared/lib/will-it-run.mjs` (RAM estimate = weights + KV cache + overhead; speed from memory bandwidth × active params; verdicts fits / streams-needed / no-fit).
- **Site** — Next.js matrix with filters, sortable columns, model detail pages + quant-quality charts, and a `bench-run.ts` runner skeleton.

## Ecosystem

BenchKit feeds the rest of the monorepo: DeskAgent's model picker consumes the
dataset + calculator (`shared/lib/will-it-run.mjs`) with an offline bundled catalog;
SlopGate, SkillHub and DeskAgent all validate against `shared/` schemas.
Run checks: `npm test && npm run build`.
