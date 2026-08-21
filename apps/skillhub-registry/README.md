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

## Configuration

| Env var | Default | Purpose |
|---------|---------|---------|
| `SKILLHUB_REGISTRY_PORT` | `8787` | TCP port |
| `SKILLHUB_REGISTRY_BIND` | `127.0.0.1` | Bind address (loopback-only by default) |
| `SKILLHUB_REGISTRY_DB` | `data/skillhub.db` | SQLite database path |
| `SKILLHUB_REGISTRY_SECRET` | random (dev) | HMAC signing secret for capability tokens; must be stable in production |

## Security

- **Authn/Authz:** publish requires a scoped, revocable `publish:<owner>` capability token.
  Reads (search, detail, files) are anonymous. Owners are namespaced — a token for `owner A`
  can only publish under `A/*`. See the CLI: `skillhub register <owner>`.
- **Publish integrity:** packages must be signed with the owner's Ed25519 signing key issued by
  the registry CA on registration. Unsigned or mismatched signatures are rejected (`403`).
  Roll over a key with `skillhub rotate`; revoke an owner with `POST /api/owners/revoke-owner`.
- **Input hardening:** manifests are validated against the embedded JSON-schema; file paths are
  checked against traversal/absolute-path patterns; per-file/total/body size caps are enforced.
- **Rate limiting:** per-IP and per-token publish buckets plus a global read bucket (fixed-window).
- **Abuse/DoS:** the DB is capped at ~1 GiB (`max_page_count`); unverified / `high_risk` packages are
  quarantined — hidden from anonymous search/detail/files unless the client opts in with
  `?quarantine=true`; download counts are batched in memory and flushed to the DB periodically
  instead of a write per request.
- **Secrets:** the signing secret and per-owner signing keys are never logged or embedded in
  build artifacts. Set `SKILLHUB_REGISTRY_SECRET` to a stable, secret value in production.

### TLS termination

Plain HTTP is for local/dev use. For public deployment, terminate TLS at a reverse proxy
(such as Caddy, nginx, or a load balancer) in front of this service, and bind the registry
loopback-only (`SKILLHUB_REGISTRY_BIND=127.0.0.1`). Do **not** bind `0.0.0.0` and serve
plaintext auth/publish endpoints to the public internet. The registry never sends credentials
or keys over the wire without TLS.

Errors are structured (`{"error": "..."}`) and never leak internal details (SQL, paths, stack
traces) to clients; internals go to the server log only.

## Test

```bash
cd apps/skillhub-registry
cargo test
```
