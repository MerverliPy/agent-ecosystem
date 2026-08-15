// Server-only dataset loader (uses node:fs). Import only from server components/pages.
import { readFileSync } from "node:fs";
import path from "node:path";
import type { BenchmarkRow } from "./types";

const DATASET_PATH = path.join(process.cwd(), "..", "..", "shared", "datasets", "benchmarks.jsonl");

export function loadRows(): BenchmarkRow[] {
  const raw = readFileSync(DATASET_PATH, "utf8");
  return raw
    .split("\n")
    .filter((l) => l.trim().length > 0)
    .map((l) => JSON.parse(l) as BenchmarkRow);
}
