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
    // Use the row's real active-parameter count (or a conservative default) instead
    // of a hardcoded 8B, so fit/speed math is not uniformly wrong for non-8B models.
    const activeParamsB = row.active_params_b ?? 8;
    const est = estimate({ name: row.model, totalParamsB: activeParamsB, bitsPerWeight: bits }, machine);
    const verdict = row.peak_ram_gb != null
      ? (row.peak_ram_gb <= machine.ramGb ? "fits" : est.verdict)
      : est.verdict;
    return {
      model: row.model,
      runtime: row.runtime,
      hardwareLabel: row.hardware?.gpu ? `${row.hardware.cpu} / ${row.hardware.gpu}` : row.hardware?.cpu ?? "unknown",
      verdict,
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

// ---- live BenchKit source with cached fallback (Phase 7 Task 2) -----------------

export const BENCHKIT_LIVE_URL =
  "https://raw.githubusercontent.com/MerverliPy/agent-ecosystem/main/shared/datasets/benchmarks.jsonl";

function parseJsonl(text: string): BenchRow[] {
  const rows: BenchRow[] = [];
  for (const line of text.split("\n")) {
    const l = line.trim();
    if (!l) continue;
    try {
      rows.push(JSON.parse(l) as BenchRow);
    } catch {
      // skip malformed lines rather than failing the whole fetch
    }
  }
  return rows;
}

function toBenchRow(raw: Record<string, unknown>): BenchRow | null {
  if (typeof raw.model !== "string" || typeof raw.runtime !== "string" || typeof raw.source_url !== "string") {
    return null;
  }
  const h = (raw.hardware ?? {}) as Record<string, unknown>;
  return {
    model: raw.model,
    runtime: raw.runtime,
    quantization: (raw.quantization as string | null | undefined) ?? null,
    tokens_per_sec: (raw.tokens_per_sec as number | null | undefined) ?? null,
    peak_ram_gb: (raw.peak_ram_gb as number | null | undefined) ?? null,
    active_params_b: (raw.active_params_b as number | null | undefined) ?? null,
    disk_size_gb: (raw.disk_size_gb as number | null | undefined) ?? null,
    source_url: raw.source_url,
    hardware: {
      cpu: (h.cpu as string | undefined) ?? "unknown",
      ram_gb: (h.ram_gb as number | null | undefined) ?? null,
      gpu: (h.gpu as string | null | undefined) ?? null,
      os: (h.os as string | null | undefined) ?? null,
    },
  };
}

let liveCache: BenchRow[] | null = null;

/** Test hook: clear the in-memory live-cache. */
export function resetLiveCache(): void {
  liveCache = null;
}

/**
 * Fetch the live BenchKit dataset; on any failure (offline, 404, bad rows) fall back
 * to the bundled catalog (offline fallback, DEC-0005). `fetchImpl` is injectable for
 * tests. Rows that fail the shape check are dropped rather than failing the fetch.
 */
export async function loadLiveCatalog(fetchImpl: typeof fetch = fetch): Promise<{ rows: BenchRow[]; source: "live" | "bundled"; error?: string }> {
  if (liveCache) return { rows: liveCache, source: "live" };
  try {
    const res = await fetchImpl(BENCHKIT_LIVE_URL);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const text = await res.text();
    const rows = parseJsonl(text)
      .map((r) => toBenchRow(r as unknown as Record<string, unknown>))
      .filter((r): r is BenchRow => r !== null);
    if (rows.length === 0) throw new Error("no valid rows");
    liveCache = rows;
    return { rows, source: "live" };
  } catch (err) {
    return {
      rows: BENCHKIT_CATALOG,
      source: "bundled",
      error: err instanceof Error ? err.message : String(err),
    };
  }
}

export { BENCHKIT_CATALOG };
