// Frontend pure-logic tests: sessions, memory filtering, approvals.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  createSession,
  addMessage,
  autoTitle,
  sortSessions,
  trimForContext,
  contextBudget,
} from "../src/lib/sessions.ts";
import {
  filterMemories,
  displayText,
  citationLine,
  confidenceLabel,
  groupByKind,
  pendingProposals,
} from "../src/lib/memory.ts";
import {
  memoryApprovalCard,
  actionApprovalCard,
  decide,
  pendingCards,
  confidenceDelta,
  applyConfidence,
} from "../src/lib/approvals.ts";
import type { MemoryEvent, Session } from "../src/lib/types.ts";

function mem(over: Partial<MemoryEvent>): MemoryEvent {
  return {
    id: "m1",
    kind: "semantic",
    content: "User prefers TypeScript for new services.",
    source: "extraction",
    confidence: 0.8,
    created_at: "2026-08-15T10:00:00Z",
    scope: { type: "companion" },
    approval: "approved",
    ...over,
  };
}

// ---------------------------------------------------------------- sessions

test("createSession builds an empty, titled session", () => {
  const s = createSession("bench-site");
  assert.match(s.id, /^sess-/);
  assert.equal(s.title, "New conversation");
  assert.equal(s.project_id, "bench-site");
  assert.equal(s.messages.length, 0);
});

test("addMessage appends and re-titles from the first user message", () => {
  let s = createSession();
  s = addMessage(s, "user", "How do I deploy the staging site?");
  assert.equal(s.messages.length, 1);
  assert.match(s.title, /^How do I deploy/);
  s = addMessage(s, "assistant", "Run the deploy script.");
  assert.equal(s.messages.length, 2);
  assert.equal(s.title, "How do I deploy the staging site?");
});

test("autoTitle truncates long user messages", () => {
  const long = "x".repeat(100);
  const t = autoTitle("New conversation", "user", long);
  assert.ok(t.length <= 60);
});

test("sortSessions orders by most recent update", () => {
  const a = { ...createSession(), updated_at: "2026-08-01T00:00:00Z" };
  const b = { ...createSession(), updated_at: "2026-08-10T00:00:00Z" };
  const [first, second] = sortSessions([a, b]);
  assert.equal(first.id, b.id);
  assert.equal(second.id, a.id);
});

test("trimForContext keeps the tail; contextBudget flags overshoot", () => {
  const s: Session = { ...createSession(), messages: Array.from({ length: 50 }, (_, i) => ({
    id: `m${i}`, session_id: s0Id(), role: "user" as const, content: "hi", created_at: "2026-08-15T00:00:00Z",
  })) };
  assert.equal(trimForContext(s, 10).length, 10);
  const big = { ...createSession(), messages: [{ id: "x", session_id: "s", role: "user" as const, content: "y".repeat(20000), created_at: "2026-08-15T00:00:00Z" }] };
  assert.equal(contextBudget(big.messages, 12000).over, true);
});

function s0Id() { return "s0"; }

// ---------------------------------------------------------------- memory

test("filterMemories applies kind, scope, approval and search", () => {
  const events = [
    mem({ id: "a", kind: "semantic", content: "TypeScript preference", tags: ["language"] }),
    mem({ id: "b", kind: "procedural", content: "Deploy via script", scope: { type: "project", project_id: "bench-site" } }),
    mem({ id: "c", kind: "working", content: "Refactoring retrieval", approval: "pending" }),
  ];
  assert.equal(filterMemories(events, { kind: "semantic", scope: "all", approval: "all", search: "" }).length, 1);
  assert.equal(filterMemories(events, { kind: "all", scope: "project", approval: "all", search: "" }).length, 1);
  assert.equal(filterMemories(events, { kind: "all", scope: "all", approval: "pending", search: "" }).length, 1);
  assert.equal(filterMemories(events, { kind: "all", scope: "all", approval: "all", search: "deploy" }).length, 1);
  assert.equal(filterMemories(events, { kind: "all", scope: "all", approval: "all", search: "type script" }).length, 1);
});

test("displayText truncates; citationLine is informative", () => {
  assert.equal(displayText(mem({ content: "abc" }), 2), "a…");
  const c = citationLine(mem({ source: "extraction" }));
  assert.match(c, /extracted · semantic/);
});

test("confidenceLabel buckets", () => {
  assert.equal(confidenceLabel(0.9), "high");
  assert.equal(confidenceLabel(0.6), "medium");
  assert.equal(confidenceLabel(0.2), "low");
});

test("groupByKind counts per kind; pendingProposals sorts oldest-first", () => {
  const events = [
    mem({ id: "a", kind: "semantic" }),
    mem({ id: "b", kind: "semantic" }),
    mem({ id: "c", kind: "working", approval: "pending", created_at: "2026-08-02T00:00:00Z" }),
    mem({ id: "d", kind: "working", approval: "pending", created_at: "2026-08-01T00:00:00Z" }),
  ];
  const g = groupByKind(events);
  assert.equal(g.semantic, 2);
  assert.equal(g.working, 2);
  assert.equal(g.episodic, 0);
  const p = pendingProposals(events);
  assert.equal(p[0].id, "d");
});

// ---------------------------------------------------------------- approvals

test("approval cards: memory writes and actions, decisions, learning signal", () => {
  const card = memoryApprovalCard(mem({ content: "remember me" }));
  assert.equal(card.kind, "memory_write");
  assert.equal(card.status, "pending");
  const action = actionApprovalCard("run: rm -rf dist");
  assert.equal(action.kind, "action");

  const approved = decide(card, "approved");
  const rejected = decide(card, "rejected");
  assert.equal(pendingCards([approved, rejected, card]).length, 1);

  assert.equal(confidenceDelta("approved"), 0.1);
  assert.equal(confidenceDelta("rejected"), -0.1);
  assert.equal(confidenceDelta("pending"), 0);
  assert.equal(applyConfidence(mem({ confidence: 0.8 }), -0.1).confidence, 0.7);
  assert.equal(applyConfidence(mem({ confidence: 0.05 }), -0.1).confidence, 0);
});
