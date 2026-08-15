// Snapshot contract test: data/skills.json must exist, parse, and carry required fields.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const dataPath = path.join(here, "..", "data", "skills.json");

test("snapshot file exists", () => {
  assert.ok(existsSync(dataPath), "data/skills.json missing — run the e2e script to generate it");
});

test("snapshot shape and required fields", () => {
  const snap = JSON.parse(readFileSync(dataPath, "utf8"));
  assert.ok(typeof snap.updated_at === "string");
  assert.ok(Array.isArray(snap.packages));
  for (const p of snap.packages) {
    assert.ok(p.name && p.name.includes("/"), `name malformed: ${p.name}`);
    assert.ok(typeof p.description === "string");
    assert.ok(typeof p.verified === "boolean");
    assert.ok(typeof p.downloads === "number");
    assert.ok(Array.isArray(p.versions));
  }
});

test("benign fixture is present and verified", () => {
  const snap = JSON.parse(readFileSync(dataPath, "utf8"));
  const hello = snap.packages.find((p) => p.name === "demo/hello-skill");
  assert.ok(hello, "demo/hello-skill missing from snapshot");
  assert.equal(hello.verified, true);
});
