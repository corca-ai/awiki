# Overview

`awiki` is a CLI for navigating and maintaining flat-file Markdown wikis.

The target workflow is a folderless note garden:

- every note is a Markdown file in the same directory
- notes may use Obsidian-style front matter
- structure comes from links, not folders or tags

## Quick Start

```sh
awiki lint
awiki path "문서 A" "문서 B"
awiki links "문서 A"
awiki rename "예전 이름" "새 이름"
awiki wanted
```

## Install

Recommended:

```sh
curl -sSfL https://raw.githubusercontent.com/corca-ai/awiki/main/install.sh | sh
```

Other options:

- `go install github.com/corca-ai/awiki/cmd/awiki@latest`
- `brew install corca-ai/tap/awiki`
- `./scripts/build.sh` for a local dev build plus `~/bin/awiki` symlink

Verify:

```sh
awiki version
```

See [Install](install.md) for details.

## Commands

- `lint` — fail on orphan documents or disconnected islands
- `path` — print the shortest path between two documents
- `rename` — rename a document and rewrite links to it
- `links` — show inbound and outbound links for a document
- `wanted` — rank the most-linked missing pages with source context
- `version` — print the build version

## Related Docs

- [Install](install.md) — installation methods
- [CLI Guide](cli.md) — command behavior and examples
- [Wiki Model](wiki-model.md) — flat-file assumptions and parsing rules
- [Build & Run](build.md) — local build and run commands
