awiki's checks validate Rust code, wiki graph behavior, document structure, and code references using the pinned toolchain, local hooks, and GitHub Actions.

## Local checks

```sh
./scripts/test.sh
./scripts/lint.sh
awiki lint docs/
```

The [test script](../scripts/test.sh) runs Cargo tests; the
[lint script](../scripts/lint.sh) runs clippy with warnings denied. Build the
current executable as described in [Build and run](<Build and run.md>) before
using it to check documentation.

The docs directory shares the root [policy](../awiki.toml), which enables all
six rules without exclusions. `awiki lint docs/` validates the flat docs wiki;
`awiki lint -recursive` also checks root documents. See
[Lint configuration](<Lint configuration.md>) for rule semantics and
[Documentation guide](<Documentation guide.md>) for writing rules.

## Regression tests

`cargo test --test cli` runs the real executable against temporary wikis.
Tests assert stdout, stderr, exit codes, file contents, and names, including
reference links, comments/code, Unicode filenames, dry runs, JSON diagnostics,
format idempotence, and graph preservation across rename. Filesystem tests also
inject commit failures to verify restoration and retained recovery copies.

[File changes](<File changes.md>) defines the editing guarantees, and
[Performance](Performance.md) describes the reproducible benchmark.

## Hooks

Enable repository hooks once after cloning:

```sh
git config core.hooksPath .githooks
```

The `.githooks/pre-commit` hook checks Rust formatting and tests,
builds `target/debug/awiki`, and runs both documentation checks with that binary.
The `.githooks/pre-push` hook runs
`./scripts/check-ci-local.sh --fast`. Use `AWIKI_SKIP_PRE_PUSH=1 git push`
only for deliberate bypasses.

## CI and release checks

The [CI workflow](../.github/workflows/ci.yml) runs formatting, the file-length
ratchet, strict clippy, rustdoc, release build/tests, and both documentation
checks. Separate jobs check the minimum supported Rust version, unused
dependencies, advisories, and licenses.

The [local CI script](../scripts/check-ci-local.sh) mirrors these checks. Its
fast mode stops after debug tests and documentation lint; its full mode adds
release checks and runs cargo-machete, cargo-deny, and cargo-dist when installed.

The main jobs and [Release](Release.md) builds use the pinned repository
toolchain. The minimum-version job reads `Cargo.toml` separately.
