"use client";

import { useState } from "react";
import { estimate } from "../../../shared/lib/will-it-run.mjs";

// Registry of model specs for the calculator. Measured rows in the dataset always take precedence;
// these are the parameters the estimate needs.
const MODEL_SPECS: Record<string, { totalParamsB: number; activeParamsB: number; bitsPerWeight: number; constantKvBytesGb?: number }> = {
  "Kimi K3 (2.78T)": { totalParamsB: 2780, activeParamsB: 104, bitsPerWeight: 4, constantKvBytesGb: 0.626 },
  "Gemma 4 26B-A4B": { totalParamsB: 26, activeParamsB: 4, bitsPerWeight: 4 },
  "MiniMax-H3 (Omni 33B)": { totalParamsB: 33, activeParamsB: 33, bitsPerWeight: 16 }, // BF16 dense
};

export default function CalculatorWidget() {
  const [modelKey, setModelKey] = useState("Gemma 4 26B-A4B");
  const [ramGb, setRamGb] = useState("16");
  const [bw, setBw] = useState("120");
  const [ctx, setCtx] = useState("8192");
  const [streaming, setStreaming] = useState(true);

  const spec = MODEL_SPECS[modelKey];
  let result: ReturnType<typeof estimate> | null = null;
  let error: string | null = null;
  try {
    result = estimate(
      { name: modelKey, ...spec },
      { ramGb: Number(ramGb), memBandwidthGbPerSec: Number(bw), contextTokens: Number(ctx), streamingSupported: streaming }
    );
  } catch (e) {
    error = e instanceof Error ? e.message : String(e);
  }

  return (
    <div className="panel">
      <h2>Will it run on my machine?</h2>
      <label>Model</label>
      <select value={modelKey} onChange={(e) => setModelKey(e.target.value)}>
        {Object.keys(MODEL_SPECS).map((m) => (
          <option key={m}>{m}</option>
        ))}
      </select>

      <div className="grid-2">
        <div>
          <label>RAM (GB)</label>
          <input type="number" value={ramGb} min="1" onChange={(e) => setRamGb(e.target.value)} />
        </div>
        <div>
          <label>Memory bandwidth (GB/s, ≈ model class)</label>
          <input type="number" value={bw} min="1" onChange={(e) => setBw(e.target.value)} />
        </div>
      </div>

      <label>Context tokens</label>
      <input type="number" value={ctx} min="1" onChange={(e) => setCtx(e.target.value)} />

      <label style={{ display: "flex", gap: 6, alignItems: "center", marginTop: 12 }}>
        <input type="checkbox" checked={streaming} onChange={(e) => setStreaming(e.target.checked)} />
        Runtime supports weight streaming (MoE from disk/NVMe)
      </label>

      {error && <p className="note">Input error: {error}</p>}
      {result && (
        <>
          <p className={`verdict ${result.verdict}`}>
            Verdict: {result.verdict === "fits" ? "fits" : result.verdict === "streams-needed" ? "needs streaming" : "does not fit"}
          </p>
          <table>
            <tbody>
              <tr><td>Estimated RAM needed</td><td className="num">{result.ramNeededGb} GB</td></tr>
              <tr><td>Weights at {spec.bitsPerWeight} bits/param</td><td className="num">{result.weightsGb} GB</td></tr>
              <tr><td>KV cache</td><td className="num">{result.kvGb} GB</td></tr>
              <tr><td>Estimated decode speed</td><td className="num">~{result.estTokensPerSec} tok/s (optimistic)</td></tr>
            </tbody>
          </table>
          <ul className="assumptions">
            {result.assumptions.map((a, i) => (
              <li key={i}>{a}</li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
