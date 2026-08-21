# Release Summary — v0.1.0 & v0.1.1

**Milestone 2 accepted.** All plan phases (1–8, 8.5, 9, 10) COMPLETE; `plan-lock.sh verify` PASS;
`run-all-checks.sh` 21/21. The release pipeline is proven end-to-end in clean CI.

## Shipped releases

| Release | What it contains |
|---------|------------------|
| **v0.1.0** | 8 per-target CLI binaries (skillhub + deskagent × linux amd64/arm64, macOS amd64/arm64), GPG-signed `SHA256SUMS`/`.sig`, `SBOM.json`, `skillhub-cli-installer.sh`, Homebrew `skillhub-cli.rb`, tar.xz + checksum, `web.tar.gz`, `slopgate-action.tar.gz`, `slopgate-0.1.0.tgz`. GHCR `.../skillhub-registry:v0.1.0`. |
| **v0.1.1** | Same 8 binaries + **native `.deb` installers** (`skillhub_0.1.1_amd64.deb`, `deskagent_0.1.1_amd64.deb`) + signed provenance. GHCR `.../skillhub-registry:v0.1.1`. `@merverli/slopgate@0.1.1` published to npm. |

- Releases: https://github.com/MerverliPy/agent-ecosystem/releases
- npm: `@merverli/slopgate` (MIT, bin `slop`); unscoped `slopgate` is rejected by npm as too
  similar to the existing `slop-gate`, hence the scoped name.
- Container: `ghcr.io/merverli/agent-ecosystem/skillhub-registry` (`v0.1.0`, `v0.1.1`, `latest`).

## Verify / reproduce

```bash
# plan lock + full suite
bash scripts/plan-lock.sh verify
bash scripts/run-all-checks.sh            # 21/21

# version consistency (VERSION=0.1.1, all 11 manifests)
bash scripts/check-versions.sh

# release-only gate (checksums, SBOM, per-target artifacts, signature)
bash scripts/release-gate.sh dist/release

# verify signed checksums
gpg --import scripts/release-gpg-public.asc
gpg --verify SHA256SUMS.sig SHA256SUMS
sha256sum -c SHA256SUMS

# npm package
npm view @merverli/slopgate

# trigger a release (manual dispatch, or push tag v<version>)
gh workflow run release.yml --ref main
git tag -a v0.1.2 -m "..." && git push origin v0.1.2
```

## Deferred / known follow-ups

- **`.rpm` installers** — fix committed (`scripts/build-native-installers.sh`, `%{name}` macro +
  Source staging) but not yet validated; `.deb` already covers Linux. Run a `v0.1.2` tag to validate
  **only** if RHEL/Fedora packages are actually needed. The step is best-effort and never fails a release.
- **macOS installers via cargo-dist** — produced on macOS runners in a future tag run if wanted
  (cargo-dist currently builds linux-amd64 installers on the publish host).

## Repository / tooling state

- GPG signing key + Automation npm token are **GitHub repo secrets** (`RELEASE_GPG_KEY`,
  `GPG_KEY_ID`, `NPM_TOKEN`) — never committed. Public key committed at `scripts/release-gpg-public.asc`.
- `cargo-dist` configured in both Rust workspaces (`allow-dirty=["ci"]` so it builds without
  owning the root workflow).
- Root `release.yml` orchestrates checks → matrix build → publish (assemble, cargo-dist installers,
  native `.deb`/`.rpm`, GPG signing, SBOM, release-gate, GHCR push, npm publish, GitHub Release).

## Next action

Stop here. The release work is complete and reviewable. Re-enter only for: (a) `.rpm` validation
via `v0.1.2`, or (b) a future feature/bugfix release (bump `VERSION`, all manifests + lockfiles,
tag `v<version>`).
