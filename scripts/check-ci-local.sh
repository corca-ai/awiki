#!/usr/bin/env bash
# Local CI mirror. Use --fast for the pre-push path; default runs the full gate.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

mode="${1:---full}"

step() {
  echo
  echo "== $* =="
}

step "format"
cargo fmt --all --check

step "file-length ratchet"
python3 scripts/check-file-lengths.py --self-test
python3 scripts/check-file-lengths.py

step "clippy"
cargo clippy --locked --all-targets --all-features -- -D warnings

step "test"
cargo test --locked

if [[ "$mode" == "--fast" ]]; then
  exit 0
fi

step "doc"
RUSTDOCFLAGS=-Dwarnings cargo doc --locked --no-deps

step "release build"
cargo build --locked --release

step "release tests"
cargo test --locked --release

if command -v cargo-machete >/dev/null 2>&1; then
  step "cargo-machete"
  cargo machete
else
  echo
  echo "== cargo-machete =="
  echo "skipped — install with: cargo install cargo-machete"
fi

if command -v cargo-deny >/dev/null 2>&1; then
  step "cargo-deny"
  cargo deny check
else
  echo
  echo "== cargo-deny =="
  echo "skipped — install with: cargo install cargo-deny"
fi

if command -v dist >/dev/null 2>&1; then
  step "cargo-dist plan"
  dist plan
else
  echo
  echo "== cargo-dist plan =="
  echo "skipped — install with: cargo install cargo-dist"
fi
