#!/usr/bin/env node
/**
 * BenchKit runner skeleton — measure a local model and append a row to the shared dataset.
 *
 * Usage:
 *   node scripts/bench-run.ts --model "My Model" --runtime llama.cpp \
 *     --command "llama-cli -m model.gguf -p 'Hello' -n 50" [--quant Q4_K_M]
 *
 * Hardware is auto-detected. Output rows carry source_url "runner:local" and are appended to
 * ../../shared/datasets/benchmarks.jsonl (must still pass validate-dataset.mjs — fill in what
 * the automated parse cannot detect: peak RAM, tokens count if the output doesn't state them).
 *
 * This is a P0 skeleton: it measures wall time and parses simple tokens/s patterns. Peak RAM
 * capture (e.g. /usr/bin/time -v or powermetrics) is the Phase 2 follow-up.
 */
import { execFileSync } from "node:child_process";
import { appendFileSync, existsSync } from "node:fs";
import { cpus, totalmem } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const datasetPath = path.join(here, "..", "..", "..", "shared", "datasets", "benchmarks.jsonl");

function arg(name: string): string | null {
  const i = process.argv.indexOf("--" + name);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : null;
}

if (process.argv.includes("--help") || process.argv.length < 6) {
  console.log(`usage: node scripts/bench-run.ts --model "<name>" --runtime "<runtime>" --command "<cmd>" [--quant <q>]`);
  process.exit(0);
}

const model = arg("model") as string | null;
const runtime = arg("runtime") as string | null;
const command = arg("command") as string | null;
const quant = arg("quant") as string | null;

if (!model || !runtime || !command) {
  console.error("missing required args (--model, --runtime, --command)");
  process.exit(2);
}

const cpu = cpus()[0]?.model ?? "unknown CPU";
const cores = cpus().length;
const ramGb = Math.round(totalmem() / 1e9);
console.log(`hardware: ${cpu} (${cores} cores), ${ramGb} GB RAM`);

console.log(`running: ${command}`);
const t0 = performance.now();
let out: string;
try {
  out = execFileSync("bash", ["-lc", command], { encoding: "utf8", timeout: 120000, maxBuffer: 64 * 1024 * 1024 });
} catch (e) {
  console.error(`command failed: ${(e as Error).message?.slice(0, 300)}`);
  process.exit(1);
}
const elapsedS = (performance.now() - t0) / 1000;

// Parse tokens/sec patterns like "123.45 tokens/s", "123.45 tok/s", or "<n> tokens" totals.
const tpsMatch = out.match(/(\d+(?:\.\d+)?)\s*(?:tokens?|tok)\/s/);
const countMatch = out.match(/(\d+)\s+tokens?/);
let tokensPerSec: number | null = tpsMatch ? Number(tpsMatch[1]) : null;
if (!tokensPerSec && countMatch) tokensPerSec = Math.round((Number(countMatch[1]) / elapsedS) * 100) / 100;

const row = {
  model,
  hardware: { cpu: `${cpu} (${cores} cores)`, ram_gb: ramGb, os: process.platform },
  runtime,
  quantization: quant ?? null,
  tokens_per_sec: tokensPerSec,
  peak_ram_gb: null, // TODO(Phase 2 follow-up): /usr/bin/time -v or powermetrics capture
  disk_size_gb: null,
  active_params_b: null,
  quality_delta: null,
  fits: null,
  source_url: "runner:local",
  submitted_at: new Date().toISOString(),
};

if (tokensPerSec) console.log(`measured: ${tokensPerSec} tokens/s (${elapsedS.toFixed(1)}s)`);
else console.log(`could not auto-parse tokens/s from output — set it manually in the appended row`);

const line = JSON.stringify(row);
appendFileSync(datasetPath, "\n" + line);
console.log(`appended row to ${datasetPath}`);
console.log(`validate: node shared/datasets/validate-dataset.mjs${existsSync(datasetPath) ? "" : ""}`);
