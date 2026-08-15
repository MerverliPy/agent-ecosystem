// Memory-event schema validation tests: four kinds + invalid cases.
import { test } from "node:test";
import assert from "node:assert/strict";
import { validateMemoryEvent } from "../validate-memory-event.mjs";

const base = {
  id: "mem-0001",
  kind: "episodic",
  content: "User said: the deploy pipeline is broken again.",
  source: "conversation",
  confidence: 0.9,
  created_at: "2026-08-15T10:00:00Z",
  scope: { type: "companion" },
};

test("valid episodic memory event passes", () => {
  assert.deepEqual(validateMemoryEvent(base), []);
});

test("valid semantic memory event passes", () => {
  const ev = {
    ...base,
    id: "sem-001",
    kind: "semantic",
    content: "User prefers TypeScript over JavaScript for new services.",
    summary: "TypeScript preference",
    source: "extraction",
    confidence: 0.8,
    episode_id: "mem-0001",
    approval: "approved",
    tags: ["preference", "language"],
  };
  assert.deepEqual(validateMemoryEvent(ev), []);
});

test("valid procedural memory event passes", () => {
  const ev = {
    ...base,
    id: "proc-001",
    kind: "procedural",
    content: "To deploy: run `bash scripts/deploy.sh staging` then verify health endpoint.",
    source: "conversation",
    confidence: 0.7,
    scope: { type: "project", project_id: "bench-site", project_path: "apps/bench-site" },
  };
  assert.deepEqual(validateMemoryEvent(ev), []);
});

test("valid working memory event with embedding + decay passes", () => {
  const ev = {
    ...base,
    id: "work-001",
    kind: "working",
    content: "Currently refactoring the retrieval module; injection budget topic open.",
    source: "synthesis",
    confidence: 0.6,
    embedding: { model: "test-hash-v1", dimensions: 4, vector: [0.1, 0.2, 0.3, 0.4] },
    decay: { half_life_days: 14, last_refreshed_at: "2026-08-15T10:00:00Z" },
  };
  assert.deepEqual(validateMemoryEvent(ev), []);
});

test("missing required fields fail", () => {
  for (const key of ["id", "kind", "content", "source", "confidence", "created_at", "scope"]) {
    const { [key]: _drop, ...rest } = base;
    const errors = validateMemoryEvent(rest);
    assert.ok(errors.some((e) => e.includes(`missing required field "${key}"`)), `expected missing ${key}`);
  }
});

test("invalid kind fails", () => {
  const errors = validateMemoryEvent({ ...base, kind: "dreams" });
  assert.ok(errors.some((e) => e.includes("kind")));
});

test("confidence out of range fails", () => {
  assert.ok(validateMemoryEvent({ ...base, confidence: 1.5 }).some((e) => e.includes("confidence")));
  assert.ok(validateMemoryEvent({ ...base, confidence: -0.1 }).some((e) => e.includes("confidence")));
});

test("malformed created_at fails", () => {
  assert.ok(validateMemoryEvent({ ...base, created_at: "yesterday-ish" }).some((e) => e.includes("created_at")));
});

test("project scope without project_id fails; companion scope passes without one", () => {
  const project = { ...base, scope: { type: "project" } };
  assert.ok(validateMemoryEvent(project).some((e) => e.includes("project_id")));
  const companion = { ...base, scope: { type: "companion" } };
  assert.deepEqual(validateMemoryEvent(companion), []);
});

test("bad approval value fails", () => {
  assert.ok(validateMemoryEvent({ ...base, approval: "maybe" }).some((e) => e.includes("approval")));
});

test("embedding shape mismatch fails", () => {
  const bad = { ...base, embedding: { model: "m", dimensions: 3, vector: [1, 2] } };
  assert.ok(validateMemoryEvent(bad).some((e) => e.includes("embedding")));
});

test("non-object input fails", () => {
  assert.ok(validateMemoryEvent(null).length > 0);
  assert.ok(validateMemoryEvent([1, 2]).length > 0);
  assert.ok(validateMemoryEvent("string").length > 0);
});
