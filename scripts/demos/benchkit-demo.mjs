// BenchKit demo — dataset + will-it-run calculator.
// Usage: node scripts/demos/benchkit-demo.mjs
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { estimate } from "../../shared/lib/will-it-run.mjs";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const dataset = readFileSync(path.join(root, "shared/datasets/benchmarks.jsonl"), "utf8")
  .split("\n")
  .map((l) => l.trim())
  .filter(Boolean)
  .map((l) => JSON.parse(l));

console.log("== BenchKit demo ==");
console.log(`dataset: ${dataset.length} rows (every row carries a source_url, DEC-0006)\n`);

for (const row of dataset) {
  console.log(`- ${row.model} @ ${row.runtime} — ${row.tokens_per_sec ?? "?"} tok/s, ${row.peak_ram_gb ?? "?"} GB peak`);
  console.log(`    source: ${row.source_url}`);
}

console.log("\n== will-it-run calculator ==");
const machine = { ramGb: 16, memBandwidthGbPerSec: 100, streamingSupported: true };
for (const model of [
  { name: "kimi-k3-in-c", totalParamsB: 2780, activeParamsB: 103, bitsPerWeight: 4, constantKvBytesGb: 0.626 },
  { name: "gemma-4-26b-a4b", totalParamsB: 26, activeParamsB: 4, bitsPerWeight: 4 },
]) {
  const r = estimate(model, machine);
  console.log(`${model.name}: ${r.verdict} — needs ${r.ramNeededGb} GB, ~${r.estTokensPerSec ?? "?"} tok/s`);
  for (const a of r.assumptions) console.log(`    · ${a}`);
}
console.log("\nBenchKit demo done.");
