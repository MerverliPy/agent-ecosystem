#!/usr/bin/env bash
# build-native-installers.sh — build .deb and .rpm packages for the linux-amd64 CLI binaries
# staged in dist/release. cargo-dist produces shell/homebrew/tar.xz; this adds native packages.
# Requires: dpkg-deb (Debian) and rpmbuild (Fedora/RHEL tooling, `rpm` package on Ubuntu).
# Usage: bash scripts/build-native-installers.sh [dist/release-dir] [version]
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
DIR="${1:-dist/release}"
VER="${2:-$(tr -d '[:space:]' < VERSION)}"
ARCH="amd64"
[ -d "$DIR" ] || { echo "no such dir: $DIR" >&2; exit 1; }

# the linux-amd64 binaries (per-target names from the build matrix)
BINS="skillhub deskagent"
TARGET="x86_64-unknown-linux-gnu"

command -v dpkg-deb >/dev/null 2>&1 || { echo "dpkg-deb not found" >&2; exit 1; }

build_deb() { # name pkg
  local name="$1"
  local pkg="$2"
  local bin="$DIR/$name-$TARGET"
  [ -f "$bin" ] || { echo "skip .deb for $name (no $bin)" >&2; return; }
  local staged; staged="$(mktemp -d)"
  mkdir -p "$staged/usr/bin" "$staged/DEBIAN"
  install -m 0755 "$bin" "$staged/usr/bin/$name"
  cat > "$staged/DEBIAN/control" <<EOF
Package: $pkg
Version: $VER
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: agent-ecosystem <releases@agent-ecosystem.invalid>
Description: $name — an agent-ecosystem command-line tool
EOF
  dpkg-deb --build "$staged" "$DIR/${pkg}_${VER}_${ARCH}.deb" >/dev/null
  rm -rf "$staged"
  echo "  built $DIR/${pkg}_${VER}_${ARCH}.deb"
}

if command -v rpmbuild >/dev/null 2>&1; then
  build_rpm() { # name pkg
    local name="$1"
    local pkg="$2"
    local bin="$DIR/$name-$TARGET"
    [ -f "$bin" ] || { echo "skip .rpm for $name (no $bin)" >&2; return; }
    local d; d="$(mktemp -d)"
    mkdir -p "$d/BUILD" "$d/RPMS" "$d/SOURCES" "$d/SPECS" "$d/tmp"
    local staged="$d/staged"
    mkdir -p "$staged/usr/bin"
    install -m 0755 "$bin" "$staged/usr/bin/$name"
    cat > "$d/SPECS/$pkg.spec" <<EOF
Name: $pkg
Version: ${VER}
Release: 1
Summary: $name — an agent-ecosystem command-line tool
License: MIT
BuildArch: $ARCH

%description
$name — an agent-ecosystem command-line tool.

%install
mkdir -p %{buildroot}/usr/bin
install -m 0755 $staged/usr/bin/$name %{buildroot}/usr/bin/$name

%files
/usr/bin/$name

%post
echo "$name installed"
EOF
    rpmbuild --define "_topdir $d" --define "_buildroot $staged" -bb "$d/SPECS/$pkg.spec" >/dev/null 2>&1 \
      && cp "$d/RPMS/$ARCH/${pkg}-${VER}-1.${ARCH}.rpm" "$DIR/" && echo "  built $DIR/${pkg}-${VER}-1.${ARCH}.rpm" || echo "  warn rpmbuild failed for $name"
    rm -rf "$d"
  }
else
  echo "rpmbuild not found — skipping .rpm (install the 'rpm' package to enable)"
  build_rpm() { :; }
fi

echo "== building native installers (.deb/.rpm) for linux amd64 =="
build_deb skillhub skillhub
build_deb deskagent deskagent
build_rpm skillhub skillhub
build_rpm deskagent deskagent
echo "done"
