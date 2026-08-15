# slopgate-dash

Per-repo SlopGate score history and trend lines, rendered from **recorded check
artifacts** (`data/history.json`). Higher score = worse.

Zero chart dependencies — the trend line is a hand-rolled SVG component
(`components/trend-chart.tsx`), same approach as BenchKit's quant charts.

## Pages

- `/` — overview table: latest score per repo, findings, trend direction (▲ worse / ▼ better).
- `/repos/<name>` — per-repo detail: stats cards, SVG trend line with a threshold-50
  gridline, and the full run log (date · score · findings · sha).

## How runs get recorded

The artifact is written by `scripts/record-run.mjs`, which shells out to the real
`slop score --json` CLI and appends a run for a repo (same-day identical runs are
deduped):

```bash
# record the seeded fixtures (default paths resolve into apps/slopgate/fixtures/)
node scripts/record-run.mjs --repo fixture-heavy
node scripts/record-run.mjs --repo fixture-mild
node scripts/record-run.mjs --repo fixture-clean

# record any other path
node scripts/record-run.mjs --repo my-service --path ../my-service/src --url https://github.com/acme/my-service --sha abc1234
```

Override the artifact location with `SLOPGATE_DATA_FILE`. In CI, the
`slopgate-action` step (or a cron) runs this after each scan and the dashboard
picks up the new point on the next build.

## Develop

```bash
npm install
npm run dev       # http://localhost:3000
npm run build     # static prerender: /, /repos/fixture-{clean,mild,heavy}
npm test          # data-contract tests (schema, ordering, aggregation, real scan values)
```
