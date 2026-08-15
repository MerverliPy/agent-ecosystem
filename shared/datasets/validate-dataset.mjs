#!/usr/bin/env node
// Validate shared/datasets/benchmarks.jsonl against the benchmark-result schema semantics.
// Zero-dependency: hand-rolled structural checks (no ajv). Exits 1 listing every error.
// Usage: node shared/datasets/validate-dataset.mjs [path]
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const datasetPath = process.argv[2] ?? path.join(here, "benchmarks.jsonl");

const ALLOWED_FITS = new Set(["fits", "streams-needed", "no-fit", null]);
const ERRORS = [];

function err(lineNo, msg) {
  ERRORS.push(`line ${lineNo}: ${msg}`);
}

function isNum(x) { return typeof x === "number" && Number.isFinite(x); }

const raw = readFileSync(datasetPath, "utf8");
const lines = raw.split("\n").filter((l) => l.trim().length > 0);

if (lines.length === 0) {
  err(0, "dataset is empty");
  process.exit(1);
}

lines.forEach((line, i) => {
  const n = i + 1;
  let row;
  try {
    row = JSON.parse(line);
  } catch (e) {
    err(n, `invalid JSON: ${e.message}`);
    return;
  }

  // required
  for (const k of ["model", "hardware", "runtime", "tokens_per_sec", "peak_ram_gb", "source_url", "submitted_at"]) {
    if (!(k in row)) err(n, `missing required field "${k}"`);
  }
  if (typeof row.model !== "string" || row.model.length === 0) err(n, "model must be a non-empty string");
  if (typeof row.runtime !== "string" || row.runtime.length === 0) err(n, "runtime must be a non-empty string");

  // hardware
  if (row.hardware && typeof row.hardware === "object") {
    if (typeof row.hardware.cpu !== "string") err(n, "hardware.cpu must be a string (required)");
    if (row.hardware.ram_gb != null && !isNum(row.hardware.ram_gb)) err(n, "hardware.ram_gb must be a number or null");
    if (row.hardware.gpu != null && typeof row.hardware.gpu !== "string") err(n, "hardware.gpu must be a string or null");
  } else {
    err(n, "hardware must be an object");
  }

  // numeric / nullable fields
  for (const k of ["tokens_per_sec", "peak_ram_gb", "disk_size_gb", "active_params_b", "quality_delta"]) {
    if (row[k] != null && !isNum(row[k])) err(n, `"${k}" must be a number or null`);
    if (isNum(row[k]) && row[k] < 0) err(n, `"${k}" cannot be negative`);
  }
  if (isNum(row.tokens_per_sec) && row.tokens_per_sec === 0) err(n, "tokens_per_sec of 0 is not meaningful; use null");

  // enums
  if (!ALLOWED_FITS.has(row.fits)) err(n, `fits must be one of fits|streams-needed|no-fit|null, got ${JSON.stringify(row.fits)}`);
  if (row.quantization != null && typeof row.quantization !== "string") err(n, "quantization must be a string or null");

  // DEC-0006: source_url present and http(s)
  if (typeof row.source_url !== "string" || !/^https?:\/\//.test(row.source_url)) err(n, "source_url must be an http(s) URL (DEC-0006)");
  if (typeof row.submitted_at !== "string" || Number.isNaN(Date.parse(row.submitted_at))) err(n, "submitted_at must be an ISO date string");
});

if (ERRORS.length > 0) {
  console.error(`DATASET-FAIL: ${ERRORS.length} error(s) in ${path.basename(datasetPath)}:`);
  for (const e of ERRORS) console.error("  " + e);
  process.exit(1);
}
console.log(`DATASET-OK: ${lines.length} rows validated (${path.basename(datasetPath)})`);
