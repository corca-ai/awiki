#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ $# -eq 0 ]]; then
  exec ./scripts/go-env.sh go test ./...
fi

exec ./scripts/go-env.sh go test "$@"
