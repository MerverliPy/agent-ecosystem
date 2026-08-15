# Milestone 2 — PHASES.md draft (for human review)

> **STATUS: DRAFT FOR REVIEW.**
> This file is a **proposed** breakdown of Milestone 2 into the content-locked
> `PHASES.md`. It is a standalone planning artifact and is **not** part of the locked
> plan. Nothing here is authoritative until the human:
>   1. reviews this draft,
>   2. runs `bash scripts/plan-lock.sh propose "<reason>"` (or directs the agent to),
>   3. pastes the approved content into `PHASES.md`,
>   4. runs `bash scripts/plan-lock.sh approve "<reason>"` from an interactive terminal.
>
> Conventions mirrored from the existing `PHASES.md` so the paste is mechanical:
> phase headers carry `<!-- STATUS --> <!-- DEPENDS_ON: ... -->`, each phase has a
> `<!-- VALIDATE: ... -->` command, tasks are `- [ ]` checkboxes, and each phase ends
> with a `**Exit criteria:**` line.

---

## Phase 8: DeskAgent CLI (terminal UI) <!-- PENDING --> <!-- DEPENDS_ON: Phase 7 -->
<!-- VALIDATE: bash scripts/plan-lock.sh verify && cd apps/deskagent && cargo build -p deskagent-cli && cargo test && bash scripts/run-all-checks.sh -->
- [ ] Add `deskagent-cli` binary crate as a third workspace member (`apps/deskagent/src-tauri/crates/deskagent-cli`), depending on `deskagent-core` with no business-logic changes to core.
- [ ] Introduce the TUI stack (ratatui + crossterm) per DEC-0004; wire a four-pane layout mirroring the web tabs (Chat / Memory+Approvals / Models / Tasks).
- [ ] Implement the chat loop: input → `capture_turn` → `chat_complete` (runtime or offline deterministic fallback, DEC-0005) → render with citations.
- [ ] Implement the model picker backed by `runtime_list_models` / `remembered_choice`.
- [ ] Implement inline approval cards (Y/n) resolving through `approval_decide`.
- [ ] Implement memory explorer + persona + export/wipe through `memory_list` / `persona_get` / `memory_export` / `memory_wipe`.
- [ ] Reuse the existing encryption key resolution (`DESKAGENT_PASSPHRASE` env, else 0600 keyfile) so at-rest encryption is not regressed (DEC-0009).
- [ ] Add CLI-specific tests plus a headless smoke (`deskagent chat` against a local Ollama model).
- **Exit criteria:** `deskagent-cli` builds and passes tests; `run-all-checks.sh` stays green; live chat smoke succeeds over the core (not the GUI); the Tauri GUI still compiles and is marked "deferred."

## Phase 9: SkillHub registry security (public multi-tenant) <!-- PENDING --> <!-- DEPENDS_ON: Phase 3 -->
<!-- VALIDATE: bash scripts/plan-lock.sh verify && (cd apps/skillhub-registry && cargo test) && (cd apps/skillhub-cli && cargo test) && bash scripts/run-all-checks.sh -->
- [ ] **MUST LAND FIRST:** normalize the package identifier model to a canonical `owner/name` used identically by schema and handlers; reset the runtime registry DB (no migration code) per the DECIDED note. The read/write key-space mismatch is unresolved until this lands — every later auth task depends on it.
- [ ] Enforce a package-name grammar and a single `canonical_id()` path for all lookups and publishes.
- [ ] Introduce owner namespaces with per-owner publish scope (only the owning identity may publish under `owner/*`).
- [ ] Add authentication/authorization for publish via self-contained, scoped, revocable capability tokens; keep read anonymous; never log or env-embed secrets.
- [ ] Add rate limiting (per-IP and per-token token buckets on publish; global read limits) using tower + governor or equivalent.
- [ ] Harden input validation: semver, manifest JSON-schema, package/file size caps, path-traversal guard on `files` keys, content-type checks, request body cap.
- [ ] Add publish integrity: package signing verified against a registry CA that issues per-owner keys; owner key rollover/revocation support.
- [ ] Harden transport/runtime: TLS termination guidance, bind-address policy, structured errors with no internal detail leakage, default-deny posture.
- [ ] Add abuse/DoS controls: max DB size, quarantine of unverified/`high_risk` packages behind explicit opt-in, batch download-count writes.
- [ ] Enforce artifact hygiene: runtime DB (`*.db`), seed tokens, and signing secrets stay out of git and out of any container image; add a guard so no build step copies them into a release artifact.
- [ ] Add an adversarial security test suite (path traversal, oversized, bad semver, unauthorized, signature mismatch) and keep the existing registry unit tests green.
- **Exit criteria:** unauthenticated publish is rejected; unauthorized owner publish is rejected; malicious fixtures fail validation; verified packages remain anonymously readable; all adversarial + existing tests green.

