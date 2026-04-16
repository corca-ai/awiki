# Install

## Binary

```sh
curl -sSfL https://raw.githubusercontent.com/corca-ai/awiki/main/install.sh | sh
```

The install script writes to `/usr/local/bin` when writable, otherwise to
`~/.local/bin`.

You can also download a prebuilt archive from
[Releases](https://github.com/corca-ai/awiki/releases/latest).

## go install

```sh
go install github.com/corca-ai/awiki/cmd/awiki@latest
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
