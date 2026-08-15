# Milestone 2 Planning — DeskAgent CLI (TUI), Registry Security, Distribution

> **STATUS: PLANNING ONLY.**
> This document is a **standalone planning artifact**. No code has been written, no
> task has been executed, and no change request has been opened against the locked
> plan. Nothing in this file is authoritative until a human reviews and explicitly
> approves it; only then does it move into the content-locked `PHASES.md` (via the
> human-led `approve` ceremony, per AGENTS.md §2 / DEC-0008).

## Decisions locked by the human (2026-08-15)

| Decision | Choice |
|----------|--------|
| SkillHub registry trust model | **Public multi-tenant** (full authN/authZ, signing, rate limiting, DoS controls, TLS) |
| DeskAgent TUI positioning | **DeskAgent CLI product tier** (separate first-class surface; GUI kept and deferred to a stable release) |
| Release/distribution scope | **CLIs**, **Registry server**, **Web apps** (GUI excluded) |
| Planning depth | **Standalone doc only** — do NOT open a lock change request yet |

---

## 1. Architecture ground truth (verified against the repo)

These facts are load-bearing for every task below and were confirmed by reading the
current source, not assumed:

1. **`deskagent-core` is Tauri-free by construction.**
   `apps/deskagent/Cargo.toml` declares a workspace with two members:
   `src-tauri` (the shell) and `src-tauri/crates/deskagent-core` (the product).
   The core (`memory`, `retrieval`, `runtime/{ollama,llama_cpp}`, `skills`, `sandbox`,
   `approvals`, `sessions`, `conversation`, `store`, `encrypt`, `capture`,
   `consolidation`, `embed`) has **no `tauri` dependency**. So a CLI reuses 100% of the
   business logic; `src-tauri/src/lib.rs` is just a thin `#[tauri::command]` shim.

2. **The React UI is presentation-only.** `App.tsx` + 8 components render four tabs
   (Chat / Memory+Approvals / Models / Tasks) and call the core exclusively through
   `bridge.ts`, which already abstracts the command surface and includes a
   localStorage/demo fallback. The TUI is a **second client** of the same core, not a
   rewrite.

3. **The registry is a bare axum+SQLite server** (`apps/skillhub-registry/src/main.rs`):
   five endpoints (`/health`, `/api/search`, `/api/packages/{owner}/{name}`,
   `/api/packages/{owner}/{name}/{version}/files`, `/api/publish`), no auth, no rate
   limiting, no body/size limits, a global `Mutex<Connection>` for all DB access, and
   binds `127.0.0.1` by default but is otherwise deployable anywhere. `POST /api/publish`
   accepts unauthenticated arbitrary JSON with arbitrary `files` content.

4. **Identifier-model inconsistency (critical).** The read handlers take
   `Path<(owner, name)>` and key packages as `format!("{owner}/{name}")`, but the
   `packages` schema uses a single `name TEXT PRIMARY KEY`, and `publish` writes a bare
   manifest `name` (no owner). Read and write disagree on what a package identity is.
   **Every auth decision is unsound until this is normalized.** This is the first
   concrete fix in the security plan.

5. **Release artifact inventory** (confirmed on disk):
   - Rust CLIs/daemons: `skillhub-cli` (bin `skillhub`), `skillhub-registry`,
     `deskagent` (Tauri shell — *excluded*), and the new `deskagent` CLI.
   - Node CLIs: `slopgate` (bin in `apps/slopgate/bin`), `slopgate-action`
     (GitHub Action, `action.yml`).
   - Next.js web apps: `bench-site`, `skillhub-web`, `slopgate-dash`.

---

## 2. Plan A — DeskAgent CLI (terminal UI)

**Goal.** Ship a first-class terminal client ("DeskAgent CLI") that exercises the full
local-first agent loop (chat with local model + memory + approvals + skills) over
`deskagent-core`, with the existing React/Tauri GUI preserved and deferred to a later
stable release.

**Non-negotiable constraints.**
- Zero refactor of `deskagent-core` business logic; only additive presentation/entrypoint.
- No regression of DEC-0009 at-rest encryption (reuse `resolve_key`: `DESKAGENT_PASSPHRASE` env, else 0600 keyfile, else documented plaintext fallback).
- DEC-0005: no mandatory telemetry; offline deterministic fallback preserved.

### Tasks