## Phase 10: Release and versioned distribution <!-- PENDING --> <!-- DEPENDS_ON: Phase 8, Phase 9 -->
<!-- VALIDATE: bash scripts/plan-lock.sh verify && bash scripts/run-all-checks.sh && bash scripts/plan-lock.sh status -->
- [ ] Establish unified versioning: single semver source per product, `CHANGELOG.md`, git tags mapping to releases.
- [ ] Build a GitHub Actions release pipeline (tag → build → sign → publish) with a linux (amd64/arm64) + macOS (arm64/amd64) matrix; DEC-0005 compliant (no telemetry).
- [ ] Produce CLI installers via `cargo dist`/`cargo-binstall` for `skillhub` and `deskagent`, plus `install.sh`/Homebrew/`.deb`/`.rpm`.
- [ ] Build a registry container image (Dockerfile) plus systemd/compose units so the secured registry is deployable.
- [ ] Produce web distribution: static builds for `bench-site`, `skillhub-web`, `slopgate-dash` with a named deploy target.
- [ ] Publish `slopgate` as an npm package/tarball and `slopgate-action` as a versioned GitHub Action.
- [ ] Add release-artifact signing and provenance (checksums, signing keys, SBOM); defer code-signing policy to the GUI milestone but plan key rotation.
- [ ] Add a **separate** `scripts/release-gate.sh` (release-only checks: artifact presence, checksum verification, version consistency across manifests) and wire it into the CI tag pipeline; do not loosen the existing `run-all-checks.sh` (which remains the general CI surrogate). Add version/self-update or binstall upgrade paths for CLI products.
- **Exit criteria:** a tagged release produces signed, verifiable artifacts for all in-scope products; a clean clone of a published binary passes `scripts/release-gate.sh`; the secured registry is deployable from its container; no runtime DB, seed token, or signing secret is baked into any release artifact; DeskAgent GUI remains excluded and deferred.

---

## Definition of Done (Milestone 2) — proposed

- Phases 8–10 all `COMPLETE`; zero tasks cancelled without documented reason in PROGRESS.md.
- `deskagent` CLI builds, passes tests, and chats with a local model end-to-end; Tauri GUI still compiles and is explicitly deferred.
- Registry rejects unauthenticated and unauthorized publishes, validates adversarial inputs, quarantines unverified/`high_risk` packages, and stays anonymously readable for verified packages.
- A tagged release produces signed, checksummed artifacts for `skillhub`, `deskagent`, `skillhub-registry` (container), `slopgate`, `slopgate-action`, and the three web apps, across linux (amd64/arm64) + macOS (arm64/amd64).
- `plan-lock.sh verify` passes at every checkpoint; PROGRESS.md contains per-phase handoffs for Phases 8–10.

---

## Notes for the human reviewer (not part of PHASES.md)

- **Dependency tags:** I marked Phase 9 as `DEPENDS_ON: Phase 3` (its codebase is the registry, already complete) rather than Phase 8, since the CLI (Phase 8) and registry security (Phase 9) are independent and can run in parallel. Phase 10 depends on both. If you prefer strict sequential numbering semantics (`Phase 9` after `Phase 8`), say so and I'll adjust.
- **Excluded vs. included:** Windows, the Tauri GUI, OIDC-only auth, voice/skill-invoke are all out of scope this milestone (recorded in the DECIDED notes).
- **Lock ceremony:** none of this is active until you run `propose` → edit `PHASES.md` → `approve`. The checklist text above is written to paste verbatim with only `[ ]`→`[x]` and status-comment changes during execution.
