import { loadRows } from "../../../lib/benchmarks";
import { slugify, fmt } from "../../../lib/types";
import { notFound } from "next/navigation";
import QuantChart from "../../../components/quant-chart";

export function generateStaticParams() {
  return [...new Set(loadRows().map((r) => slugify(r.model)))].map((slug) => ({ slug }));
}

export default async function ModelPage({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const rows = loadRows().filter((r) => slugify(r.model) === slug);
  if (rows.length === 0) notFound();
  const model = rows[0].model;

  return (
    <div className="container">
      <a className="back" href="/">← back to matrix</a>
      <h1>{model}</h1>
      <p className="sub">{rows.length} configuration(s) across runtimes and hardware.</p>

      <div className="grid-2">
        <div className="panel">
          <h2>Configurations</h2>
          <table>
            <thead>
              <tr><th>Runtime</th><th>Hardware</th><th>Quant</th><th>tokens/s</th><th>peak RAM</th></tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={i}>
                  <td>{r.runtime}</td>
                  <td>{r.hardware.cpu}</td>
                  <td>{r.quantization ?? "—"}</td>
                  <td className="num">{fmt(r.tokens_per_sec)}</td>
                  <td className="num">{fmt(r.peak_ram_gb)} GB</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="panel">
          <h2>Quantization quality</h2>
          <QuantChart rows={rows} />
        </div>
      </div>

      <div className="panel">
        <h2>Sources</h2>
        <ul className="assumptions">
          {[...new Set(rows.map((r) => r.source_url))].map((u) => (
            <li key={u}><a href={u} target="_blank" rel="noreferrer">{u}</a></li>
          ))}
        </ul>
      </div>
    </div>
  );
}
