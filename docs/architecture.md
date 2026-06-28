# Architecture

## Overview

`awiki` is intentionally small Rust CLI code with a few focused dependencies:
Rayon for parallel document parsing, `rustc-hash` for fast in-memory indexes,
and Unicode normalization for stable document identity.

The codebase is centered in:

- `src/main.rs` — CLI parsing, file loading, front matter parsing, link parsing,
  graph analysis, terminal output, and rename rewriting

## Package Responsibilities

### `src/main.rs`

The binary keeps side effects at the command boundary and uses plain structs for
the wiki model. It is responsible for:

- parsing flags and positional arguments
- loading a wiki root
- parsing documents in parallel after deterministic file discovery
- resolving identifiers and links
- building index-based directed, inbound, and undirected graphs
- translating results into CLI output and exit codes

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

- unit tests in `src/main.rs` cover parsing, graph logic, and report behavior

## Related Docs

- [Wiki Model](wiki-model.md) — runtime rules and semantics
- [CLI Guide](cli.md) — user-facing command behavior
