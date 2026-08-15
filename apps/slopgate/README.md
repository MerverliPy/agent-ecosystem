# SlopGate

Deterministic AI-slop detector: a 40+ rule pack for code, comments, commit messages,
PR descriptions and docs, a 0–100 scorer, a CI entry point, a GitHub Action, and a
per-repo trend dashboard.

> Slop score is **higher = worse**. A clean repo scores ~0; a repo saturated with
> AI-generated habits scores 60–100. CI gates fail when the score exceeds a threshold.

## Quick start

```bash
# scan a tree
slop scan ./

# score 0-100 with per-rule breakdown
slop score ./ --json

# CI gate: exit 0 if score <= threshold, 1 if above
slop lint ./ --threshold 50

# lint a commit message too
slop lint ./ --threshold 50 --commit-msg "fix typo"

# apply the text rule packs to any blob (commit msg, PR body, docs)
slop check-text --text "As an AI language model, I cannot help with this."

# LLM review layer (bring-your-own-key; disabled without a key — deterministic core always works)
SLOPGATE_LLM_KEY=sk-... slop llm-review --text "wip"
```

Run from this directory (`apps/slopgate`) via `node --experimental-strip-types src/cli.ts`
or the `slop` bin. Node ≥ 22.6 required (type stripping). Zero runtime dependencies.

## Rule pack

46 rules across seven categories (see `slop rules`):

| Prefix | Category | Examples |
|--------|----------|----------|
| `DEAD-*` | Dead abstractions | unreferenced interfaces, empty interfaces, abstract-without-abstract, empty subclasses, pass-through wrappers |
| `UNUSED-*` | Unused helpers | exported helpers never imported, local functions never called, unused imports/variables, duplicate imports |
| `COMM-*` | Cargo-cult comments | comments that restate code, boilerplate headers, bare TODO/FIXME, placeholders, vacuous comments, commented-out code |
| `NAME-*` | Generic naming | `utils.ts`, `data`/`tmp`/`foo`, type-suffixed names, cryptic single letters, duplicated words |
| `OVER-*` | Over-engineering | async-without-await, promise anti-pattern, empty catch, stateless singletons, parameterless factories, empty branches |
| `COMMIT-*` | Boilerplate text | "fix typo", "update README", "wip", "cleanup", empty messages, missing PR descriptions |
| `AI-*` | AI phrasing | "As an AI language model", refusal boilerplate, "certainly! here's", "let me know if you have any questions" |

Every rule has fixture tests (`test/rules.test.ts`). Rules are regex/brace-based by
design: deterministic, dependency-free, and documented as heuristics.

## Scoring

```
score = min(100, Σ severity weights + density bonus)
```

| Severity | Weight |
|----------|--------|
| high     | 10     |
| medium   | 5      |
| low      | 2      |

Files with > 5 findings add a capped density bonus so one giant sloppy file cannot
hide in a large repo.

## Fixtures

`fixtures/` holds three seeded repos used to assert ordering and gating:

| Fixture | Score | What it demonstrates |
|---------|-------|----------------------|
| `clean` | ~0    | well-named, fully-used, honest-comment code |
| `mild`  | ~29   | a few unused helpers, a generic file name, a bare TODO |
| `heavy` | ~100  | every category of slop, including AI phrasing in the README |

Tests assert `clean < mild < heavy` and that `slop lint --threshold 50` fails on
`heavy` while passing on `clean`.

## LLM review layer

`slop llm-review` runs the deterministic pattern catalog **always**, then optionally
asks an LLM (OpenAI-compatible endpoint) to score prose 0–100 and name the slop type.
The LLM pass is **disabled by default** — no key, no call, no telemetry (DEC-0005).

```bash
SLOPGATE_LLM_KEY=sk-... slop llm-review --file docs/architecture.md
```

Env: `SLOPGATE_LLM_KEY` (or `OPENAI_API_KEY`), `SLOPGATE_LLM_URL`,
`SLOPGATE_LLM_MODEL`.

## GitHub Action

See `apps/slopgate-action/` — posts a PR comment, writes `slopgate.sarif`,
writes the step summary, and fails CI above `threshold` when `block: true`.

## Dashboard

See `apps/slopgate-dash/` — per-repo score history and a trend line rendered from
recorded check artifacts (`slopgate-dash/data/history.json`).

## Tests & typecheck

```bash
npm test            # 80 tests: rules, scanner, scoring, CLI, LLM layer
npx tsc --noEmit    # strict typecheck
```
