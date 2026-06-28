# Install

## Install Script

```sh
curl -sSfL https://raw.githubusercontent.com/corca-ai/awiki/main/install.sh | sh
```

The install script builds the tagged release with Cargo, then writes to
`/usr/local/bin` when writable, otherwise to `~/.local/bin`.

## cargo install

```sh
cargo install --git https://github.com/corca-ai/awiki
```

## Homebrew

```sh
brew install corca-ai/tap/awiki
```

## From Source

```sh
./scripts/build.sh
```

This creates `bin/awiki` and refreshes `~/bin/awiki` as a symlink to the
local build. To make that development build win over a Homebrew install, keep
`~/bin` ahead of Homebrew on `PATH`:

```sh
export PATH="$HOME/bin:$PATH"
```

## Verify

```sh
awiki version
```

## Related Docs

- [Overview](overview.md) — quick start
- [Build & Run](build.md) — local development builds
- [Release](release.md) — release artifacts and Homebrew publishing
