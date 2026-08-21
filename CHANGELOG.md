# Changelog

Unified versioning: the canonical version lives in `VERSION`. Each product ships the same
semver at a given milestone (they may diverge in later releases). A release is a git tag
`v<version>` (e.g. `v0.1.0`) mapping to `VERSION`, `CHANGELOG.md`, and all product manifests —
enforced by `bash scripts/check-versions.sh`.

## [0.1.1] — 2026-08-21

Patch release. Builds on v0.1.0 with the release pipeline fully proven end-to-end.

### Added
- Native `.deb`/`.rpm` installers for the linux-amd64 CLIs (`scripts/build-native-installers.sh`).
- GitHub Actions release pipeline hardened for the full inventory (per-target artifacts, GPG
  signing, SBOM, GHCR container push, npm publish, `.deb`/`.rpm`).
- `slopgate` published to npm as `@merverli/slopgate` (scoped name; unscoped `slopgate` is
  rejected as too similar to the existing `slop-gate`).
- Release notes (`docs/RELEASE-v0.1.0.md`) + project GPG public key
  (`scripts/release-gpg-public.asc`) for verifying signed artifacts.

### Changed
- Unified versioning now enforces tag-vs-`VERSION` consistency (`check-versions.sh`).

## [0.1.0] — 2026-08-21

First milestone release. Linux (amd64/arm64) + macOS (arm64/amd64) targets; the DeskAgent
Tauri GUI is explicitly **deferred** (terminal CLI `deskagent` is the shipped interface).

### bench-site
- Searchable benchmark matrix, "Will it run?" calculator, model detail + quant charts, local runner skeleton.

### skillhub (CLI)
- `search` / `info` / `install` / `update` / `remove` / `verify` / `scan` / `harnesses`.
- `register <owner>` mints a scoped capability token + Ed25519 signing key.
- `publish` now requires a capability token and signs the package with the owner's key.

### skillhub-registry
- Canonical `owner/name` identity model; per-owner publish scope via scoped, revocable
  HMAC-signed capability tokens; anonymous reads.
- Publish integrity via per-owner Ed25519 signing (registry CA); key rotation + owner revocation.
- Input hardening (JSON-schema, semver, size caps, path traversal), rate limiting, abuse/DoS
  controls (DB size cap, quarantine opt-in, batched download writes), non-leaking structured errors.

### slopgate / slopgate-action / slopgate-dash
- Deterministic rule pack, `scan`/`score`/`lint`, GitHub Action w/ SARIF + threshold gating, dashboard.

### deskagent
- Local-first, approval-gated memory core; Tauri shell (deferred); terminal-UI CLI (`deskagent`)
  with chat, memory explorer, model picker, inline approvals, mobile (Moshi/SSH) narrow-width mode.

### Infrastructure
- `scripts/run-all-checks.sh` — 21 checks; artifact-hygiene guard; version-consistency check.
