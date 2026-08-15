// will-it-run — parametric estimate of whether a model fits on a machine, and rough speed.
// Shared library used by apps/bench-site (matrix + calculator) and later DeskAgent's model picker.
// Assumptions are returned alongside every result; measured rows from the dataset are preferred
// over these estimates (which are optimistic upper bounds).

/**
 * @typedef {{name: string, totalParamsB: number, activeParamsB?: number, bitsPerWeight?: number,
 *            kvBytesPerToken?: number, constantKvBytesGb?: number}} ModelSpec
 * @typedef {{ramGb: number, memBandwidthGbPerSec?: number, baseOverheadGb?: number,
 *            contextTokens?: number, streamingSupported?: boolean, utilization?: number}} MachineSpec
 */

export const DEFAULT_BITS = 4; // common local quant (MXFP4 / Q4)
export const DEFAULT_KV_BYTES_PER_TOKEN = 0.0024; // ~2.4 MB/pos (Kimi K3 MLA scale)
export const DEFAULT_BASE_OVERHEAD_GB = 1.5; // runtime + OS + embeddings
export const DEFAULT_UTILIZATION = 0.1; // sustained decode rarely exceeds ~10% of raw bandwidth

/**
 * @param {ModelSpec} model
 * @param {MachineSpec} machine
 * @returns {{verdict: "fits"|"streams-needed"|"no-fit", ramNeededGb: number, weightsGb: number,
 *            kvGb: number, estTokensPerSec: number|null, assumptions: string[]}}
 */
export function estimate(model, machine) {
  if (!model || typeof model !== "object") throw new Error("model spec required");
  if (!machine || typeof machine !== "object") throw new Error("machine spec required");
  if (!(model.totalParamsB > 0)) throw new Error("totalParamsB must be > 0");
  if (!(machine.ramGb > 0)) throw new Error("ramGb must be > 0");

  const active = model.activeParamsB ?? model.totalParamsB;
  if (!(active > 0)) throw new Error("activeParamsB must be > 0");
  if (active > model.totalParamsB) throw new Error("activeParamsB cannot exceed totalParamsB (MoE check)");

  const bits = model.bitsPerWeight ?? DEFAULT_BITS;
  if (!(bits > 0)) throw new Error("bitsPerWeight must be > 0");

  const weightsBytes = model.totalParamsB * 1e9 * (bits / 8);
  const weightsGb = weightsBytes / 1e9;

  const ctx = machine.contextTokens ?? 4096;
  if (!(ctx > 0)) throw new Error("contextTokens must be > 0");

  // KDA-style constant-state models (e.g. Kimi K3): KV does not grow with context.
  const kvGb = model.constantKvBytesGb ?? (ctx * (model.kvBytesPerToken ?? DEFAULT_KV_BYTES_PER_TOKEN));

  const overhead = machine.baseOverheadGb ?? DEFAULT_BASE_OVERHEAD_GB;
  const ramNeededGb = weightsGb + kvGb + overhead;

  const avail = machine.ramGb;
  let verdict;
  if (ramNeededGb <= avail) verdict = "fits";
  else if (machine.streamingSupported === true) verdict = "streams-needed";
  else verdict = "no-fit";

  // Speed: bytes of weights touched per decoded token ≈ active params × bits.
  const decodeBytes = active * 1e9 * (bits / 8);
  const bw = machine.memBandwidthGbPerSec ?? 100;
  if (!(bw > 0)) throw new Error("memBandwidthGbPerSec must be > 0");
  const util = machine.utilization ?? DEFAULT_UTILIZATION;
  const estTokensPerSec = decodeBytes > 0 ? (bw * 1e9 * util) / decodeBytes : null;

  const assumptions = [
    `weights at ${bits} bits/param: ${weightsGb.toFixed(0)} GB`,
    `KV cache: ${model.constantKvBytesGb ? "constant (" + model.constantKvBytesGb + " GB, KDA-style)" : ctx + " tokens × " + (model.kvBytesPerToken ?? DEFAULT_KV_BYTES_PER_TOKEN) + " GB/pos"}`,
    `base overhead: ${overhead} GB`,
    `speed is an optimistic estimate (utilization ${Math.round(util * 100)}%); prefer measured rows`,
  ];

  return {
    verdict,
    ramNeededGb: round2(ramNeededGb),
    weightsGb: round2(weightsGb),
    kvGb: round2(kvGb),
    estTokensPerSec: estTokensPerSec ? round2(estTokensPerSec) : null,
    assumptions,
  };
}

export function round2(n) {
  return Math.round(n * 100) / 100;
}
