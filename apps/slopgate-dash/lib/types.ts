// SlopGate dashboard data contract — mirrors the recorded check artifact
// (data/history.json). Produced by scripts/record-run.mjs from `slop score --json`.

export interface ScoreRun {
  date: string; // YYYY-MM-DD
  score: number; // 0-100, higher = more slop
  findings: number;
  high?: number;
  medium?: number;
  low?: number;
  sha?: string;
}

export interface RepoHistory {
  name: string;
  url?: string;
  runs: ScoreRun[];
}

export interface HistoryFile {
  generatedAt?: string;
  repos: RepoHistory[];
}
