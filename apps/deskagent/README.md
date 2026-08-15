# DeskAgent

A local-first personal agent with a **self-memory system** (DEC-0009): memory is
stored locally, encrypted at rest, exportable, deletable, and never silently
written — every distilled memory write routes through an approval card.

Tauri 2 + React + TypeScript shell over a pure-Rust memory core.

## Architecture

```
apps/deskagent/
├── src/                      # React frontend (Vite)
│   ├── lib/                  # pure logic: sessions, memory filtering, approvals, bridge
│   ├── components/           # ChatWindow, SessionList, MemoryExplorer, PersonaCard, ApprovalCard
│   └── App.tsx               # shell: sidebar + chat/memory tabs
├── src-tauri/                # Tauri 2 shell
│   ├── src/lib.rs            # commands → deskagent-core (session/memory/approval/persona)
│   └── crates/deskagent-core # the memory engine (no Tauri dep, fully unit-tested)
│       └── src/
│           ├── store.rs      # SQLite store (rusqlite bundled): 4 memory kinds, scopes, approvals
│           ├── encrypt.rs    # AES-256-GCM at rest, PBKDF2 key derivation
│           ├── embed.rs      # deterministic HashEmbedder (default) + fastembed-rs (feature)
│           ├── capture.rs    # capture pipeline: raw episodes + extraction pass (5 turns, max 20)
│           ├── consolidation.rs  # persona regen (every 50 memories), dedupe, conflicts, decay
│           ├── retrieval.rs  # hybrid keyword+embedding retrieval, RRF, strict injection budget
│           ├── approvals.rs  # propose-to-remember cards + learning signal
│           └── sessions.rs   # session/message persistence
├── shared/schemas/           # memory-event.schema.json + zero-dep validator + tests
└── test/                     # frontend pure-logic tests (node:test)
```

## Memory model

| Kind | What it holds | Approval |
|------|---------------|----------|
| `episodic` | raw conversation turns (the user's own log) | stored directly |
| `semantic` | distilled facts & preferences (extraction pass) | **pending → approved/rejected** |
| `procedural` | workflow / how-to knowledge | **pending → approved/rejected** |
| `working` | in-flight context | **pending → approved/rejected** |

Scopes: `companion` (personal, applies everywhere) and `project` (only retrieved
for that project) — DEC-0009. Retrieval only ever sees **approved** memories.

## Run

```bash
npm install
npm test                 # 10 frontend logic tests
cargo test               # 35 core tests (SQLite store, crypto, capture, persona, retrieval)
cargo check              # workspace incl. Tauri shell
npm run tauri dev        # desktop app (needs tauri-cli + webkit2gtk on Linux)
```

The UI also runs in a plain browser (`npm run dev`) in demo mode — localStorage
backed, with seeded memories — so the shell is explorable without the desktop
runtime. In the Tauri shell the same commands hit the Rust core.

## At-rest encryption

Content columns are AES-256-GCM encrypted when a key is available:
`DESKAGENT_PASSPHRASE` env, else an auto-generated keyfile (`deskagent.key`,
mode 0600) in the app-data dir. The ciphertext-never-equals-plaintext property is
tested. OS keyring integration is a follow-up.

## Embeddings

Default `HashEmbedder` is deterministic and offline (P0 retrieval + tests).
Real semantic embeddings plug in behind the same `Embedder` trait with
`cargo build --features fastembed` (fastembed-rs + ONNX; downloads the
all-MiniLM-L12-v2 model on first use).

## Phase 6 hook

```
The chat's assistant reply is a placeholder ("model runtime lands in Phase 6").
```

Phase 6 replaced that: `chat_complete` runs the remembered backend/model with the
memory-injected system prompt, and falls back to a deterministic reply when the
runtime is offline (DEC-0005).

## Phase 6: runtime, skills, sandbox

- **Runtime layer** (`crates/deskagent-core/src/runtime/`): pluggable `Backend` trait;
  `OllamaBackend` (native `/api/chat`) and `LlamaCppBackend` (OpenAI-compatible
  `/v1/chat/completions` — the Metal path on Apple Silicon). A `ModelRegistry`
  lists models, chats, and persists the choice. Mock-server tests + a live smoke
  test (`cargo test -p deskagent-core -- --ignored ollama_live`) verified against a
  real local Ollama (qwen2.5-coder:7b).
- **Model picker** (`src/lib/picker.ts` + `src/components/ModelPicker.tsx`):
  consumes BenchKit via `shared/lib/will-it-run.mjs` with the bundled catalog
  (`src/lib/benchkit-catalog.ts`, generated from `shared/datasets/benchmarks.jsonl`
  by `scripts/sync-catalog.mjs`) as the offline fallback. Shows "runs on your
  machine / streams / no-fit" per model with RAM + speed estimates.
- **Skill integration** (`crates/deskagent-core/src/skills.rs`): install/update/remove
  skills from a SkillHub registry (Phase 3 API + `skillhub.json` + lockfile format),
  path-traversal guarded; installed skills surface as approval-gated procedural
  memory.
- **Action sandbox** (`crates/deskagent-core/src/sandbox.rs`): risky actions
  (shell/file/network) render as approval cards and are blocked until approved; a
  shared undo log records approved actions AND approved memory writes, with
  revert marking (reversal is the app layer's job).
- **Memory into conversation** (`crates/deskagent-core/src/conversation.rs`):
  persona + scoped retrieval hits (strict budget) injected into the system prompt;
  assistant messages carry "I remember…" citations with sources.
- **Voice + scheduled tasks** (`ChatWindow` mic stub + `TasksPanel` + `src/lib/tasks.ts`):
  getUserMedia placeholder transcript; local task list with due/roll/done logic.
