import Link from "next/link";
import { loadHistory, latestScores } from "@/lib/history";

function badgeFor(score: number): string {
  if (score >= 60) return "bad bad";
  if (score >= 25) return "badge warn";
  return "badge good";
}

export default function Home() {
  const history = loadHistory();
  const rows = latestScores(history);

  return (
    <main className="wrap">
      <header>
        <h1>SlopGate</h1>
        <p>
          Per-repo AI-slop scores (0–100, higher = worse) from recorded check artifacts.
        </p>
      </header>
      <table className="table">
        <thead>
          <tr>
            <th>Repo</th>
            <th>Latest score</th>
            <th>Findings</th>
            <th>Trend</th>
            <th>Last run</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((repo) => (
            <tr key={repo.name}>
              <td>
                <Link href={`/repos/${encodeURIComponent(repo.name)}`}>{repo.name}</Link>
              </td>
              <td>
                <span className={badgeFor(repo.latest.score)}>{repo.latest.score}/100</span>
              </td>
              <td>{repo.latest.findings}</td>
              <td>
                {repo.trend === "up" ? "▲ worse" : repo.trend === "down" ? "▼ better" : "— flat"}
              </td>
              <td className="muted">{repo.latest.date}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </main>
  );
}
