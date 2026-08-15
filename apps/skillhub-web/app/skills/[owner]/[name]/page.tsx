import { loadSnapshot } from "../../../../lib/skills";
import { slugify, fmt } from "../../../../lib/types";
import { notFound } from "next/navigation";
import { Badges } from "../../../../components/browse";

export function generateStaticParams() {
  return loadSnapshot().packages.map((p) => ({
    owner: p.name.split("/")[0],
    name: p.name.split("/")[1],
  }));
}

export default async function SkillPage({ params }: { params: Promise<{ owner: string; name: string }> }) {
  const { owner, name } = await params;
  const snap = loadSnapshot();
  const pkg = snap.packages.find((p) => slugify(p.name) === slugify(`${owner}/${name}`));
  if (!pkg) notFound();

  const latest = pkg.versions[0];

  return (
    <div className="container">
      <a className="back" href="/">← back to registry</a>
      <h1>{pkg.name}</h1>
      <p className="sub">{pkg.description}</p>
      <p><Badges pkg={pkg} /></p>

      <div className="panel">
        <h2>Install</h2>
        {(latest?.harnesses ?? ["pi"]).map((h) => (
          <code key={h} className="install">skillhub install {pkg.name} --harness {h}</code>
        ))}
        <p className="note">Run inside the target machine; the CLI detects the harness and writes its skills directory.</p>
      </div>

      <div className="panel">
        <h2>Package info</h2>
        <p>License: <code>{pkg.license}</code></p>
        <p>Repository: <a href={pkg.repo} target="_blank" rel="noreferrer">{pkg.repo}</a></p>
        <p>Installs: {fmt(pkg.downloads)}</p>
        <p>Latest version: <code>{latest?.version ?? "—"}</code> (published {latest?.published_at ?? "—"})</p>
      </div>

      <div className="panel">
        <h2>Versions</h2>
        <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
          <thead><tr style={{ textAlign: "left", color: "var(--muted)" }}><th>Version</th><th>Verified</th><th>Harnesses</th><th>Permissions</th></tr></thead>
          <tbody>
            {pkg.versions.map((v) => (
              <tr key={v.version} style={{ borderTop: "1px solid var(--border)" }}>
                <td><code>{v.version}</code></td>
                <td>{v.verified ? "✅" : "⚠️ unverified"}</td>
                <td>{v.harnesses.join(", ")}</td>
                <td>{v.permissions.join(", ") || "none declared"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
