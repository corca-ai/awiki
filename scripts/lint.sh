#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ $# -eq 0 ]]; then
  exec ./scripts/go-env.sh golangci-lint run ./...
fi

exec ./scripts/go-env.sh golangci-lint run "$@"
