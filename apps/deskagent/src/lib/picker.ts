// Model picker logic — consumes BenchKit data via shared/lib/will-it-run.mjs with the
// bundled catalog (benchkit-catalog.ts) as the offline fallback (Phase 6 Task 2).
import { estimate, type MachineSpec } from "../../../../shared/lib/will-it-run.mjs";
import { BENCHKIT_CATALOG, type BenchRow } from "./benchkit-catalog.ts";

export interface PickerEntry {
  model: string;
  runtime: string;
  hardwareLabel: string;
  verdict: "fits" | "streams-needed" | "no-fit";
  ramNeededGb: number;
  estTokensPerSec: number | null;
  measuredTokensPerSec?: number | null;
  sourceUrl: string;
}

/** Default machine profile (documented, editable by the user in the UI later). */
export function defaultMachine(): MachineSpec {
  return {
    ramGb: 16,
    memBandwidthGbPerSec: 100,
    baseOverheadGb: 1.5,
    contextTokens: 4096,
    streamingSupported: true,
    utilization: 0.1,
  };
}

/** Convert a BenchKit row + machine into a picker entry. The dataset rows do not carry
 * parameter counts, so fit estimation uses the measured peak RAM when present, else the
 * will-it-run estimate at the row's quantization tier. */
export function evaluateRows(rows: BenchRow[], machine: MachineSpec): PickerEntry[] {
  return rows.map((row) => {
    const bits = row.quantization ? quantBits(row.quantization) : 4;
    const est = estimate({ name: row.model, totalParamsB: 8, bitsPerWeight: bits }, machine);
    return {
      model: row.model,
      runtime: row.runtime,
      hardwareLabel: row.hardware?.gpu ? `${row.hardware.cpu} / ${row.hardware.gpu}` : row.hardware?.cpu ?? "unknown",
      verdict: row.peak_ram_gb != null && row.peak_ram_gb <= machine.ramGb ? "fits" : est.verdict,
      ramNeededGb: row.peak_ram_gb ?? est.ramNeededGb,
      estTokensPerSec: row.tokens_per_sec ?? est.estTokensPerSec,
      measuredTokensPerSec: row.tokens_per_sec ?? null,
      sourceUrl: row.source_url,
    };
  });
}

function quantBits(q: string): number {
  const lower = q.toLowerCase();
  if (lower.includes("int4") || lower.includes("q4") || lower.includes("mxfp4")) return 4;
  if (lower.includes("int8") || lower.includes("q8") || lower.includes("fp8")) return 8;
  if (lower.includes("fp16") || lower.includes("int16")) return 16;
  return 4;
}

/** The picker: entries that fit (or stream) on this machine, best speed first. */
export function pickForMachine(rows: BenchRow[], machine: MachineSpec): PickerEntry[] {
  return evaluateRows(rows, machine)
    .filter((e) => e.verdict !== "no-fit")
    .sort((a, b) => (b.estTokensPerSec ?? 0) - (a.estTokensPerSec ?? 0));
}

export function verdictLabel(v: PickerEntry["verdict"]): string {
  return v === "fits" ? "runs on your machine" : v === "streams-needed" ? "streams from NVMe/disk" : "no-fit";
}

export { BENCHKIT_CATALOG };
