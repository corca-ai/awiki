# Build & Run

## Prerequisites

Rust and Cargo are required.

```sh
rustup toolchain install stable
rustup default stable
```

## Build

```sh
./scripts/build.sh
```

The build script writes the binary to `bin/awiki` and refreshes
`~/bin/awiki` as a symlink to that local build, so a development build can
take precedence over a Homebrew install when `~/bin` appears earlier on
`PATH`.

If needed, add `~/bin` near the front of your shell profile:

```sh
export PATH="$HOME/bin:$PATH"
```

For tagged releases, `cargo-dist` reads the version from `Cargo.toml` and the
tag. Local development builds use:

```sh
AWIKI_VERSION=v0.1.0 ./scripts/build.sh
```

To build without updating the `~/bin/awiki` symlink:

```sh
AWIKI_SKIP_LINK=1 ./scripts/build.sh
```

## Run

Run from the project root while developing, or from a wiki root in normal use:

```sh
awiki lint
awiki path "Alpha" "Beta"
awiki links "Alpha"
awiki rename "Old" "New"
awiki version
```

Each command accepts `-root <dir>` to point at a wiki directory explicitly.

## Related Docs

- [Overview](overview.md) — quick start and install paths
- [Install](install.md) — installation methods
- [Testing](testing.md) — test, lint, and pre-commit workflow
- [Release](release.md) — CI and tagged release flow
