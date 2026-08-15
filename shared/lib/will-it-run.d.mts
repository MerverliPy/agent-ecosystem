// Ambient type declarations for will-it-run.mjs (plain ESM, no TS declarations).
// Consumed by DeskAgent's model picker (and any TS consumer).
export interface ModelSpec {
  name: string;
  totalParamsB: number;
  activeParamsB?: number;
  bitsPerWeight?: number;
  kvBytesPerToken?: number;
  constantKvBytesGb?: number;
}
export interface MachineSpec {
  ramGb: number;
  memBandwidthGbPerSec?: number;
  baseOverheadGb?: number;
  contextTokens?: number;
  streamingSupported?: boolean;
  utilization?: number;
}
export function estimate(
  model: ModelSpec,
  machine: MachineSpec
): {
  verdict: "fits" | "streams-needed" | "no-fit";
  ramNeededGb: number;
  weightsGb: number;
  kvGb: number;
  estTokensPerSec: number | null;
  assumptions: string[];
};
export function round2(n: number): number;
