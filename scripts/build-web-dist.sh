#!/usr/bin/env bash
# build-web-dist.sh — produce static builds of the three web apps into dist/web/<app>.
# Each Next.js app is configured with `output: "export"` and deploys as static files to a
# named target (e.g. GitHub Pages / S3 / Netlify). Called by the release pipeline.
# Usage: bash scripts/build-web-dist.sh   (from the repo root)
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
OUT="$ROOT/dist/web"
APPS="bench-site skillhub-web slopgate-dash"

rm -rf "$OUT"
mkdir -p "$OUT"
for d in $APPS; do
  echo "== building $d (static) =="
  (cd "apps/$d" && npm run build) || { echo "build failed for $d" >&2; exit 1; }
  [ -d "apps/$d/out" ] || { echo "no static output dir for $d" >&2; exit 1; }
  mkdir -p "$OUT/$d"
  cp -r "apps/$d/out/." "$OUT/$d/"
  echo "   -> $OUT/$d"
done

# deploy-target manifest: named deploy targets per app
cat > "$OUT/DEPLOY.md" <<'MD'
# Web static deploy targets

Each directory is a self-contained static site (Next.js `output: export`). Deploy any of them
to your named target (GitHub Pages, an S3 bucket + CloudFront, Netlify, nginx root, ...):

| App | Static root | Example target |
|-----|-------------|----------------|
| bench-site | `dist/web/bench-site/` | GitHub Pages: `https://<user>.github.io/bench-site/` |
| skillhub-web | `dist/web/skillhub-web/` | nginx root `/var/www/skillhub` |
| slopgate-dash | `dist/web/slopgate-dash/` | S3 bucket `slopgate-dash` + CloudFront |

All assets are static; no build step runs on the deploy target.
MD
echo "web static dist written to $OUT"
