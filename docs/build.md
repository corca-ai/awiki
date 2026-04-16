# Build & Run

## Prerequisites

A Go toolchain is required. The project uses [mise](https://mise.jdx.dev/) to
manage it (`mise.toml`).

`awiki` requires Go 1.25 or newer.

```sh
mise install
```

If `go` is not on `PATH`, resolve it via mise:

```sh
export PATH="$(mise where go)/bin:$PATH"
```

The local helper scripts normalize `GOROOT` to match the active `go` binary, so
an outdated exported `GOROOT` does not break builds.

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

For release builds, inject the version via `ldflags`:

```sh
./scripts/build.sh -trimpath -ldflags="-s -w -X main.version=v0.1.0"
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

## Cache

`awiki` stores parsed wiki state under the user cache directory rather than the
config directory.

Examples:

- macOS: `~/Library/Caches/awiki`
- Linux: `~/.cache/awiki`

Each wiki root gets its own hashed cache directory. On later runs, unchanged
Markdown files are reused from cache and only files with changed modification
times or sizes are reparsed. The cache also stores each document's first
visible content line so commands that print page previews do not need to reread
every file.

## Related Docs

- [Overview](overview.md) — quick start and install paths
- [Install](install.md) — installation methods
- [Testing](testing.md) — test, lint, and pre-commit workflow
- [Release](release.md) — CI and tagged release flow
