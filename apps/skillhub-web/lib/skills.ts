// Server-only snapshot loader (uses node:fs). Import only from server components/pages.
import { readFileSync } from "node:fs";
import path from "node:path";
import type { Snapshot } from "./types";

const DATA_PATH = path.join(process.cwd(), "data", "skills.json");

export function loadSnapshot(): Snapshot {
  const raw = readFileSync(DATA_PATH, "utf8");
  return JSON.parse(raw) as Snapshot;
}