| # | Task | Notes / mapping |
|---|------|-----------------|
| A1 | New binary crate `src-tauri/crates/deskagent-cli` (depends on `deskagent-core`) | Add as third workspace member; `[[bin]] name = "deskagent"` |
| A2 | Terminal UI stack: **ratatui** + **crossterm** | Rust-native, DEC-0004 "Rust for CLIs" |
| A3 | Four-pane IA mirroring the web tabs: Chat / Memory+Approvals / Models / Tasks | 1:1 with `App.tsx` |
| A4 | Chat loop: input → `capture_turn` → `chat_complete` (model or offline fallback) → render + citations | Maps to `handleSend`/`chat_complete`; reuses `build_chat_context` |
| A5 | Model picker via `runtime_list_models` / `remembered_choice` | Maps to `ModelPicker.tsx` |
| A6 | Inline approval cards (Y/n) → `approval_decide` | Maps to `ApprovalCard`/`onDecide` |
| A7 | Memory explorer + persona + export/wipe | Maps to `MemoryExplorer`/`PersonaCard`/`memory_export`/`memory_wipe` |
| A8 | Encryption parity: reuse `resolve_key` into the CLI's `open_store` | No DEC-0009 regression |
| A9 | CLI-specific tests + a headless smoke (`deskagent chat` vs local Ollama) | `cargo build -p deskagent-cli` + `cargo test` + live smoke |

### Validation gate
`cargo build -p deskagent-cli` (0) · `cargo test` (0) · `run-all-checks.sh` still 19/19 ·
live Ollama smoke (0). Tauri GUI still compiles but is explicitly marked "deferred."

### Deliberate trade-offs / risks (flagged for the human)
- The TUI does **not** carry over markdown-rich rendering, inline images, or mouse-native
  desktop UX. That is the point of a separate CLI tier, but it serves a power/headless
  user segment, not the consumer GUI segment.
- `embed.rs`/fastembed (vector search) status is unchanged by the TUI.
- Feature parity is "core loop + memory + approvals + skills," **not** voice/tasks/skill-invoke (those remain product stubs regardless of UI).

---

## 3. Plan B — SkillHub Registry security (public multi-tenant)

**Threat model (public multi-tenant).** The registry is reachable by untrusted clients;
package namespaces are owned and contested; publish must be authenticated and
authorized; read must survive abuse. Anonymous read of *verified* packages is allowed;
unverified/`high_risk` packages are quarantined behind explicit opt-in.

### B0 — Normalize the identifier model (blocking prerequisite)
- Introduce a canonical identity `owner/name` used consistently by **both** schema and
  handlers. Migration for existing rows (assign a default/synthetic owner), or reset the
  DB in pre-release. No auth work proceeds until read and write share one key space.
- Enforce a package-name grammar (e.g. `^[a-z0-9](?:[-_a-z0-9]*[a-z0-9])?$`) and route
  names through a single `canonical_id()`.

### Tasks

| # | Task | Notes |
|---|------|-------|
| B1 | Owner namespaces + publish scope | Per-owner publish authorization; only the owner may publish under `owner/*` |
| B2 | AuthN/AuthZ | Scoped publish tokens (capability token per owner, revocable). Read stays anonymous; write is authenticated. No secrets in env/logs. |
| B3 | Rate limiting | Token bucket per-IP **and** per-token on `/api/publish`; global read limits to stop scraping/DoS (tower + governor or hand-rolled) |
| B4 | Input validation + limits | Semver enforcement, manifest JSON-schema validation, package/file **size caps**, path-traversal guard on `files` keys, content-type checks, request body cap |
| B5 | Publish integrity + provenance | Sign packages at publish; registry verifies signature. Pairs with existing `sha2` in `skillhub-cli`. Optional CLI-side verification on install. |
| B6 | Transport + runtime hardening | TLS termination guidance, bind-address policy, secret injection model, structured error handling (no internal detail leak), default deny. |
| B7 | Abuse/DoS controls | Max DB size, quarantine `high_risk`/unverified behind explicit opt-in, download-count write batching (remove per-read write amplification). |
| B8 | Security test suite | Adversarial publish fixtures (path traversal, oversized, bad semver, unauthorized, signature-mismatch) + unit/integration tests; keep existing 6/6 tests green. |

### Validation gate
`cargo test -p skillhub-registry` (all, incl. new adversarial suites) (0) ·
`skillhub-cli` e2e (14/14) still green against the secured server · manual authN/AuthZ
spot-check · `run-all-checks.sh` still 19/19.

---

## 4. Plan C — Installers, releases, versioned distribution

**Sequencing rule: Plan B must land before Plan C ships the registry/CLI**, otherwise
we distribute software that talks to an unsecured publish endpoint. Plan A is
independent and may proceed in parallel with B.

### Tasks

