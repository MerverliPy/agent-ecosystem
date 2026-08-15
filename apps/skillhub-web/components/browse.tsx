"use client";

import { useMemo, useState } from "react";
import type { SkillPackage } from "../lib/types";
import { slugify } from "../lib/types";

export function Badges({ pkg }: { pkg: SkillPackage }) {
  return (
    <span>
      {pkg.verified ? <span className="badge verified">verified</span> : <span className="badge muted">unverified</span>}
      {pkg.high_risk ? <span className="badge high-risk">high-risk</span> : null}
    </span>
  );
}

export default function Browse({ packages }: { packages: SkillPackage[] }) {
  const [q, setQ] = useState("");
  const filtered = useMemo(() => {
    if (!q) return packages;
    const needle = q.toLowerCase();
    return packages.filter((p) => `${p.name} ${p.description} ${p.license}`.toLowerCase().includes(needle));
  }, [packages, q]);

  return (
    <>
      <input
        className="search"
        type="text"
        placeholder="Search skills (name, description)…"
        value={q}
        onChange={(e) => setQ(e.target.value)}
      />
      <p className="note">{filtered.length} of {packages.length} packages</p>
      <div className="grid">
        {filtered.map((p) => (
          <a key={p.name} href={`/skills/${slugify(p.name)}`} className="card" style={{ color: "inherit" }}>
            <h3>{p.name}</h3>
            <p className="desc">{p.description}</p>
            <Badges pkg={p} />
            <p className="meta">{p.downloads} installs · {p.license}</p>
          </a>
        ))}
        {filtered.length === 0 && <p className="note">No packages match.</p>}
      </div>
    </>
  );
}
