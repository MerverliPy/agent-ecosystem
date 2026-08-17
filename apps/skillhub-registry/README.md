# SkillHub Registry

The SkillHub skill registry API — an immutable publish service for AI coding-agent skills.

Rust, axum + SQLite. See the [SkillHub product overview](../README.md) for the full picture.

## What it does

- **Immutable publish** — a duplicate skill+version publish returns `409 Conflict`
- **Version listing** — all published versions of a skill
- **Download counting** — per-skill download totals
- **Search** — find skills by name/tags for the web UI and CLI

## Run

```bash
cd apps/skillhub-registry
cargo run
```

The registry listens on its configured port; the CLI (`apps/skillhub-cli`) and web
site (`apps/skillhub-web`) both speak this API. The end-to-end demo
(`bash scripts/demos/skillhub-install-demo.sh`) starts its own registry on an
ephemeral port and exercises publish → install → `verify --quality`.

## Ecosystem

- **Consumed by** SkillHub CLI and web (search, install, badges)
- **Consumed by** DeskAgent's in-app skill installer
- Feeds **SkillHub's** optional `verify --quality` check via the SlopGate scanner

## Test

```bash
cd apps/skillhub-registry
cargo test
```
