# slopgate-action

GitHub Action that runs the SlopGate scanner on a repo, posts a PR comment,
writes a SARIF artifact, and gates CI on a 0–100 slop score (higher = worse).

Zero runtime dependencies: it shells out to the `slop` CLI
(`apps/slopgate/src/cli.ts`, run via `node --experimental-strip-types`) and uses
Node built-ins + global fetch for the GitHub REST calls.

## Inputs

| Input | Default | Description |
|-------|---------|-------------|
| `path` | `.` | Path to scan (relative to repo root). |
| `threshold` | `50` | Max acceptable slop score. Job fails when exceeded and `block` is true. |
| `block` | `true` | Fail CI when the score exceeds the threshold. |
| `comment` | `true` | Post a report comment on pull requests. |
| `sarif` | `true` | Write `slopgate.sarif` to `GITHUB_WORKSPACE`. |
| `token` | `${{ github.token }}` | Token for PR comments. |

## Usage

```yaml
name: slopgate
on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read
  pull-requests: write   # for PR comments

jobs:
  slopgate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: ./apps/slopgate-action
        with:
          path: ./src
          threshold: 50
          block: true
      - name: Upload SARIF
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: slopgate-sarif
          path: slopgate.sarif
```

What the action does:

1. `slop scan <path> --json` + `slop score <path> --json` (via the action's own
   spawn path — the CLI is resolved relative to the action, so it works in any checkout).
2. Gates: `score > threshold && block` → exit 1, failing the job.
3. On `pull_request` events, posts a markdown report comment (score, breakdown,
   top rules). Comment failures are non-fatal.
4. Writes the job step summary (`GITHUB_STEP_SUMMARY`) and `slopgate.sarif`
   (SARIF 2.1.0) to the workspace. Upload the SARIF with `actions/upload-artifact`.

## Tests

```bash
npm test        # 9 tests: inputs, gate, comment/summary builders, real CLI integration
npm run build   # syntax check (this is the Phase 4 VALIDATE build hook)
```
