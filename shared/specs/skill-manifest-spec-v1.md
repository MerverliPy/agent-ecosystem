# Skill Manifest Spec — DRAFT v1

> Draft for Phase 1. Finalized (and machine-validated via `skill-manifest.schema.json`) in Phase 3.

A skill is a directory containing a `SKILL.md` procedure plus optional assets. This manifest makes skills
distributable, versionable, and installable across harnesses via SkillHub.

## Location

`skillhub.toml` at the skill repository root (TBD: `skillhub.json` if TOML proves awkward).

## Fields (draft)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | `owner/name`, lowercase, `[a-z0-9-]+` |
| `version` | string | yes | SemVer (`1.2.3`) |
| `description` | string | yes | One-line summary shown in search |
| `license` | string | yes | SPDX id; must be permissive (DEC-0002) |
| `repo` | string | yes | Source git repo URL (used by `publish`) |
| `harnesses` | array\<string> | yes | Supported harnesses: `claude-code`, `codex`, `cursor`, `gemini-cli`, `copilot`, `pi`, `openclaw` |
| `dependencies` | array\<{name, version}> | no | Skills/MCPs this skill requires |
| `permissions` | array\<string> | no | Declared permissions: `files.read`, `files.write`, `shell`, `network`, `secrets`, `browser` |
| `entrypoint` | string | no | Path to `SKILL.md` (default: `SKILL.md`) |

## Rules (draft)

1. `permissions` must be declared; skills requesting `shell` or `network` get a "high-risk" badge.
2. Version is immutable once published — re-publishing requires a new version.
3. The SkillHub security scanner must pass before a skill is listed (verified badge).
4. Install writes the skill into the detected harness's skills directory and records
   `skillhub.lock.json` (name, version, resolved dependencies, source, install date).

## Open questions (resolve in Phase 3)

- TOML vs JSON for the manifest file.
- Dependency version ranges (`^1.2.0`) — support or pin-exact only.
- MCP server deps: reference by `server-name@owner/name` convention.
