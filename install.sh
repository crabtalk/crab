#!/bin/sh
# Installs `crab` from the latest release of github.com/crabtalk/crab.
#
#   curl -fsSL https://crab.talk/install.sh | sh
#
# CRAB_INSTALL_DIR sets where `crab` goes; it defaults to ~/.local/bin.

set -eu

REPO="crabtalk/crab"
INSTALL_DIR="${CRAB_INSTALL_DIR:-$HOME/.local/bin}"

err() {
  echo "crab: $*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || err "this script needs \`$1\`"
}

need curl
need tar
need uname
need mktemp

os=$(uname -s)
arch=$(uname -m)
case "$arch" in
  x86_64 | amd64) arch=x86_64 ;;
  arm64 | aarch64) arch=aarch64 ;;
  *) err "no release for the $arch architecture" ;;
esac
case "$os" in
  Darwin) target="$arch-apple-darwin" ;;
  Linux) target="$arch-unknown-linux-gnu" ;;
  *) err "no release for $os; on Windows, download crab-x86_64-pc-windows-msvc.zip from https://github.com/$REPO/releases/latest" ;;
esac

url="https://github.com/$REPO/releases/latest/download/crab-$target.tar.gz"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "downloading $url" >&2
curl -fL --progress-bar "$url" -o "$tmp/crab.tar.gz" || err "could not download $url"
tar -xzf "$tmp/crab.tar.gz" -C "$tmp"
mkdir -p "$INSTALL_DIR"
mv "$tmp/crab" "$INSTALL_DIR/crab"
chmod +x "$INSTALL_DIR/crab"

echo "installed $("$INSTALL_DIR/crab" --version 2>/dev/null || echo crab) to $INSTALL_DIR/crab" >&2
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "add $INSTALL_DIR to your PATH to run crab" >&2 ;;
esac
