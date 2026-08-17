# SkillHub Web

The SkillHub skill registry website — search, verified / high-risk / quality badges, and per-harness install commands.

Next.js + React. See the [SkillHub product overview](../README.md) for the full picture.

## What it does

- **Search grid** — browse skills published to the registry
- **Badges** — verified, high-risk, and 0–100 quality badges per package
- **Install commands** — copyable per-harness install instructions
- **Version tables** — all published versions with download counts

## Run

```bash
cd apps/skillhub-web
npm install
npm run dev        # development server
npm run build      # production build
npm start          # serve the production build
```

## Ecosystem

- **Speaks to** `apps/skillhub-registry` (search, versions, downloads)
- Verified badges derive from **BenchKit**'s credibility model (scan integrity, not sponsors — DEC-0006)
- Quality badges come from **SlopGate**'s scanner via the CLI's `verify --quality`

## Test

```bash
cd apps/skillhub-web
npm test
```
