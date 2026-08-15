// Memory explorer logic: filtering, sorting and display formatting. Pure functions.

import type { MemoryEvent, MemoryKind, MemoryScope, ScopeType } from "./types.ts";

export interface MemoryFilter {
  kind: MemoryKind | "all";
  scope: ScopeType | "all";
  approval: "approved" | "pending" | "rejected" | "all";
  search: string;
}

export const DEFAULT_FILTER: MemoryFilter = {
  kind: "all",
  scope: "all",
  approval: "all",
  search: "",
};

export const KIND_LABELS: Record<MemoryKind, string> = {
  episodic: "Episodes",
  semantic: "Facts & preferences",
  procedural: "How-to",
  working: "Working context",
};

/** Filter + sort memories for the explorer. */
export function filterMemories(events: MemoryEvent[], filter: MemoryFilter): MemoryEvent[] {
  const q = filter.search.trim().toLowerCase();
  return events
    .filter((e) => filter.kind === "all" || e.kind === filter.kind)
    .filter((e) => filter.scope === "all" || e.scope.type === filter.scope)
    .filter((e) => filter.approval === "all" || e.approval === filter.approval)
    .filter((e) => {
      if (!q) return true;
      const hay = `${e.content} ${e.summary ?? ""} ${(e.tags ?? []).join(" ")}`.toLowerCase();
      return q.split(/\s+/).every((word) => hay.includes(word));
    })
    .sort((a, b) => b.created_at.localeCompare(a.created_at));
}

/** Short display text for a memory row. */
export function displayText(e: MemoryEvent, maxLen = 140): string {
  const text = e.summary?.trim() || e.content.trim();
  return text.length > maxLen ? `${text.slice(0, maxLen - 1)}…` : text;
}

/** Citation snippet for chat: "I remember… (source, kind)". */
export function citationLine(e: MemoryEvent): string {
  const origin = e.source === "extraction" ? "extracted" : e.source;
  return `“${displayText(e, 90)}” — ${origin} · ${e.kind}${e.scope.type === "project" ? ` · ${e.scope.project_id}` : ""}`;
}

/** A human-readable confidence label. */
export function confidenceLabel(c: number): string {
  if (c >= 0.8) return "high";
  if (c >= 0.5) return "medium";
  return "low";
}

/** Group memories by kind for the explorer sidebar. */
export function groupByKind(events: MemoryEvent[]): Record<MemoryKind, number> {
  const out: Record<MemoryKind, number> = { episodic: 0, semantic: 0, procedural: 0, working: 0 };
  for (const e of events) out[e.kind] += 1;
  return out;
}

/** Sort candidates for the "propose to remember" flow (newest + lowest confidence first). */
export function pendingProposals(events: MemoryEvent[]): MemoryEvent[] {
  return events
    .filter((e) => e.approval === "pending")
    .sort((a, b) => a.created_at.localeCompare(b.created_at));
}
