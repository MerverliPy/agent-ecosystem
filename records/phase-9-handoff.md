# Phase 9 Handoff — SkillHub registry security (public multi-tenant)

## Completion state

- Phase status: **COMPLETE** (2026-08-21)
- Tasks: **11/11** completed; 0 blocked/cancelled
- Checkpoint tag `phase-9-start` (deleted after this handoff)
- VALIDATE hook (declared in PHASES.md): `plan-lock.sh verify && (cd apps/skillhub-registry && cargo test) && (cd apps/skillhub-cli && cargo test) && run-all-checks.sh` — **all green**
- Exit criteria: unauthenticated publish rejected; unauthorized owner publish rejected; malicious fixtures fail validation; verified packages remain anonymously readable; all adversarial + existing tests green — **all met**

## FILES CHANGED (phase-9-start..HEAD, +1608 / −65)

- `apps/skillhub-registry/src/main.rs` (+~1300): canonical `owner/name` id model + `canonical_id()`; owners/capabilities tables; HMAC-SHA256 self-contained scoped revocable capability tokens; owner namespaces + per-owner publish scope; rate limiting (fixed-window per-IP/per-token/global-read); input hardening (embedded JSON-schema, semver, size caps, path-traversal guard, body limit); Ed25519 per-owner signing via registry CA + verify-on-publish + key rotation/owner revocation; bind policy + non-leaking structured errors + default-deny; DB size cap (max_page_count); quarantine opt-in; batched download writes. Tests 4 → 28.
- `apps/skillhub-registry/Cargo.toml`: +hmac, sha2, base64, rand, jsonschema, ed25519-dalek, tower, http-body-util (dev).
- `apps/skillhub-registry/README.md`: Configuration, Security, TLS termination, Abuse/DoS docs.
- `apps/skillhub-cli/src/main.rs`: +Register/Revoke/Rotate cmds, Publish --signing-key, cmd_register prints token+signing_key, sign_package + package_digest_input.
- `apps/skillhub-cli/src/registry.rs`: register_owner/rotate_key return full JSON; publish sends Bearer.
- `apps/skillhub-cli/Cargo.toml`: +ed25519-dalek, base64.
- `scripts/check-artifact-hygiene.sh`: **new** — guard that DBs/.env/keys stay out of git + artifacts.
- `scripts/run-all-checks.sh`: wired hygiene guard (19 → 21 checks).
- `scripts/demos/skillhub-install-demo.sh`: register owner, export token + signing key.
- `PHASES.md`, `PROGRESS.md`, `README.md`: plan status + logs + check count.

## VALIDATIONS ACTUALLY RUN (all exit 0 unless noted)

| Command | Result |
|---|---|
| `bash scripts/plan-lock.sh verify` (pre/post each task) | PASS |
| `cargo test` (skillhub-registry) | 28/28 (was 4 at phase start) |
| `cargo test` (skillhub-cli) | 9/9 |
| `cargo build` (skillhub-registry) | clean, 0 warnings |
| `cargo build` (skillhub-cli) | clean (3 pre-existing warnings in scan.rs) |
| `bash scripts/check-artifact-hygiene.sh` | ARTIFACT-HYGIENE-OK |
| `bash scripts/run-all-checks.sh` | passed 21 / failed 0, RUN-ALL-CHECKS-OK |
| `bash scripts/demos/skillhub-install-demo.sh` | exit 0, signature-verified publish → install → verify --quality |

## UNRESOLVED GATES

- None blocking. Two notes for the human:
  1. `AGENTS.md` still states "20 checks"; the hygiene guard raised run-all-checks to **21**. AGENTS.md is above the free-form channel (agents must not edit it), so this needs a human-approved wording update.
  2. Historical "20/20" references in `records/phase-8-*.md` and earlier PROGRESS.md entries are left as history (not rewritten).

## EXACT NEXT ACTION

Phase 10, Task 1: **Establish unified versioning** — single semver source per product, `CHANGELOG.md`, git tags mapping to releases. Phase 10 depends on Phase 8 (COMPLETE) and Phase 9 (COMPLETE). Its VALIDATE hook is `plan-lock.sh verify && run-all-checks.sh && plan-lock.sh status`.

## MILESTONE ACCEPTANCE CLAIMED: NO

(Phase 10 remains; Milestone 2 is not complete until Phases 8–10 are all done.)
