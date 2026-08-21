#!/usr/bin/env bash
# install.sh — install a SkillHub CLI binary from the latest GitHub release.
#   curl -sSL https://raw.githubusercontent.com/<owner>/agent-ecosystem/main/install.sh | bash
#
# Downloads the prebuilt binary for the current OS/arch into $PREFIX/bin (default ~/.local/bin).
# Homebrew/.deb/.rpm installers are produced from the same release artifacts by cargo-dist.
set -uo pipefail

CLI="${1:-skillhub}"            # skillhub | deskagent
VERSION="${VERSION:-latest}"    # latest | v0.1.0
PREFIX="${PREFIX:-$HOME/.local}"
REPO="${REPO:-agent-ecosystem}"
GH="${GH:-https://github.com/$REPO/releases}"
API="${API:-https://api.github.com/repos/$REPO/releases}"
BIN_DIR="${PREFIX}/bin"

[ "$CLI" = skillhub ] || [ "$CLI" = deskagent ] || { echo "unknown CLI '$CLI' (use skillhub or deskagent)" >&2; exit 2; }

# map platform -> release artifact suffix (uploaded by .github/workflows/release.yml)
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)  TARGET=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) TARGET=aarch64-unknown-linux-gnu ;;
  Darwin-x86_64) TARGET=x86_64-apple-darwin ;;
  Darwin-arm64)  TARGET=aarch64-apple-darwin ;;
  *) echo "unsupported platform: $(uname -s)-$(uname -m)" >&2; exit 2 ;;
esac

if [ "$VERSION" = latest ]; then
  TAG="$(curl -fsSL "$API/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)"
else
  TAG="$VERSION"
fi
[ -n "$TAG" ] || { echo "could not resolve release tag" >&2; exit 1; }

URL="$GH/download/$TAG/${CLI}-${TARGET}"
echo "fetching $CLI $TAG ($TARGET) -> $BIN_DIR/$CLI"
mkdir -p "$BIN_DIR"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
curl -fL "$URL" -o "$tmp" || { echo "download failed: $URL" >&2; exit 1; }
install -m 0755 "$tmp" "$BIN_DIR/$CLI"
echo "installed $BIN_DIR/$CLI (v$TAG)"
echo "ensure $BIN_DIR is on your PATH, e.g.: export PATH=\"$BIN_DIR:\$PATH\""
