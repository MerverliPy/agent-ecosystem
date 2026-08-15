// Load and aggregate the recorded history artifact (data/history.json).
import { readFileSync } from "node:fs";
import path from "node:path";
import type { HistoryFile, RepoHistory, ScoreRun } from "./types";

const DATA_FILE = path.join(process.cwd(), "data", "history.json");

export function loadHistory(): HistoryFile {
  try {
    const raw = readFileSync(DATA_FILE, "utf8");
    return JSON.parse(raw) as HistoryFile;
  } catch (err) {
    throw new Error(`Failed to read ${DATA_FILE}: ${(err as Error).message}`);
  }
}

export function repoRuns(history: HistoryFile, name: string): RepoHistory | undefined {
  return history.repos.find((r) => r.name === name);
}

/** Latest score per repo, for the overview table. */
export function latestScores(history: HistoryFile): Array<RepoHistory & { latest: ScoreRun; trend: "up" | "down" | "flat" }> {
  return history.repos.map((repo) => {
    const sorted = [...repo.runs].sort((a, b) => a.date.localeCompare(b.date));
    const latest = sorted[sorted.length - 1];
    const prev = sorted[sorted.length - 2];
    let trend: "up" | "down" | "flat" = "flat";
    if (prev && latest.score > prev.score) trend = "up";
    else if (prev && latest.score < prev.score) trend = "down";
    return { ...repo, latest, trend };
  });
}

export function sortedRuns(repo: RepoHistory): ScoreRun[] {
  return [...repo.runs].sort((a, b) => a.date.localeCompare(b.date));
}
