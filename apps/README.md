# apps/ — one directory per product (DEC-0001)

| Dir | Product | Phase |
|-----|---------|-------|
| `bench-site/` | BenchKit — benchmark dataset, calculator, matrix site | 2 |
| `skillhub-cli/` | SkillHub — Rust CLI (install/search/update/verify) | 3 |
| `skillhub-registry/` | SkillHub — registry API (Rust, axum + SQLite) | 3 |
| `skillhub-web/` | SkillHub — Next.js site | 3 |
| `slopgate/` | SlopGate — rule pack + `slop` CLI | 4 |
| `slopgate-action/` | SlopGate — GitHub Action | 4 |
| `slopgate-dash/` | SlopGate — trend dashboard | 4 |
| `deskagent/` | DeskAgent — Tauri 2 + React personal agent | 5–6 |

## Ecosystem

The four products build on each other (see the root README for the full story):

- **BenchKit** (`bench-site/`) — local-inference benchmark data + the `will-it-run`
  calculator (`shared/lib/will-it-run.mjs`). **Consumed by** DeskAgent's model picker.
- **SkillHub** (`skillhub-cli/`, `skillhub-registry/`, `skillhub-web/`) — the skill
  marketplace. **Consumed by** DeskAgent's in-app skill installer.
- **SlopGate** (`slopgate/`, `slopgate-action/`, `slopgate-dash/`) — AI-slop
  detection. **Consumed by** SkillHub's optional quality check.
- **DeskAgent** (`deskagent/`) — the personal agent that pulls it together: BenchKit
  data for model choice, SkillHub for skills, approval cards for every memory write.

Shared assets live in `../shared/` (specs, schemas, datasets, `will-it-run.mjs`).
