# Agent Guide

`awiki` is a CLI for exploring and maintaining flat-file Markdown wikis.

## Read First

- [Documentation guide](docs/metadoc.md) — rules for writing and maintaining docs
- [Overview](docs/overview.md) — what awiki is, install paths, and quick start
- [CLI Reference](docs/cli.md) — command semantics and examples
- [Wiki Model](docs/wiki-model.md) — flat-by-default and recursive layouts, identity resolution, and link parsing
- [Link Resolution](docs/link-resolution.md) — Obsidian-aligned link resolution rules
- [Build & Run](docs/build.md) — local toolchain and build commands
- [Testing](docs/testing.md) — test, lint, and hook workflow
- [Release](docs/release.md) — CI and tagged release publishing
- [Architecture](docs/architecture.md) — package layout and responsibility boundaries

## Project Layout

- [src/main.rs](src/main.rs) — CLI entry point, wiki loading, parsing, graph analysis, and rename logic

Note: `CLAUDE.md` is a symlink to `AGENTS.md`.
