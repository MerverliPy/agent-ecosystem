# The Synergy Build — agent-ecosystem

A monorepo building four interoperable products around the agent/AI ecosystem, driven by a **content-locked build plan**.

| App | Dir | What it is |
|-----|-----|------------|
| BenchKit | `apps/bench-site` | Local-inference benchmark dataset, "will-it-run" calculator, searchable matrix site |
| SkillHub | `apps/skillhub-cli`, `apps/skillhub-registry`, `apps/skillhub-web` | Cross-harness skill package manager + registry + security scanner |
| SlopGate | `apps/slopgate`, `apps/slopgate-action`, `apps/slopgate-dash` | Anti-slop lint rules, 0–100 score, CI PR gate |
| DeskAgent | `apps/deskagent` | Consumer local-first agent desktop app (Tauri 2 + React) |

## Plan lock

- `PHASES.md` — the build plan. **Content-locked.** Agents may tick checkboxes/status only.
- `PLAN.lock` — lock manifest: content hash + approval token hash + approval history.
- `PROGRESS.md` — agent-writable execution status and change requests.
- `scripts/plan-lock.sh` — `verify | status | propose | init | approve | check-staged | check-push`.

**Commands:**

```bash
bash scripts/plan-lock.sh verify     # integrity check — must pass before/after every phase
bash scripts/plan-lock.sh status     # lock info + open change requests
bash scripts/plan-lock.sh propose "why"  # agents' only channel to request a plan change
bash scripts/plan-lock.sh approve "why"  # HUMAN ONLY: re-lock after an approved content change
```

**Approval ceremony for plan changes (humans only):**

1. Agent runs `propose "<reason>"` → request lands in `PROGRESS.md`.
2. Human edits `PHASES.md` (or tells the agent exactly what to write and approves it).
3. Human runs `scripts/plan-lock.sh approve "<reason>"` from an interactive terminal, types `APPROVE`.
4. Lock re-hashes; `verify` passes again.

Git hooks (pre-commit/pre-push) block any commit that changes `PHASES.md` content or `PLAN.lock` unless
`PLAN_APPROVAL_TOKEN` (the human-held token) is in the environment. Install with
`bash hooks/install-hooks.sh`.

## Layout

```
apps/            one directory per product (DEC-0001)
shared/
  schemas/       JSON schemas (benchmark-result, skill-manifest)
  specs/         markdown specs (skill-manifest-spec-v1)
  datasets/      seeded benchmark data
scripts/         plan-lock.sh, verify-env.sh, run-all-checks.sh
records/         per-phase handoff records
```

See `PHASES.md` for the full plan and locked constraints.
