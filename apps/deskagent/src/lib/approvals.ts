// Approval card logic: every memory write routes through propose-to-remember
// (DEC-0009). Pure state transitions; the Rust core records the learning signal.

import type { ApprovalCard, ApprovalStatus, MemoryEvent } from "./types.ts";
import { newId, nowIso } from "./sessions.ts";

export function memoryApprovalCard(event: MemoryEvent): ApprovalCard {
  return {
    id: newId("appr"),
    kind: "memory_write",
    description: `Remember: ${event.content.slice(0, 120)}`,
    event,
    created_at: nowIso(),
    status: "pending",
  };
}

export function actionApprovalCard(description: string): ApprovalCard {
  return {
    id: newId("appr"),
    kind: "action",
    description,
    created_at: nowIso(),
    status: "pending",
  };
}

export function decide(card: ApprovalCard, status: "approved" | "rejected"): ApprovalCard {
  return { ...card, status };
}

export function pendingCards(cards: ApprovalCard[]): ApprovalCard[] {
  return cards.filter((c) => c.status === "pending");
}

/** Recorded learning signal: an approved write strengthens confidence, a rejected one weakens it. */
export function confidenceDelta(status: ApprovalStatus): number {
  return status === "approved" ? 0.1 : status === "rejected" ? -0.1 : 0;
}

export function applyConfidence(event: MemoryEvent, delta: number): MemoryEvent {
  const next = Math.max(0, Math.min(1, event.confidence + delta));
  return { ...event, confidence: Number(next.toFixed(3)) };
}
