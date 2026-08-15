// Quantization-quality bars: relative error vs. unquantized, from rows with quality_delta.
// Zero-dependency inline SVG. Server-safe (no hooks).
import type { BenchmarkRow } from "../lib/types";

export default function QuantChart({ rows }: { rows: BenchmarkRow[] }) {
  const withDelta = rows.filter((r) => r.quality_delta != null);
  if (withDelta.length === 0) {
    return <p className="note">No quantization-quality measurements for this model yet.</p>;
  }
  const max = Math.max(...withDelta.map((r) => r.quality_delta as number), 1);
  return (
    <div>
      {withDelta.map((r, i) => {
        const v = r.quality_delta as number;
        const w = Math.max(2, Math.round((v / max) * 100));
        return (
          <div key={i} style={{ marginBottom: 12 }}>
            <div className="note">
              {r.quantization ?? "?"} — {v}% relative error
              {r.tokens_per_sec ? ` · ${r.tokens_per_sec} tok/s` : ""}
            </div>
            <svg width="100%" height="16" role="img" aria-label={`${r.quantization}: ${v}% error`}>
              <rect x="0" y="0" width="100%" height="16" rx="4" fill="var(--panel-2)" />
              <rect x="0" y="0" width={`${w}%`} height="16" rx="4" fill="var(--amber)" />
            </svg>
          </div>
        );
      })}
      <p className="note">Higher = more accuracy lost to quantization. Source: {withDelta[0].source_url}</p>
    </div>
  );
}
