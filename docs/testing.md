# Testing

## Unit Tests

```sh
./scripts/test.sh
```

The test suite covers command behavior, parsing, graph analysis, and rename
rewrites.

## Lint

The project uses `cargo clippy` for linting.

```sh
./scripts/lint.sh
```

## Pre-commit Hook

Enable the repository hook once after cloning:

```sh
git config core.hooksPath .githooks
```

The pre-commit hook runs:

- `cargo fmt --all --check`
- `cargo test`
- `AWIKI_BUILD_OUTPUT=.tmp-bin/awiki AWIKI_SKIP_LINK=1 ./scripts/build.sh`

The pre-push hook runs:

- `./scripts/check-ci-local.sh --fast`

Use `AWIKI_SKIP_PRE_PUSH=1 git push` only for deliberate bypasses.

## CI

GitHub Actions runs on every push to `main` and on pull requests.

The CI job executes:

1. `cargo fmt --check`
2. `python3 scripts/check-file-lengths.py`
3. `cargo clippy --locked --all-targets --all-features -- -D warnings`
4. `cargo doc --locked --no-deps` with warnings denied
5. `cargo build --locked --release`
6. `cargo test --locked --release`
7. `cargo machete`
8. `cargo deny check`

## Related Docs

- [Build & Run](build.md) — local build commands
- [Release](release.md) — tagged release flow
