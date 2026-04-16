#!/usr/bin/env bash
set -euo pipefail

if [[ $# -eq 0 ]]; then
  echo "usage: $0 <command> [args...]" >&2
  exit 64
fi

if ! command -v go >/dev/null 2>&1; then
  echo "error: go is not on PATH" >&2
  exit 127
fi

GOROOT="$(env -u GOROOT go env GOROOT)"
export GOROOT
unset GOTOOLDIR

exec "$@"
