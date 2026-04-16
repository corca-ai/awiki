#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${AWIKI_BUILD_OUTPUT:-$ROOT/bin/awiki}"
LINK_DIR="${AWIKI_LINK_DIR:-$HOME/bin}"
LINK_PATH="${AWIKI_LINK_PATH:-$LINK_DIR/awiki}"

cd "$ROOT"
mkdir -p "$(dirname "$OUTPUT")"

./scripts/go-env.sh go build "$@" -o "$OUTPUT" ./cmd/awiki

if [[ "${AWIKI_SKIP_LINK:-0}" == "1" ]]; then
  exit 0
fi

mkdir -p "$LINK_DIR"
ln -sfn "$OUTPUT" "$LINK_PATH"

if [[ ":$PATH:" != *":$LINK_DIR:"* ]]; then
  echo "warning: $LINK_DIR is not on PATH; awiki may still resolve to the Homebrew install" >&2
  echo "hint: export PATH=\"$LINK_DIR:\$PATH\"" >&2
  exit 0
fi

if command -v awiki >/dev/null 2>&1; then
  RESOLVED="$(command -v awiki)"
  if [[ "$RESOLVED" != "$LINK_PATH" ]]; then
    echo "warning: awiki resolves to $RESOLVED instead of $LINK_PATH" >&2
    echo "hint: move $LINK_DIR ahead of Homebrew in PATH" >&2
  fi
fi
