// Bridge between the React UI and the Tauri Rust core. When running inside the
// desktop shell (`__TAURI_INTERNALS__` present) it invokes Rust commands; in a plain
// browser (vite dev / tests) it falls back to localStorage-backed demo data so the
// UI is explorable without the desktop runtime. All logic stays in the pure modules.

import type { ApprovalCard, MemoryEvent, Persona, Session } from "./types.ts";
import { createSession, addMessage, newId, nowIso } from "./sessions.ts";

interface InvokeFn {
  (cmd: string, args?: Record<string, unknown>): Promise<unknown>;
}

function tauriInvoke(): InvokeFn | null {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  if (!w.__TAURI_INTERNALS__) return null;
  return async (cmd, args) => {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke(cmd, args);
  };
}

const STORE_KEYS = {
  sessions: "deskagent.sessions",
  memories: "deskagent.memories",
  persona: "deskagent.persona",
  approvals: "deskagent.approvals",
};

function read<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: unknown): void {
  localStorage.setItem(key, JSON.stringify(value));
}

export interface DeskAgentBridge {
  listSessions(): Promise<Session[]>;
  createSession(projectId?: string): Promise<Session>;
  appendMessage(sessionId: string, role: "user" | "assistant", content: string): Promise<Session>;
  listMemories(): Promise<MemoryEvent[]>;
  listApprovals(): Promise<ApprovalCard[]>;
  getPersona(): Promise<Persona | null>;
}

function demoBridge(): DeskAgentBridge {
  const sessions = read<Session[]>(STORE_KEYS.sessions, []);
  const memories = read<MemoryEvent[]>(STORE_KEYS.memories, seedMemories());
  const approvals = read<ApprovalCard[]>(STORE_KEYS.approvals, []);
  const persona = read<Persona | null>(STORE_KEYS.persona, null);

  return {
    async listSessions() {
      return sessions;
    },
    async createSession(projectId) {
      const s = createSession(projectId);
      sessions.unshift(s);
      write(STORE_KEYS.sessions, sessions);
      return s;
    },
    async appendMessage(sessionId, role, content) {
      const i = sessions.findIndex((s) => s.id === sessionId);
      const updated = addMessage(sessions[i] ?? createSession(), role, content);
      if (i >= 0) sessions[i] = updated;
      else sessions.unshift(updated);
      write(STORE_KEYS.sessions, sessions);
      return updated;
    },
    async listMemories() {
      return memories;
    },
    async listApprovals() {
      return approvals;
    },
    async getPersona() {
      return persona;
    },
  };
}

async function rustBridge(): Promise<DeskAgentBridge> {
  const invoke = tauriInvoke();
  if (!invoke) throw new Error("no bridge");
  return {
    listSessions: async () => (await invoke("session_list")) as Session[],
    createSession: async (projectId) => (await invoke("session_create", { projectId })) as Session,
    appendMessage: async (sessionId, role, content) =>
      (await invoke("session_append", { sessionId, role, content })) as Session,
    listMemories: async () => (await invoke("memory_list")) as MemoryEvent[],
    listApprovals: async () => (await invoke("approval_list")) as ApprovalCard[],
    getPersona: async () => (await invoke("persona_get")) as Persona | null,
  };
}

let cached: DeskAgentBridge | null = null;

export function getBridge(): DeskAgentBridge {
  if (cached) return cached;
  cached = tauriInvoke() ? rustBridgeSafe() : demoBridge();
  return cached;
}

function rustBridgeSafe(): DeskAgentBridge {
  // Wrap so UI errors surface gracefully if a command is missing in dev builds.
  return new Proxy(
    {},
    {
      get(_t, prop) {
        return async (...args: unknown[]) => {
          const b = await rustBridge();
          const fn = (b as unknown as Record<string, (...a: unknown[]) => unknown>)[prop as string];
          return fn(...args);
        };
      },
    }
  ) as DeskAgentBridge;
}

function seedMemories(): MemoryEvent[] {
  const base = nowIso();
  return [
    {
      id: "demo-sem-1",
      kind: "semantic",
      content: "User prefers TypeScript over JavaScript for new services.",
      summary: "TypeScript preference",
      source: "extraction",
      confidence: 0.85,
      created_at: base,
      scope: { type: "companion" },
      approval: "approved",
      tags: ["preference", "language"],
    },
    {
      id: "demo-proc-1",
      kind: "procedural",
      content: "Deploy staging: run `bash scripts/deploy.sh staging`, then check the health endpoint.",
      source: "conversation",
      confidence: 0.7,
      created_at: base,
      scope: { type: "project", project_id: "bench-site" },
      approval: "approved",
    },
    {
      id: "demo-epi-1",
      kind: "episodic",
      content: "User reported the deploy pipeline failing on staging at 09:41.",
      source: "conversation",
      confidence: 0.9,
      created_at: base,
      scope: { type: "project", project_id: "bench-site" },
      approval: "approved",
    },
    {
      id: "demo-work-1",
      kind: "working",
      content: "Mid-refactor of the retrieval module; injection budget topic still open.",
      source: "synthesis",
      confidence: 0.6,
      created_at: base,
      scope: { type: "companion" },
      approval: "pending",
    },
  ];
}

export { newId, nowIso };
