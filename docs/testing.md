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

- `./scripts/test.sh`
- `./scripts/lint.sh`
- `AWIKI_BUILD_OUTPUT=.tmp-bin/awiki AWIKI_SKIP_LINK=1 ./scripts/build.sh`

## CI

GitHub Actions runs on every push to `main` and on pull requests.

The CI job executes:

1. `cargo fmt --check`
2. `cargo test --locked`
3. `cargo clippy --locked --all-targets --all-features -- -D warnings`
4. `cargo build --locked --release`

## Related Docs

- [Build & Run](build.md) — local build commands
- [Release](release.md) — tagged release flow
