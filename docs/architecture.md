# Architecture

## Overview

`awiki` is intentionally small and uses the Go standard library plus a minimal
Unicode normalization dependency.

The codebase is split into:

- `cmd/awiki` — CLI parsing, help text, command dispatch, and terminal output
- `internal/awiki/wiki` — file loading, front matter parsing, link parsing, graph analysis, and rename rewriting

## Package Responsibilities

### `cmd/awiki`

`main.go` is the command boundary.

It is responsible for:

- parsing flags and positional arguments
- loading a wiki root
- translating library results into CLI output and exit codes

### `internal/awiki/wiki`

This package owns the wiki model and behavior.

- `vault.go` loads documents, resolves identifiers, builds the graph, and implements `lint`, `path`, and `links` queries
- `frontmatter.go` parses supported front matter fields and updates `title` during rename
- `links.go` parses supported link syntax in front matter and body text, and rewrites matching links during rename
- `rename.go` coordinates file renaming and atomic file rewrites

## Design Choices

- flat-file by default: only top-level `.md` files are documents, identified by basename
- opt-in `-recursive` walks subdirectories and identifies documents by repo-relative path; flat is the single-directory special case of the same rules
- identifier resolution is case-insensitive
- identifier resolution normalizes Unicode so macOS and Linux filenames resolve consistently
- graph connectivity resolves links by canonical document identity (basename when flat, repo-relative path when recursive, Obsidian-aligned); front matter `title` and `aliases` do not participate
- broken links are allowed and preserved
- graph connectivity ignores unresolved links and self-links
- rename avoids rewriting code fences, images, and external links

## Tests

- `cmd/awiki/main_test.go` covers command-facing behavior
- `internal/awiki/wiki/vault_test.go` covers parsing, graph logic, and rename behavior

## Related Docs

- [Wiki Model](wiki-model.md) — runtime rules and semantics
- [CLI Guide](cli.md) — user-facing command behavior
