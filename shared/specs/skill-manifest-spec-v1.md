# Skill Manifest Spec — v1 (FINAL)

> Status: FINAL (Phase 3). Machine-validated by `shared/schemas/skill-manifest.schema.json`.
> Decision log: manifest format is **JSON** (`skillhub.json`) — validates directly against the JSON
> schema with zero extra tooling (the TOML option from the draft is dropped). Dependency version
> ranges support exact pins and `^`-caret ranges; the lockfile always records exact resolved versions.

A skill is a directory containing a `SKILL.md` procedure plus optional assets. This manifest makes
skills distributable, versionable, and installable across harnesses via SkillHub.

## File

`skillhub.json` at the skill repository root.

## Fields

| Field | Type | Required | Rules |
|-------|------|----------|-------|
| `name` | string | yes | `owner/name`, lowercase, `[a-z0-9-]+` |
| `version` | string | yes | SemVer `MAJOR.MINOR.PATCH` |
| `description` | string | yes | 1–200 chars, shown in search |
| `license` | string | yes | SPDX id, must be permissive (DEC-0002): MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, MIT-0, Zlib, Unlicense, CC0-1.0, MPL-2.0. GPL/AGPL/LGPL rejected. |
| `repo` | string | yes | Source git repo URL (used by `publish` and provenance) |
| `harnesses` | array\<string> | yes | `claude-code`, `codex`, `cursor`, `gemini-cli`, `copilot`, `pi`, `openclaw` |
| `dependencies` | array\<{name, version}> | no | `version` = exact pin or `^`-caret range |
| `permissions` | array\<string> | no | `files.read`, `files.write`, `shell`, `network`, `secrets`, `browser` |
| `entrypoint` | string | no | Path to `SKILL.md` (default `SKILL.md`) |

## Rules

1. `permissions` must be declared; skills requesting `shell` or `network` get a high-risk badge.
2. Version is immutable once published — re-publishing the same version is rejected (409).
3. The SkillHub security scanner must pass with zero high-severity findings to earn the `verified` badge.
4. `install` writes the skill into the detected harness's skills directory as `owner/name/` and records
   an entry in `skillhub.lock.json` (name, resolved version, source, checksum, harness, installed_at).
5. `update` re-fetches the latest version, diffs the lockfile, and replaces the skill directory.
6. `remove` deletes the skill directory and its lockfile entry.

## Resolved open questions (from draft)

- Manifest format: **JSON** (draft asked TOML vs JSON → JSON).
- Dependency ranges: exact pins + `^` caret; lockfile records exact resolution.
- MCP server deps: deferred to v2 (documented as future `mcp` field).
