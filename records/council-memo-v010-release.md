# Council Decision Memo — v0.1.0 release readiness

- **Date:** 2026-08-21
- **Mode:** council-mode (supervisor-mediated). Roster: `council-architect`, `council-operator`, `council-skeptic` (all fresh context). Passes run: 1 (independent reports). Pass 2 skipped — council converged on an unambiguous recommendation; no material dispute remained that could alter it.
- **Question:** Is `agent-ecosystem` ready to publish a public `v0.1.0` (git tag `v0.1.0` → `.github/workflows/release.yml` → GitHub Release; optional `npm publish` of `apps/slopgate`)?

## Recommendation

**NOT READY — do not push `v0.1.0` as currently configured.** The checked-in release workflow is deterministically unable to complete and would not produce the release inventory it claims. (Architect: "sound in outline, but not coherent enough to ship." Operator: "NOT READY… will fail on CI and ship broken/incomplete artifacts." Skeptic: "VETO.")

The plan content (all phases COMPLETE, lock verify PASS, local `run-all-checks` 21/21) is not the blocker; the **release pipeline implementation** is.

## Verified blockers (deterministic — confirmed by direct read of committed files)

| # | Severity | Finding | Fix path |
|---|----------|---------|----------|
| 1 | BLOCKER | `release.yml` has **no `npm ci`/`npm install`** before `run-all-checks`, which runs web tests/`npx tsc`/Next/Vite. Clean runners have no `node_modules` → web checks fail. | Add `npm ci` per app (or root) before run-all-checks. |
| 2 | BLOCKER | `release-gate.sh` requires `dist/release/SBOM.json` (check 5), but `release.yml` **never calls `gen-sbom.sh`** → gate fails on all matrix jobs. | Add a `gen-sbom.sh` step before the gate. |
| 3 | BLOCKER | Artifact **naming mismatch + matrix collision**: all 4 jobs copy bare `skillhub`/`deskagent` into `dist/release/` and upload `dist/release/*`; `install.sh` downloads `${CLI}-${TARGET}`. Concurrent jobs overwrite; installs 404. | Emit per-target names (`skillhub-<target>`), align `install.sh` + gate + upload glob. |
| 4 | BLOCKER | Linux jobs fail at `deskagent cargo check (Tauri shell)` in run-all-checks (needs `libwebkit2gtk-4.1-dev`/`libgtk-3-dev`; not installed). | Install webkit/gtk deps on linux jobs (or scope the Tauri check out of CI). |
| 5 | BLOCKER | **No `LICENSE` file**; `apps/slopgate/package.json` and `skillhub-cli`/`skillhub-registry` Cargo manifests lack `license` metadata (conflicts with DEC-0002). | Add LICENSE text + per-crate/package license fields. |
| 6 | HIGH | Signing is **not wired**: `RELEASE_GPG_KEY` is only a comment; `sign-artifacts.sh` treats no-GPG as success (unsigned); the gate never verifies a signature. | Decide signed-vs-unsigned for v0.1.0; if signed, import key + require sig. |
| 7 | HIGH | In-scope artifacts **not published**: registry container built but not pushed (no GHCR); web dists stay in `dist/web` (not uploaded); `slopgate-action` has no step; no `.deb`/`.rpm`/cargo-dist installers are produced (workflow never runs `cargo dist`). | Narrow v0.1.0 scope to CLI binaries, or wire each destination in. |
| 8 | HIGH | `npm publish` step is **dead** (`if` tests `env.NPM_TOKEN`, but NPM_TOKEN is only in step `env:`, evaluated after the `if`) and needs namespace ownership. | Gate on `secrets.NPM_TOKEN`, or cut npm from v0.1.0. |
| 9 | HIGH | `install.sh` default `REPO=agent-ecosystem` yields invalid URLs (no owner); actual repo is `MerverliPy/agent-ecosystem`. | Fix default to `MerverliPy/agent-ecosystem`. |
| 10 | MED | `check-versions.sh` never compares the release tag to `VERSION` and misses `src-tauri/tauri.conf.json`; final-handoff "11/11" miscounts (10 checks). | Add tag-vs-VERSION check + tauri.conf; correct count. |
| 11 | MED | `deploy/skillhub-registry.service` uses `User=nobody` + `ReadWritePaths=/var/lib/skillhub-registry` without creating/owning that dir → fresh-host deploy may fail. | Add `StateDirectory=skillhub-registry`. |
| 12 | MED | Root `README.md` still says Milestone 2 "in progress", Phases 9–10 "pending" — contradicts PHASES.md + handoff. | Update README. |
| 13 | MED | `Cargo.lock` is gitignored (mutable dependency ranges); SBOM is lightweight existence-only; `.dockerignore` sits below the root context used by the workflow; hygiene gate only depth-1 name checks. | Reproducibility/attestation hardening (may be deferred, but claims should not overstate). |

## Accepted feedback

- All operator + skeptic blockers above (verified). Also accepted: `VERSION`, manifests, `output:"export"` ×3, Dockerfile/.dockerignore/deploy layout, `check-artifact-hygiene.sh` soundness, `sign-artifacts`/`gen-sbom` script correctness, and the `owner/name`/auth/validation work in Phase 9 were found sound. These positives do not overcome the workflow blockers.

## Owner decisions required (before any tag push)

1. **Scope of v0.1.0:** CLI binaries only (skillhub + deskagent, target-qualified, checksummed + SBOM'd), or the full claimed inventory (container→GHCR, web archives, slopgate-action, installers)? The council's smallest-viable slice = the two CLIs.
2. **Signed or unsigned** release; if signed, supply a `RELEASE_GPG_KEY` secret.
3. **npm**: include `slopgate` npm (requires `NPM_TOKEN` + namespace ownership + an `npm pack --dry-run` review) or cut from v0.1.0.
4. **License**: which license text + metadata to apply (DEC-0002).
5. Whether to fix the in-repo blockers and re-run a **clean CI dry run** before publishing (recommended), versus narrowing scope.

## Confidence

**High.** The core blockers are deterministic control-flow mismatches in checked-in files (no SBOM generation before an unconditional gate; no dependency install; artifact-name mismatch; no license). Fresh command execution would refine secondary risks but cannot make the workflow succeed as written. (Note: advisors lacked an exec tool; `plan-lock verify`/`run-all-checks` were assessed statically and from records.)

## What would change the decision

A clean GitHub-hosted **dry run** of the exact release commit that passes all 4 matrix targets after dependency + system-package install, producing target-qualified assets matching `install.sh`, a published/exported image, a validated SBOM + checksums, and a decision on scope/signing/npm/license. Until then, do not push `v0.1.0`.
