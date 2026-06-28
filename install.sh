#!/bin/sh
set -eu

REPO="https://github.com/corca-ai/awiki"
BINARY="awiki"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required to install $BINARY from source" >&2
  exit 127
fi

INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
if [ ! -w "$INSTALL_DIR" ] 2>/dev/null; then
  INSTALL_DIR="$HOME/.local/bin"
  mkdir -p "$INSTALL_DIR"
fi

VERSION="${VERSION:-$(curl -sSf "https://api.github.com/repos/corca-ai/awiki/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)}"
if [ -z "$VERSION" ]; then
  echo "Failed to determine latest version" >&2
  exit 1
fi

echo "Installing $BINARY $VERSION to $INSTALL_DIR"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

cargo install --git "$REPO" --tag "$VERSION" --root "$TMP" --locked
install "$TMP/bin/$BINARY" "$INSTALL_DIR/$BINARY"
echo "Installed $INSTALL_DIR/$BINARY"
