// Dashboard data-contract tests: history.json shape, ordering, aggregation.
import { test } from "node:test";
import assert from "node:assert/strict";
import { loadHistory, repoRuns, latestScores, sortedRuns } from "../lib/history.ts";

// The artifact lives at data/history.json; the test imports the same lib used by
// the Next pages (lib/history.ts is loaded via the compiled .js here).
test("history artifact loads and matches the schema", () => {
  const history = loadHistory();
  assert.ok(Array.isArray(history.repos));
  assert.ok(history.repos.length >= 3, "expected >= 3 repos in the artifact");
  for (const repo of history.repos) {
    assert.equal(typeof repo.name, "string");
    assert.ok(repo.runs.length >= 2, `${repo.name} needs >= 2 runs for a trend`);
    for (const run of repo.runs) {
      assert.match(run.date, /^\d{4}-\d{2}-\d{2}$/);
      assert.ok(run.score >= 0 && run.score <= 100, `${repo.name} run ${run.date} score out of range`);
      assert.equal(typeof run.findings, "number");
    }
  }
});

test("runs are recorded in chronological order", () => {
  const history = loadHistory();
  for (const repo of history.repos) {
    const sorted = sortedRuns(repo);
    for (let i = 1; i < sorted.length; i++) {
      assert.ok(sorted[i].date >= sorted[i - 1].date, `${repo.name} out of order`);
    }
  }
});

test("latestScores picks the newest run and computes the trend", () => {
  const history = loadHistory();
  const rows = latestScores(history);
  for (const row of rows) {
    assert.equal(row.latest, row.runs[row.runs.length - 1]);
    assert.ok(["up", "down", "flat"].includes(row.trend));
  }
  const heavy = rows.find((r) => r.name === "fixture-heavy");
  const clean = rows.find((r) => r.name === "fixture-clean");
  assert.ok(heavy && clean);
  assert.ok(heavy.latest.score > clean.latest.score, "heavy should outscore clean");
  assert.equal(heavy.trend, "up");
  assert.equal(clean.trend, "down");
});

test("repoRuns finds a repo by name and returns undefined for unknown", () => {
  const history = loadHistory();
  assert.ok(repoRuns(history, "fixture-mild"));
  assert.equal(repoRuns(history, "nope"), undefined);
});

test("artifact scores match the seeded fixtures (real scan values)", () => {
  const history = loadHistory();
  const byName = Object.fromEntries(history.repos.map((r) => [r.name, r]));
  // The 2026-08-15 runs were recorded by scripts/record-run.mjs from real scans.
  assert.equal(byName["fixture-clean"].runs.at(-1).score, 0);
  assert.equal(byName["fixture-mild"].runs.at(-1).score, 29);
  assert.equal(byName["fixture-heavy"].runs.at(-1).score, 100);
});
