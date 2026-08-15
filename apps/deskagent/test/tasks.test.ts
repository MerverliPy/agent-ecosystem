// Scheduled tasks + voice tests (Phase 6 Task 6 stubs).
import { test } from "node:test";
import assert from "node:assert/strict";
import { createTask, isDue, markDone, roll } from "../src/lib/tasks.ts";

test("task lifecycle: create, due, done, roll", () => {
  const past = new Date(Date.now() - 60_000).toISOString();
  const task = createTask("prune cache", "daily", past);
  assert.equal(task.status, "scheduled");
  assert.equal(isDue(task, new Date().toISOString()), true);

  const done = markDone(task);
  assert.equal(done.status, "done");
  assert.equal(isDue(done, new Date().toISOString()), false);

  const rolled = roll(task, new Date().toISOString());
  assert.equal(rolled.status, "scheduled");
  assert.ok(rolled.next_run > task.next_run);
});

test("weekly rolls +7 days, once stays scheduled until run", () => {
  const now = "2026-08-15T12:00:00Z";
  const weekly = roll(createTask("weekly digest", "weekly", now), now);
  assert.ok(weekly.next_run > now);
  const once = createTask("one-off", "once", now);
  assert.equal(once.status, "scheduled");
});
