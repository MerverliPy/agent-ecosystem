import { useMemo, useState } from "react";
import { BENCHKIT_CATALOG, defaultMachine, evaluateRows, pickForMachine, verdictLabel } from "../lib/picker.ts";

// BenchKit-driven model picker (Phase 6 Task 2): shows "runs on your machine" per
// model from the bundled catalog + will-it-run; offline by design (DEC-0005).
export default function ModelPicker() {
  const [ramGb, setRamGb] = useState(16);
  const machine = useMemo(() => ({ ...defaultMachine(), ramGb }), [ramGb]);
  const entries = useMemo(() => evaluateRows(BENCHKIT_CATALOG, machine), [machine]);
  const picks = useMemo(() => pickForMachine(BENCHKIT_CATALOG, machine), [machine]);

  return (
    <section className="picker">
      <header>
        <h3>Model picker</h3>
        <span className="muted small">fit from BenchKit data (will-it-run · {BENCHKIT_CATALOG.length} models)</span>
      </header>
      <label className="small">
        Machine RAM: <input type="range" min={4} max={256} step={4} value={ramGb} onChange={(e) => setRamGb(Number(e.target.value))} />
        <strong>{ramGb} GB</strong>
      </label>
      {picks.length > 0 && (
        <p className="small muted">Best for this machine: <strong>{picks[0].model}</strong> (~{picks[0].estTokensPerSec?.toFixed(1)} tok/s)</p>
      )}
      <table className="table small">
        <thead>
          <tr>
            <th>Model</th>
            <th>Runtime</th>
            <th>RAM est.</th>
            <th>tok/s</th>
            <th>Verdict</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((e) => (
            <tr key={`${e.model}-${e.runtime}`}>
              <td title={e.sourceUrl}>{e.model}</td>
              <td>{e.runtime}</td>
              <td>{e.ramNeededGb.toFixed(0)} GB</td>
              <td>{e.estTokensPerSec != null ? e.estTokensPerSec.toFixed(1) : "—"}</td>
              <td>
                <span className={e.verdict === "fits" ? "badge good" : e.verdict === "streams-needed" ? "badge warn" : "badge bad"}>
                  {verdictLabel(e.verdict)}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="muted small">Data: {BENCHKIT_CATALOG.length} rows from shared/datasets/benchmarks.jsonl (DEC-0006 — every row links a source).</p>
    </section>
  );
}
