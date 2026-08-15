// Session persistence logic: pure session/message helpers. The actual persistence
// layer is the Rust `sessions` module (SQLite); these functions are the client-side
// contract used by both the UI and the bridge.

import type { ChatMessage, ChatRole, Session } from "./types.ts";

export function newId(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function nowIso(): string {
  return new Date().toISOString();
}

export function createSession(projectId?: string): Session {
  const now = nowIso();
  return {
    id: newId("sess"),
    title: "New conversation",
    project_id: projectId,
    created_at: now,
    updated_at: now,
    messages: [],
  };
}

export function addMessage(session: Session, role: ChatRole, content: string, citations?: ChatMessage["citations"]): Session {
  const message: ChatMessage = {
    id: newId("msg"),
    session_id: session.id,
    role,
    content,
    created_at: nowIso(),
    ...(citations ? { citations } : {}),
  };
  return {
    ...session,
    messages: [...session.messages, message],
    updated_at: nowIso(),
    title: autoTitle(session.title, role, content),
  };
}

/** Keep the title meaningful: first user message becomes the title (truncated). */
export function autoTitle(current: string, role: ChatRole, content: string): string {
  if (current !== "New conversation") return current;
  if (role !== "user") return current;
  const oneLine = content.trim().replace(/\s+/g, " ");
  return oneLine.length > 60 ? `${oneLine.slice(0, 59)}…` : oneLine || "New conversation";
}

export function messageCount(session: Session): number {
  return session.messages.length;
}

/** Sort sessions most-recently-updated first. */
export function sortSessions(sessions: Session[]): Session[] {
  return [...sessions].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
}

/** Trim a session's history for context injection (keeps the last N messages). */
export function trimForContext(session: Session, keep = 40): ChatMessage[] {
  return session.messages.length > keep ? session.messages.slice(-keep) : session.messages;
}

/** Build the injection budget warning when context would be too large. */
export function contextBudget(messages: ChatMessage[], budgetChars = 12000): { used: number; budget: number; over: boolean } {
  const used = messages.reduce((n, m) => n + m.content.length, 0);
  return { used, budget: budgetChars, over: used > budgetChars };
}
