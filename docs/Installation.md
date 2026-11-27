awiki can be installed through Homebrew, a Cargo-based install script, or Cargo directly; local source builds use the repository's pinned Rust toolchain.

## Homebrew

```sh
brew install corca-ai/tap/awiki
awiki version
```

The formula installs the binaries published by the
[Release](Release.md) workflow.

## Install script

```sh
curl -sSfL https://raw.githubusercontent.com/corca-ai/awiki/main/install.sh | sh
```

The [install script](../install.sh) requires Cargo and builds the latest tagged
release with its locked dependencies. It writes to `/usr/local/bin` when
writable, otherwise `~/.local/bin`. `VERSION` selects a tag and `INSTALL_DIR`
selects the preferred destination.

## Cargo

```sh
cargo install --git https://github.com/corca-ai/awiki --locked
```

Building the current repository requires Rust 1.100 or newer. See
[Build and run](<Build and run.md>) for checkout builds and the optional
`~/bin/awiki` development symlink.

## First check

Run `awiki version` to verify installation, then follow the
[Overview](Overview.md) for wiki commands. Use `awiki lint docs/` from this
repository's root after changing documentation.
