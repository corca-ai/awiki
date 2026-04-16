# Agent Guide

`awiki` is a CLI for exploring and maintaining flat-file Markdown wikis.

## Read First

- [Documentation guide](docs/metadoc.md) — rules for writing and maintaining docs
- [Overview](docs/overview.md) — what awiki is, install paths, and quick start
- [CLI Reference](docs/cli.md) — command semantics and examples
- [Wiki Model](docs/wiki-model.md) — flat-file assumptions, identity resolution, and link parsing
- [Build & Run](docs/build.md) — local toolchain and build commands
- [Testing](docs/testing.md) — test, lint, and hook workflow
- [Release](docs/release.md) — CI, goreleaser, and Homebrew publishing
- [Architecture](docs/architecture.md) — package layout and responsibility boundaries

## Project Layout

- [cmd/awiki/main.go](cmd/awiki/main.go) — CLI entry point and command dispatch
- [internal/awiki/wiki](internal/awiki/wiki) — wiki loading, parsing, graph analysis, and rename logic

Note: `CLAUDE.md` is a symlink to `AGENTS.md`.
