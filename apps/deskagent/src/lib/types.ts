// DeskAgent shared types — mirror the memory-event schema (shared/schemas) and the
// Rust core (src-tauri/crates/deskagent-core). Kept dependency-free for tests.

export type MemoryKind = "episodic" | "semantic" | "procedural" | "working";
export type MemorySource =
  | "conversation"
  | "user"
  | "file"
  | "api"
  | "reflection"
  | "extraction"
  | "synthesis"
  | "other";
export type ApprovalStatus = "pending" | "approved" | "rejected";
export type ScopeType = "companion" | "project";

export interface MemoryScope {
  type: ScopeType;
  project_id?: string;
  project_path?: string;
}

export interface MemoryEvent {
  id: string;
  kind: MemoryKind;
  content: string;
  summary?: string;
  source: MemorySource;
  confidence: number;
  created_at: string;
  updated_at?: string;
  episode_id?: string;
  scope: MemoryScope;
  approval: ApprovalStatus;
  tags?: string[];
}

export type ChatRole = "user" | "assistant" | "system";

export interface ChatMessage {
  id: string;
  session_id: string;
  role: ChatRole;
  content: string;
  created_at: string;
  /** Optional memory citations: "I remember…" with sources. */
  citations?: MemoryEvent[];
}

export interface Session {
  id: string;
  title: string;
  project_id?: string;
  created_at: string;
  updated_at: string;
  messages: ChatMessage[];
}

export interface Persona {
  version: number;
  generated_at: string;
  summary: string;
  facts: string[];
  preferences: string[];
  skills: string[];
  memories_count: number;
}

export interface ApprovalCard {
  id: string;
  kind: "memory_write" | "action";
  description: string;
  event?: MemoryEvent;
  created_at: string;
  status: ApprovalStatus;
}
