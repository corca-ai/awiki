# Architecture

## Overview

`awiki` is intentionally small Rust CLI code with a few focused dependencies:
Rayon for parallel document parsing, `rustc-hash` for fast in-memory indexes,
and Unicode normalization for stable document identity.

The codebase keeps process concerns at the edge and wiki behavior in focused
modules:

- `src/main.rs` — process entry point
- `src/cli.rs` — command parsing, command orchestration, and exit behavior
- `src/output.rs` — terminal-oriented formatting
- `src/text.rs` — line scanning, inline-code masking, and small text helpers
- `src/wiki/` — wiki model, loading, parsing, graph analysis, and rename logic

## Package Responsibilities

### `src/cli.rs`

The CLI layer owns user-visible command flow:

- parsing flags and positional arguments
- loading a wiki root
- translating results into CLI output and exit codes

### `src/wiki/model.rs`

The model module owns the plain structs shared by the wiki implementation:

- documents, links, front matter, and lint issues
- in-memory graph indexes
- reports returned by analysis and rename operations

### `src/wiki/load.rs`

The load module owns filesystem discovery and graph construction:

- parsing documents in parallel after deterministic file discovery
- resolving identifiers and links
- building index-based directed, inbound, and undirected graphs

### Parsing and Analysis Modules

- `src/wiki/frontmatter.rs` parses and rewrites YAML front matter fields used by
  the tool
- `src/wiki/format.rs` rewrites Markdown documents with awiki's default style
- `src/wiki/links.rs` parses wiki/Markdown links and detects link-only lines
- `src/wiki/path.rs` normalizes document names and repo-relative paths
- `src/wiki/analysis.rs` implements lint, shortest-path, and wanted-page logic
- `src/wiki/suggest.rs` implements non-failing refactoring hints, with page
  length and near-duplicate helpers under `src/wiki/suggest/`
- `src/wiki/rename.rs` rewrites document links and performs the filesystem rename

## Design Choices

- flat-file by default: only top-level `.md` files are documents, identified by basename
- opt-in `-recursive` walks subdirectories and identifies documents by repo-relative path; flat is the single-directory special case of the same rules
- identifier resolution is case-insensitive
- identifier resolution normalizes Unicode so macOS and Linux filenames resolve consistently
- graph connectivity resolves links by canonical document identity (basename when flat, repo-relative path when recursive, Obsidian-aligned); front matter `title` and `aliases` do not participate
- document parsing is parallelized after file discovery; graph construction uses
  document indexes instead of repeated string-key graph traversal
- broken links are allowed and preserved
- graph connectivity ignores unresolved links and self-links
- rename avoids rewriting code fences, images, and external links

## Tests

- unit tests in `src/wiki/tests.rs` cover parsing, graph logic, and report
  behavior

## Related Docs

- [Wiki Model](wiki-model.md) — runtime rules and semantics
- [CLI Guide](cli.md) — user-facing command behavior
