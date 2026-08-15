#!/usr/bin/env node
// Record a SlopGate check artifact: run `slop score --json` against a path and append
// the run to data/history.json under a repo name. This is the "reads check artifacts"
// pipeline: CI (or a cron) calls this after each scan and the dashboard renders the trend.
//
// Usage:
//   node scripts/record-run.mjs --repo <name> [--path <scan-path>] [--url <repo-url>] [--sha <sha>]
//   --path defaults to ../slopgate/fixtures/<name> so the seeded fixtures can be recorded directly.
//
// Reads SLOPGATE_DATA_FILE (default data/history.json) to override the artifact path.

import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const CLI = path.join(ROOT, "..", "slopgate", "src", "cli.ts");
const DATA_FILE = process.env.SLOPGATE_DATA_FILE ?? path.join(ROOT, "data", "history.json");

function arg(args, flag) {
  const i = args.indexOf(flag);
  return i === -1 ? undefined : args[i + 1];
}

function main(args) {
  const repo = arg(args, "--repo");
  if (!repo) {
    console.error("record-run: --repo <name> is required");
    process.exit(2);
  }
  const fixtureDir = repo.replace(/^fixture-/, "");
  const scanPath = arg(args, "--path") ?? path.join(ROOT, "..", "slopgate", "fixtures", fixtureDir);
  const url = arg(args, "--url");
  const sha = arg(args, "--sha");
  const date = new Date().toISOString().slice(0, 10);

  const res = spawnSync(process.execPath, ["--experimental-strip-types", CLI, "score", scanPath, "--json"], {
    encoding: "utf8",
  });
  if (res.status !== 0) {
    console.error(`slop score failed (exit ${res.status}): ${res.stderr}`);
    process.exit(1);
  }
  const score = JSON.parse(res.stdout);

  let history = { repos: [] };
  if (existsSync(DATA_FILE)) {
    history = JSON.parse(readFileSync(DATA_FILE, "utf8"));
  }

  let entry = history.repos.find((r) => r.name === repo);
  if (!entry) {
    entry = { name: repo, runs: [] };
    if (url) entry.url = url;
    history.repos.push(entry);
  }

  // Drop an identical run recorded the same day to keep the artifact deduped.
  entry.runs = entry.runs.filter((r) => !(r.date === date && r.score === score.score && r.findings === score.totalFindings));
  entry.runs.push({
    date,
    score: score.score,
    findings: score.totalFindings,
    high: score.high,
    medium: score.medium,
    low: score.low,
    ...(sha ? { sha } : {}),
  });
  entry.runs.sort((a, b) => a.date.localeCompare(b.date));
  history.generatedAt = new Date().toISOString();

  mkdirSync(path.dirname(DATA_FILE), { recursive: true });
  writeFileSync(DATA_FILE, JSON.stringify(history, null, 2) + "\n");
  console.log(`recorded ${repo} @ ${date}: score ${score.score}, findings ${score.totalFindings} → ${DATA_FILE}`);
}

main(process.argv.slice(2));
