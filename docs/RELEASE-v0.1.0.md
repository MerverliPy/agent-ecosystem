# Release v0.1.0

**First milestone release** of the agent-ecosystem monorepo: four products that work together
to build, verify, and deploy AI coding-agent skills — **BenchKit**, **SkillHub**, **SlopGate**,
and **DeskAgent**.

Release tag: `v0.1.0` · Milestone 2 accepted (Phases 1–8, 8.5, 9, 10 all complete) ·
`run-all-checks.sh` green (21 checks) · artifacts GPG-signed.

---

## What's included

### BenchKit
- Searchable hardware/quantization benchmark matrix ("Will it run?" calculator).
- Model detail pages with quantization-quality charts; local runner skeleton.

### SkillHub (registry + CLI)
- A secured, public multi-tenant skill registry (`skillhub-registry`):
  - Canonical `owner/name` identity model; per-owner publish scope.
  - Scoped, **revocable** capability tokens (anonymous reads).
  - **Package signing** via a per-owner Ed25519 key issued by the registry CA; key rotation.
  - Input hardening (JSON-schema, semver, size caps, path-traversal), rate limiting,
    quarantine of unverified/high-risk packages, abuse/DoS controls.
- `skillhub` CLI: `search / info / install / update / remove / verify / scan / register / rotate`.
- 27-rule security scanner; SlopGate quality check in `verify`.

### SlopGate
- Deterministic rule pack + `scan`/`score`/`lint`, a GitHub Action with SARIF + threshold
  gating, and a per-repo score dashboard.

### DeskAgent
- Local-first, approval-gated memory core (episodic/semantic/procedural + persona).
- `deskagent` terminal-UI CLI: chat with citations, memory explorer, model picker, inline
  approvals, and mobile (Moshi/SSH) narrow-width mode.
- The Tauri GUI is **deferred**; the CLI is the shipped interface.

---

## Install

**CLI binaries** (from this release):
```bash
# skillhub
curl -sSL https://raw.githubusercontent.com/MerverliPy/agent-ecosystem/main/install.sh | bash -s skillhub
# deskagent
curl -sSL https://raw.githubusercontent.com/MerverliPy/agent-ecosystem/main/install.sh | bash -s deskagent
```
Or install the release tarball / Homebrew formula directly from the assets below.

**Registry container:**
```bash
docker pull ghcr.io/merverlipy/agent-ecosystem/skillhub-registry:v0.1.0
```
See `deploy/` (docker-compose + Caddy TLS + systemd unit) for deployment.

---

## Artifacts

| Kind | Files |
|------|-------|
| CLI binaries (4 targets each) | `skillhub-*`, `deskagent-*` (linux amd64/arm64, macOS amd64/arm64) |
| Provenance | `SHA256SUMS`, `SHA256SUMS.sig` (GPG), `SBOM.json` |
| Installers | `skillhub-cli-installer.sh`, `skillhub-cli.rb` (Homebrew), `skillhub-cli-x86_64-unknown-linux-gnu.tar.xz` |
| Web static dists | `web.tar.gz` (bench-site, skillhub-web, slopgate-dash) |
| Action | `slopgate-action.tar.gz` |
| npm package | `@merverli/slopgate` (scoped name; publishable tarball attached) |

---

## Security & verification

- Release artifacts are **GPG-signed**; the public key is committed at
  `scripts/release-gpg-public.asc`. Verify with:
  ```bash
  gpg --import scripts/release-gpg-public.asc
  gpg --verify SHA256SUMS.sig SHA256SUMS
  sha256sum -c SHA256SUMS
  ```
- No runtime DB, seed token, or signing secret is baked into any artifact (enforced by
  `scripts/check-artifact-hygiene.sh` and `scripts/release-gate.sh`).
- License: MIT (`LICENSE`). DEC-0005 compliant (no telemetry).

---

## Known limitations / deferred

- **npm publish**: `slopgate` is published under the scoped name **`@merverli/slopgate`** (npm
  rejects the unscoped `slopgate` as too similar to the existing `slop-gate`). Uses an npm
  Automation token; a publishable tarball is also attached to this release.
- **`.deb`/`.rpm`** installers require `cargo dist init` in the two Rust workspaces
  (documented); the shell/Homebrew installers and per-target tarballs are provided now.
- macOS installers via cargo-dist are produced on macOS runners in a follow-up.
- DeskAgent Tauri GUI remains deferred.

---

_Generated from the v0.1.0 milestone. See `CHANGELOG.md` and `records/final-handoff.md`._
