#!/usr/bin/env node
// Validate memory-event objects against the memory-event schema semantics.
// Zero-dependency, hand-rolled checks (repo convention — mirrors the benchmark
// dataset validator). Exits 1 listing every error.
// Usage: node shared/schemas/validate-memory-event.mjs <file.json|file.jsonl|->  (- = stdin)
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const KINDS = new Set(["episodic", "semantic", "procedural", "working"]);
const SOURCES = new Set(["conversation", "user", "file", "api", "reflection", "extraction", "synthesis", "other"]);
const APPROVALS = new Set(["pending", "approved", "rejected"]);
const ERRORS = [];

function err(ctx, msg) {
  ERRORS.push(`${ctx}: ${msg}`);
}

function isNum(x) {
  return typeof x === "number" && Number.isFinite(x);
}

function isDate(s) {
  return typeof s === "string" && !Number.isNaN(Date.parse(s));
}

/** Validate a single parsed event object. Returns the list of errors. */
export function validateMemoryEvent(event, ctx = "event") {
  const errors = [];
  const e = (msg) => errors.push(`${ctx}: ${msg}`);

  if (event === null || typeof event !== "object" || Array.isArray(event)) {
    e("must be an object");
    return errors;
  }

  for (const k of ["id", "kind", "content", "source", "confidence", "created_at", "scope"]) {
    if (!(k in event)) e(`missing required field "${k}"`);
  }

  if ("id" in event && (typeof event.id !== "string" || event.id.length === 0 || !/^[A-Za-z0-9_-]+$/.test(event.id))) {
    e("id must be a non-empty string of [A-Za-z0-9_-]");
  }
  if ("kind" in event && !KINDS.has(event.kind)) e(`kind must be one of ${[...KINDS].join("|")}`);
  if ("content" in event && (typeof event.content !== "string" || event.content.length === 0)) e("content must be a non-empty string");
  if ("summary" in event && typeof event.summary !== "string") e("summary must be a string");
  if ("source" in event && !SOURCES.has(event.source)) e(`source must be one of ${[...SOURCES].join("|")}`);
  if ("confidence" in event && (!isNum(event.confidence) || event.confidence < 0 || event.confidence > 1)) {
    e("confidence must be a number in [0, 1]");
  }
  for (const k of ["created_at", "updated_at"]) {
    if (k in event && !isDate(event[k])) e(`${k} must be an ISO date-time string`);
  }
  if ("episode_id" in event && typeof event.episode_id !== "string") e("episode_id must be a string");
  if ("approval" in event && !APPROVALS.has(event.approval)) e(`approval must be one of ${[...APPROVALS].join("|")}`);

  // scope
  if ("scope" in event) {
    const s = event.scope;
    if (s === null || typeof s !== "object" || Array.isArray(s)) {
      e("scope must be an object");
    } else {
      if (!["companion", "project"].includes(s.type)) e("scope.type must be companion|project");
      if (s.type === "project" && (typeof s.project_id !== "string" || s.project_id.length === 0)) {
        e("scope.type=project requires a non-empty project_id");
      }
      if ("project_path" in s && typeof s.project_path !== "string") e("scope.project_path must be a string");
    }
  }

  // decay
  if ("decay" in event) {
    const d = event.decay;
    if (d === null || typeof d !== "object") {
      e("decay must be an object");
    } else {
      if ("half_life_days" in d && (!isNum(d.half_life_days) || d.half_life_days < 0)) e("decay.half_life_days must be a number >= 0");
      if ("last_refreshed_at" in d && !isDate(d.last_refreshed_at)) e("decay.last_refreshed_at must be an ISO date-time");
    }
  }

  // embedding
  if ("embedding" in event) {
    const em = event.embedding;
    if (em === null || typeof em !== "object" || Array.isArray(em)) {
      e("embedding must be an object");
    } else {
      if (typeof em.model !== "string" || em.model.length === 0) e("embedding.model must be a non-empty string");
      if (!Number.isInteger(em.dimensions) || em.dimensions < 1) e("embedding.dimensions must be an integer >= 1");
      if (!Array.isArray(em.vector) || em.vector.length !== em.dimensions || em.vector.some((v) => !isNum(v))) {
        e("embedding.vector must be an array of numbers with length == dimensions");
      }
    }
  }

  if ("tags" in event && (!Array.isArray(event.tags) || event.tags.some((t) => typeof t !== "string"))) {
    e("tags must be an array of strings");
  }

  return errors;
}

// CLI mode (default export used by tests):
const isCli = process.argv[1]?.endsWith("validate-memory-event.mjs");
if (isCli) {
  const input = process.argv[2] ?? "-";
  let raw;
  try {
    raw = input === "-" ? readFileSync(0, "utf8") : readFileSync(path.resolve(input), "utf8");
  } catch (err) {
    console.error(`Cannot read input ${input}: ${err.message}`);
    process.exit(2);
  }
  const isJsonl = /\.jsonl$/.test(input);
  let events = [];
  if (isJsonl) {
    events = raw.split("\n").filter((l) => l.trim()).map((l) => JSON.parse(l));
  } else {
    events = [JSON.parse(raw)];
  }
  events.forEach((ev, i) => {
    validateMemoryEvent(ev, `event ${i}`).forEach((msg) => ERRORS.push(msg));
  });
  if (ERRORS.length > 0) {
    console.error(`MEMORY-SCHEMA-FAIL: ${ERRORS.length} error(s):`);
    for (const e of ERRORS) console.error("  " + e);
    process.exit(1);
  }
  console.log(`MEMORY-SCHEMA-OK: ${events.length} event(s) validated`);
}
