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
- `cache.go` stores parsed document metadata and first-visible-line previews in the user cache directory and reuses them on subsequent loads
- `frontmatter.go` parses supported front matter fields and updates `title` during rename
- `links.go` parses supported link syntax in front matter and body text, and rewrites matching links during rename
- `rename.go` coordinates file renaming and atomic file rewrites

## Design Choices

- flat-file only: only top-level `.md` files are considered documents
- identifier resolution is case-insensitive
- identifier resolution normalizes Unicode so macOS and Linux filenames resolve consistently
- parsed vault state is cached under the user cache directory and invalidated by file `mtime` and size changes
- broken links are allowed and preserved
- graph connectivity ignores unresolved links and self-links
- rename avoids rewriting code fences, images, and external links

## Tests

- `cmd/awiki/main_test.go` covers command-facing behavior
- `internal/awiki/wiki/vault_test.go` covers parsing, graph logic, and rename behavior

## Related Docs

- [Wiki Model](wiki-model.md) — runtime rules and semantics
- [CLI Guide](cli.md) — user-facing command behavior
