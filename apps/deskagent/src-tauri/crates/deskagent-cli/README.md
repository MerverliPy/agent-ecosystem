# deskagent-cli — terminal UI + headless commands

A third workspace member of the DeskAgent workspace (`apps/deskagent`). The CLI
is a pure-Rust **ratatui + crossterm** TUI (DEC-0004) over the `deskagent-core`
memory engine, plus headless subcommands for scripts and CI. It mirrors the web
app's tabs with a four-pane layout and reuses the core **exactly** as the Tauri
shell does — no business-logic changes to core.

```
crates/deskagent-cli/
├── Cargo.toml
└── src/
    ├── main.rs     # clap CLI: TUI (default) + chat/models/approvals/memory/persona/export/wipe
    ├── data.rs     # data dir + at-rest encryption key resolution (mirrors the shell)
    ├── chat.rs     # chat engine: capture_turn → extraction pass → complete with citations
    ├── app.rs      # TUI state: tabs, key handling, inline Y/n approval flow
    └── ui.rs       # pure ratatui rendering (TestBackend-tested)
```

## Build & test

```bash
cd apps/deskagent
cargo build -p deskagent-cli        # binary: target/debug/deskagent
cargo test -p deskagent-cli         # 27 unit tests + 1 ignored live smoke
cargo test -p deskagent-cli -- --ignored deskagent_chat_live   # needs a local Ollama
```

## Run

```bash
deskagent                    # interactive four-pane TUI (Chat / Memory+Approvals / Models / Tasks)
deskagent chat "Hello"       # one-shot: capture + complete, prints the reply with citations
deskagent chat "Hi" --session sess-…        # reuse a session
deskagent models             # list backend models; remembered choice printed
deskagent models --pick llama3.2:3b         # remember a default model
deskagent approvals          # approval cards; approve/reject <id> (short prefix ok)
deskagent memory             # all memories; --approved, --kind semantic|procedural|…
deskagent persona            # persona card
deskagent export --out x.json               # DEC-0009 export
deskagent wipe --yes         # DEC-0009 delete everything (guarded)
```

`--data-dir <dir>` overrides the store location (`DESKAGENT_DATA_DIR` env, else
`~/.local/share/deskagent`). `--backend ollama|llama.cpp` and `--base-url`
override the runtime; a model given with `--model` is remembered as the default.

## TUI keys

| Key | Where | Action |
|-----|-------|--------|
| `Tab` / `Shift-Tab` | anywhere | switch pane |
| `Esc` / `Ctrl-C` | anywhere | quit |
| `Enter` | Chat | send the input line |
| `y` / `n` | Memory+Approvals | approve / reject the `▶` approval card (inline) |
| `j` / `k` | Memory / Models | scroll / move selection |
| `Enter` / `m` | Models | remember the selected model |
| `r` | Memory / Models | reload |
| `e` | Memory | export memories to a JSON file in the data dir |

## Chat engine (DEC-0005, DEC-0009)

One turn = `capture_turn` (raw episode) → scheduled extraction pass (every 5
user turns, max 20 proposals) → `build_chat_context` (persona + scoped
retrieval, strict injection budget) → backend `chat` → assistant message with
"I remember…" citations. When the runtime is unreachable the CLI replies with a
deterministic fallback instead of erroring — offline-first by default; nothing
phones home.

## Encryption (DEC-0009)

Identical key resolution to the Tauri shell: `DESKAGENT_PASSPHRASE` (persisted
salt) wins; otherwise a 0600 `deskagent.key` keyfile is generated on first use.
Memory content columns are AES-256-GCM encrypted at rest; the status bar shows
which source is active. The CLI and GUI can read the same store when they share
a data dir.

## Live smoke (Phase 8 exit criterion)

```bash
DESKAGENT_DATA_DIR=$(mktemp -d) cargo run -p deskagent-cli -- chat "Hello, DeskAgent."
```

Against a running local Ollama this auto-picks the first reachable model and
replies with real text + citations. Covered by the `#[ignore]`d
`deskagent_chat_live` test (run with `--ignored`).
