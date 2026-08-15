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

The chat's assistant reply is a placeholder ("model runtime lands in Phase 6").
The capture pipeline already runs on every message, so by the time the model
runtime lands, sessions are being remembered and proposals are waiting for
approval.
