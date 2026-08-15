"use client";

import { useMemo, useState } from "react";
import type { BenchmarkRow } from "../lib/types";
import { fmt, slugify } from "../lib/types";

const FITS_LABEL: Record<string, string> = {
  fits: "fits",
  "streams-needed": "streams",
  "no-fit": "no-fit",
};

function Badge({ fits }: { fits?: BenchmarkRow["fits"] }) {
  const cls = fits ? fits : "none";
  return <span className={`badge ${cls}`}>{fits ? FITS_LABEL[fits] : "unassessed"}</span>;
}

export default function Matrix({ rows }: { rows: BenchmarkRow[] }) {
  const [q, setQ] = useState("");
  const [runtime, setRuntime] = useState("all");
  const [fits, setFits] = useState("all");
  const [sort, setSort] = useState<"tps" | "ram">("tps");

  const runtimes = useMemo(() => [...new Set(rows.map((r) => r.runtime))].sort(), [rows]);

  const filtered = useMemo(() => {
    let out = rows.filter((r) => {
      if (runtime !== "all" && r.runtime !== runtime) return false;
      if (fits !== "all" && (r.fits ?? "none") !== fits) return false;
      if (q) {
        const hay = `${r.model} ${r.runtime} ${r.hardware.cpu} ${r.hardware.gpu ?? ""} ${r.quantization ?? ""}`.toLowerCase();
        if (!hay.includes(q.toLowerCase())) return false;
      }
      return true;
    });
    out = [...out].sort((a, b) => {
      if (sort === "ram") return (b.peak_ram_gb ?? -1) - (a.peak_ram_gb ?? -1);
      return (b.tokens_per_sec ?? -1) - (a.tokens_per_sec ?? -1);
    });
    return out;
  }, [rows, q, runtime, fits, sort]);

  return (
    <div className="panel">
      <div className="filters">
        <input type="text" placeholder="Search model, runtime, hardware…" value={q} onChange={(e) => setQ(e.target.value)} />
        <select value={runtime} onChange={(e) => setRuntime(e.target.value)}>
          <option value="all">runtime: all</option>
          {runtimes.map((r) => (
            <option key={r} value={r}>{r}</option>
          ))}
        </select>
        <select value={fits} onChange={(e) => setFits(e.target.value)}>
          <option value="all">fit: all</option>
          <option value="fits">fits</option>
          <option value="streams-needed">streams</option>
          <option value="no-fit">no-fit</option>
        </select>
        <select value={sort} onChange={(e) => setSort(e.target.value as "tps" | "ram")}>
          <option value="tps">sort: tokens/sec</option>
          <option value="ram">sort: peak RAM</option>
        </select>
        <span className="note">{filtered.length} of {rows.length} rows</span>
      </div>

      <table>
        <thead>
          <tr>
            <th>Model</th>
            <th>Runtime</th>
            <th>Hardware</th>
            <th>Quant</th>
            <th>tokens/s</th>
            <th>peak RAM</th>
            <th>disk</th>
            <th>fit</th>
            <th>source</th>
          </tr>
        </thead>
        <tbody>
          {filtered.map((r, i) => (
            <tr key={i}>
              <td>
                <a href={`/models/${slugify(r.model)}`}>{r.model}</a>
              </td>
              <td>{r.runtime}</td>
              <td>{r.hardware.cpu}{r.hardware.gpu ? ` / ${r.hardware.gpu}` : ""}</td>
              <td>{r.quantization ?? "—"}</td>
              <td className="num">{fmt(r.tokens_per_sec)}</td>
              <td className="num">{fmt(r.peak_ram_gb)} GB</td>
              <td className="num">{fmt(r.disk_size_gb, 0)} GB</td>
              <td><Badge fits={r.fits} /></td>
              <td><a href={r.source_url} target="_blank" rel="noreferrer">src↗</a></td>
            </tr>
          ))}
          {filtered.length === 0 && (
            <tr><td colSpan={9} className="note">No rows match the filters.</td></tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
