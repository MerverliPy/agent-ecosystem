# SkillHub

A skill marketplace for AI coding agents: Rust CLI + registry API + Next.js site.

- **CLI** (`apps/skillhub-cli`, Rust/clap): `search`, `info`, `install` (writes
  `skillhub.lock.json` into the detected harness's skills dir), `update`, `remove`,
  `verify` (27-rule security scanner), `scan`, `harnesses`, `publish`. `verify
  --quality` also runs the SlopGate scanner for a 0–100 quality score.
- **Registry** (`apps/skillhub-registry`, axum + SQLite): immutable publish (409 on
  duplicate), version listing, download counting, search.
- **Web** (`apps/skillhub-web`, Next.js): search grid, verified / high-risk /
  quality badges, per-harness install commands, version tables.

## Ecosystem

- **Consumes BenchKit**'s credibility model: verified badges derive from scan integrity, not sponsors (DEC-0006).
- **Consumed by DeskAgent**: its in-app skill installer speaks the registry API and
  lockfile format.
- **Consumes SlopGate**: `verify --quality` runs the slop scanner; the web snapshot
  carries `quality_score` per package.

Run the end-to-end test: `bash apps/skillhub-cli/scripts/e2e.sh` (14 checks).
