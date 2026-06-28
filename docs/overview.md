# Overview

`awiki` is a CLI for navigating and maintaining flat-file Markdown wikis.

It is built for regular wiki gardening: finding disconnected notes, following
graph paths, ranking missing pages, and surfacing refactoring candidates before
the wiki becomes hard to navigate.

The default workflow is a folderless note garden:

- every note is a Markdown file in the same directory
- notes may use Obsidian-style front matter
- structure comes from links, not folders or tags

Nested directory layouts are also supported with `-recursive`, which identifies
documents by repo-relative path.

## Quick Start

From a wiki directory:

```sh
awiki lint
awiki suggest
awiki path "문서 A" "문서 B"
awiki links "문서 A"
awiki rename "예전 이름" "새 이름"
awiki wanted
```

From elsewhere:

```sh
awiki suggest -root ~/wiki
awiki lint -root ~/wiki -recursive
```

`lint` is for rules that should fail automation. `suggest` is for refactoring
hints that deserve human judgment.

## Install

Recommended:

```sh
brew install corca-ai/tap/awiki
```

Other options:

- `curl -sSfL https://raw.githubusercontent.com/corca-ai/awiki/main/install.sh | sh`
- `cargo install --git https://github.com/corca-ai/awiki`
- `./scripts/build.sh` for a local dev build plus `~/bin/awiki` symlink

Verify:

```sh
awiki version
```

See [Install](install.md) for details.

## Commands

- `lint` — fail on orphan documents or disconnected islands
- `suggest` — show non-failing refactoring candidates and graph-quality hints
- `path` — print the shortest path between two documents
- `rename` — rename a document and rewrite links to it
- `links` — show inbound and outbound links for a document
- `wanted` — rank the most-linked missing pages with source context
- `version` — print the build version

## Choosing a Command

- Use `lint` in hooks or CI when you want a pass/fail answer.
- Use `suggest` during cleanup sessions when you want a prioritized reading
  list of pages to inspect.
- Use `wanted` when many notes point at pages that do not exist yet.
- Use `links` when a single page feels misplaced or under-connected.
- Use `path` when two ideas should be related but the current route is unclear.
- Use `rename` when a page name changes and existing links need to keep working.

## Output Model

Most document output uses:

```text
[[Page Name]]: First visible line
```

This makes output readable in a terminal and easy to filter with tools such as
`grep`, `sed`, and coding agents. Metadata lines generally begin with `//`.

## Related Docs

- [Install](install.md) — installation methods
- [CLI Guide](cli.md) — command behavior and examples
- [Wiki Model](wiki-model.md) — flat-file assumptions and parsing rules
- [Build & Run](build.md) — local build and run commands