| # | Task | Notes |
|---|------|-------|
| C1 | Unified versioning + changelog | Single semver source per product; `CHANGELOG.md`; git tags map to releases |
| C2 | Release pipeline (GitHub Actions) | Build matrix: target/OS/arch per artifact; release on tag. DEC-0005 compliant. |
| C3 | CLI installers | `cargo dist`/`cargo-binstall` for Rust CLIs; `install.sh`; Homebrew; `.deb`/`.rpm` |
| C4 | Registry distribution | Dockerfile + container image; systemd/compose unit (so the *secured* registry is actually deployable) |
| C5 | Web app distribution | `bench-site`, `skillhub-web`, `slopgate-dash` static builds + named deploy target |
| C6 | Node CLI/action distribution | `slopgate` npm package + tarball; `slopgate-action` as a versioned GitHub Action |
| C7 | Signing + provenance | Release-artifact checksums, signing keys, SBOM; code-signing policy deferred to GUI release but key rotation planned |
| C8 | Release gate + upgrade path | Extend `run-all-checks.sh` into a release gate; `skillhub`/`slopgate` version flags + self-update or binstall flow |

### Artifact matrix (target)

| Product | Type | Distribution |
|---------|------|--------------|
| `skillhub` | Rust CLI | bins (.deb/.rpm/Homebrew/install.sh) |
| `skillhub-registry` | Rust daemon | container + systemd/compose |
| `deskagent` CLI | Rust CLI | bins (same path as skillhub) |
| `slopgate` | Node CLI | npm + tarball |
| `slopgate-action` | GitHub Action | versioned action tag |
| `bench-site` / `skillhub-web` / `slopgate-dash` | Next.js | static build + deploy target |
| DeskAgent GUI | Tauri | **deferred** (out of scope this milestone) |

---

## 5. Proposed phase structure & dependencies

```
Phase A  (DeskAgent CLI)     ────────────────►   parallel with B
Phase B  (registry security) ────────────────►   MUST precede C
Phase C  (release/distribution) ─────────────►   depends on B
```

Suggested execution order if approved:
1. **B0** (identifier normalization) — unblocks all of B.
2. **A** (CLI) — independent, can be worked in parallel with B.
3. **B1–B8** (security layers).
4. **C** (distribution) — only after B's validation gate passes.

---

## 6. Open items — DECIDED (2026-08-15)

1. **Canonical package identity — DB reset accepted.** The registry DB is a runtime,
   gitignored, developer-seeded artifact with no real users or published packages yet.
   Migration buys nothing at pre-release. **Resolution:** reset the DB under the new
   `owner/name` schema; record a migration note but ship no migration code.

2. **Identity provider — self-contained capability tokens.** Capability tokens are
   self-issued, revocable, and scoped per owner, with no external IdP dependency —
   consistent with the local-first / no-mandatory-cloud ethos (DEC-0005). OIDC/GitHub is
   a future *optional* auth front that mints capability tokens, not a prerequisite.
   **Resolution:** capability tokens first; OIDC deferred and layered later if at all.

3. **Signing key model — single registry CA issuing per-owner keys.** A single CA keeps
   the trust root simple (one public key to verify against) while still binding each
   owner to a distinct, revocable identity and namespace. This is the `npm`/`crates.io`
   central-trusted-publisher pattern suited to a public multi-tenant registry.
   **Resolution:** registry CA issues per-owner credentials; owner rollover/revocation
   is part of B2/B5.

4. **Release cadence + targets — narrow first.** v0.1.0 targets linux (amd64 + arm64)
   and macOS (arm64 + amd64). Windows deferred to the GUI milestone (changes installer
   tooling and CI cost, and DeskAgent GUI is already deferred). Milestone cadence gated
   by `run-all-checks.sh`. **Resolution:** linux + macOS at v0.1.0; Windows deferred.

5. **CLI interface — ratatui + crossterm, in-app scrollback.** ratatui is the mature Rust
   TUI standard (DEC-0004) and won't become a self-written maintenance tax. Chat history
   scrolls **in-app** (ratatui scrollable list); post-exit shell scrollback is an
   optional print-and-exit flag, not a default. No custom terminfo/alt-screen code.
   **Resolution:** ratatui + crossterm; in-app scrollable history as default.

---

## 7. Do not proceed unless authorized

Nothing above is queued for execution. The correct on-file channel, *when the human
approves*, is `bash scripts/plan-lock.sh propose "<reason>"` (AGENTS.md §2), after which
the human edits `PHASES.md` and re-locks with `approve` from an interactive terminal.
Until that happens, this document is advisory only.
