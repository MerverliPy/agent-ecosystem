import Link from "next/link";
import { notFound } from "next/navigation";
import { loadHistory, repoRuns, sortedRuns } from "@/lib/history";
import TrendChart from "@/components/trend-chart";
import type { Metadata } from "next";

interface Params {
  params: Promise<{ repo: string }>;
}

export function generateStaticParams() {
  return loadHistory().repos.map((r) => ({ repo: r.name }));
}

export async function generateMetadata({ params }: Params): Promise<Metadata> {
  const { repo } = await params;
  return { title: `${decodeURIComponent(repo)} — SlopGate` };
}

export default async function RepoPage({ params }: Params) {
  const { repo: encoded } = await params;
  const name = decodeURIComponent(encoded);
  const history = loadHistory();
  const repo = repoRuns(history, name);
  if (!repo) notFound();

  const runs = sortedRuns(repo);
  const latest = runs[runs.length - 1];

  return (
    <main className="wrap">
      <Link className="back" href="/">
        ← All repos
      </Link>
      <header>
        <h1>{repo.name}</h1>
        {repo.url ? (
          <p className="muted">
            <a href={repo.url}>{repo.url}</a>
          </p>
        ) : (
          <p className="muted">Score history from recorded check artifacts.</p>
        )}
      </header>
      <div className="stats">
        <div className="stat">
          <div className="label">Latest score</div>
          <div className="value">{latest.score}/100</div>
        </div>
        <div className="stat">
          <div className="label">Findings</div>
          <div className="value">{latest.findings}</div>
        </div>
        <div className="stat">
          <div className="label">High severity</div>
          <div className="value">{latest.high ?? 0}</div>
        </div>
        <div className="stat">
          <div className="label">Runs recorded</div>
          <div className="value">{runs.length}</div>
        </div>
      </div>
      <TrendChart runs={runs} />
      <ul className="runs">
        {[...runs].reverse().map((r) => (
          <li key={r.date}>
            <span>{r.date}</span>
            <span>
              score <strong>{r.score}/100</strong> · {r.findings} findings
              {r.sha ? <span className="muted"> · {r.sha.slice(0, 7)}</span> : null}
            </span>
          </li>
        ))}
      </ul>
    </main>
  );
}
