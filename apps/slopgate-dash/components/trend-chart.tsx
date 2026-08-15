"use client";

// Zero-dependency SVG trend line: score (0-100, higher = worse) over runs.
import type { ScoreRun } from "@/lib/types";

const W = 720;
const H = 220;
const PAD = 36;

function points(runs: ScoreRun[]): string {
  const n = Math.max(runs.length - 1, 1);
  return runs
    .map((r, i) => {
      const x = PAD + (i * (W - PAD * 2)) / n;
      const y = PAD + ((100 - r.score) * (H - PAD * 2)) / 100;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
}

export default function TrendChart({ runs }: { runs: ScoreRun[] }) {
  const sorted = [...runs].sort((a, b) => a.date.localeCompare(b.date));
  if (sorted.length < 2) {
    return (
      <p className="muted">
        Need at least two recorded runs to draw a trend (have {sorted.length}).
      </p>
    );
  }
  const pts = points(sorted);
  const last = sorted[sorted.length - 1];
  const color = last.score >= 60 ? "#e5484d" : last.score >= 25 ? "#f5a524" : "#30a46c";

  return (
    <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label="Slop score trend line" className="chart">
      {/* threshold gridline at 50 */}
      <line x1={PAD} y1={yFor(50)} x2={W - PAD} y2={yFor(50)} stroke="#666" strokeDasharray="4 4" strokeWidth="1" />
      <text x={W - PAD + 4} y={yFor(50) + 4} fontSize="11" fill="#888">
        threshold 50
      </text>
      {[0, 25, 50, 75, 100].map((v) => (
        <text key={v} x={4} y={yFor(v) + 4} fontSize="11" fill="#888">
          {v}
        </text>
      ))}
      <polyline points={pts} fill="none" stroke={color} strokeWidth="2.5" strokeLinejoin="round" strokeLinecap="round" />
      {sorted.map((r, i) => {
        const x = PAD + (i * (W - PAD * 2)) / Math.max(sorted.length - 1, 1);
        const y = yFor(r.score);
        return (
          <g key={r.date}>
            <circle cx={x} cy={y} r="4" fill={color} />
            <text x={x} y={H - 8} fontSize="11" fill="#aaa" textAnchor="middle">
              {r.date.slice(5)}
            </text>
          </g>
        );
      })}
    </svg>
  );

  function yFor(score: number): number {
    return PAD + ((100 - score) * (H - PAD * 2)) / 100;
  }
}
