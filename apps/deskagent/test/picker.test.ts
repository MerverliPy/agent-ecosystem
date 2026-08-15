// Model picker logic tests: BenchKit-driven fit verdicts + offline catalog fallback.
import { test } from "node:test";
import assert from "node:assert/strict";
import { BENCHKIT_CATALOG } from "../src/lib/benchkit-catalog.ts";
import {
  defaultMachine,
  evaluateRows,
  loadLiveCatalog,
  pickForMachine,
  resetLiveCache,
  verdictLabel,
} from "../src/lib/picker.ts";

test("bundled catalog mirrors the BenchKit dataset (7 rows, DEC-0006 sources)", () => {
  assert.equal(BENCHKIT_CATALOG.length, 7);
  for (const row of BENCHKIT_CATALOG) {
    assert.ok(row.source_url.startsWith("http"), `missing source_url for ${row.model}`);
    assert.ok(row.model.length > 0);
  }
});

test("evaluateRows produces fits verdicts for a big machine", () => {
  const machine = { ...defaultMachine(), ramGb: 256 };
  const entries = evaluateRows(BENCHKIT_CATALOG, machine);
  assert.equal(entries.length, BENCHKIT_CATALOG.length);
  assert.ok(entries.every((e) => e.verdict === "fits"), "256GB machine fits everything in the catalog");
  for (const e of entries) {
    assert.ok(e.ramNeededGb > 0);
    assert.ok(e.sourceUrl.startsWith("http"));
  }
});

test("pickForMachine orders by speed and drops no-fit", () => {
  const machine = defaultMachine(); // 16 GB
  const picks = pickForMachine(BENCHKIT_CATALOG, machine);
  assert.ok(picks.every((p) => p.verdict !== "no-fit"));
  for (let i = 1; i < picks.length; i++) {
    assert.ok((picks[i - 1].estTokensPerSec ?? 0) >= (picks[i].estTokensPerSec ?? 0));
  }
  assert.ok(picks.length > 0, "at least one model runs on a 16GB machine");
});

test("measured tokens_per_sec preferred over estimate when present", () => {
  const machine = defaultMachine();
  const entries = evaluateRows(BENCHKIT_CATALOG, machine);
  const measured = entries.find((e) => e.measuredTokensPerSec != null);
  assert.ok(measured, "catalog has measured rows");
  assert.equal(measured?.estTokensPerSec, measured?.measuredTokensPerSec);
});

test("verdictLabel is human readable", () => {
  assert.equal(verdictLabel("fits"), "runs on your machine");
  assert.equal(verdictLabel("streams-needed"), "streams from NVMe/disk");
  assert.equal(verdictLabel("no-fit"), "no-fit");
});

test("defaultMachine is a sane 16GB profile", () => {
  const m = defaultMachine();
  assert.equal(m.ramGb, 16);
  assert.ok(m.memBandwidthGbPerSec! > 0);
  assert.equal(m.streamingSupported, true);
});

// ---- live BenchKit fetch with cached fallback (Phase 7 Task 2) -----------------

const SAMPLE_JSONL = [
  JSON.stringify({
    model: "live-model",
    runtime: "ollama",
    source_url: "https://example.com/x",
    hardware: { cpu: "M3", ram_gb: 64, os: "macos" },
    tokens_per_sec: 12.5,
    peak_ram_gb: 8.1,
  }),
  "{not json",
  JSON.stringify({ model: "missing-fields" }),
].join("\n");

const okFetch = (async () => ({
  ok: true,
  status: 200,
  async text() {
    return SAMPLE_JSONL;
  },
})) as unknown as typeof fetch;

const failFetch = (async () =>
  ({ ok: false, status: 503, async text() { return ""; } })) as unknown as typeof fetch;

test("loadLiveCatalog uses live rows when the fetch succeeds", async () => {
  const { rows, source, error } = await loadLiveCatalog(okFetch);
  if (source !== "live") console.error("DBG live fetch failed:", error);
  assert.equal(source, "live");
  assert.ok(rows.some((r) => r.model === "live-model"));
  assert.equal(rows.some((r) => r.model === "missing-fields"), false, "bad rows dropped");
});

test("loadLiveCatalog falls back to the bundled catalog on failure", async () => {
  resetLiveCache();
  const { rows, source, error } = await loadLiveCatalog(failFetch);
  assert.equal(source, "bundled");
  assert.ok(error, "error surfaced");
  assert.equal(rows, BENCHKIT_CATALOG);
});
