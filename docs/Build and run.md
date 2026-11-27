awiki pins Rust 1.100.0 for development, CI, and release builds, builds a local executable through scripts/build.sh, and validates its own documentation with the resulting CLI.

## Toolchain

The [repository toolchain](../rust-toolchain.toml) selects Rust 1.100.0 with
rustfmt and clippy. Rustup installs and selects it automatically inside this
checkout; changing the global default is unnecessary. `Cargo.toml` requires
Rust 1.100 or newer for source builds.

```sh
rustup show active-toolchain
rustc --version
```

## Build

```sh
./scripts/build.sh
```

The [build script](../scripts/build.sh) uses locked dependencies, copies the
release binary to `bin/awiki`, and points `~/bin/awiki` at it. Keep `~/bin`
ahead of Homebrew on `PATH` to use that local build:

```sh
export PATH="$HOME/bin:$PATH"
```

Set `AWIKI_SKIP_LINK=1` to skip the symlink or `AWIKI_BUILD_OUTPUT` to select the
copied binary's destination. `AWIKI_VERSION` overrides the compiled version;
otherwise [`version()`](../src/cli.rs#version) uses the Cargo package version.

## Run and check

After building, run from the repository root:

```sh
awiki lint docs/
```

To use Cargo without installing or linking a binary:

```sh
cargo run --locked -- lint docs/
```

The docs directory shares the root [lint policy](../awiki.toml) through its
`awiki.toml` symlink. Run `awiki lint -recursive` as an additional check of
root documents and their cross-directory links. [Testing](Testing.md) covers
automated checks; [Installation](Installation.md) covers user installation.

For normal wiki usage, follow the [Overview](Overview.md) and the command examples
in [Command-line interface](<Command-line interface.md>).
