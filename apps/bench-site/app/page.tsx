import { loadRows } from "../lib/benchmarks";
import Matrix from "../components/matrix";
import CalculatorWidget from "../components/calculator-widget";

export default function Home() {
  const rows = loadRows();
  return (
    <div className="container">
      <h1>BenchKit</h1>
      <p className="sub">
        Can my machine run it? Local-inference benchmark matrix — every row links to an attributable
        source (DEC-0006). Measured data is preferred over the calculator&apos;s estimates.
      </p>
      <Matrix rows={rows} />
      <div className="grid-2">
        <CalculatorWidget />
        <div className="panel">
          <h2>About the data</h2>
          <ul className="assumptions">
            <li>Rows are attributed to repos/papers via <code>source_url</code>; nothing is fabricated (nulls where unpublished).</li>
            <li>Speed estimates are optimistic upper bounds; the runner CLI can add real measured rows.</li>
            <li>Kimi K3: MXFP4 experts (QAT), int8 ≈ 1% / int4 ≈ 17% reconstruction error, KDA KV cache constant at any context.</li>
            <li>Add a row: <code>node apps/bench-site/scripts/bench-run.ts --help</code></li>
          </ul>
        </div>
      </div>
    </div>
  );
}
