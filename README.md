# awiki

`awiki` is a CLI for exploring and maintaining flat-file Markdown wikis.

It helps answer practical wiki-gardening questions:

- Which pages are isolated from the rest of the wiki?
- Which missing pages are linked often enough that they should probably exist?
- How do two pages connect through the link graph?
- Which pages look like refactoring candidates because they are too long, too
  thin, too distant, or near-duplicates?

`awiki` is optimized for a folderless wiki: every note lives in the same
directory, uses Markdown plus Obsidian-style front matter, and is structured
through links instead of folders or tags. Nested layouts are supported too via
`-recursive`, where documents are identified by repo-relative path.

## Install

```sh
brew install corca-ai/tap/awiki
```

Other install paths are documented in [Install](docs/install.md).

## Quick Start

Run from a wiki directory:

```sh
awiki lint
awiki format
awiki suggest
awiki wanted
awiki links "Graph theory"
awiki path "Probability" "Bayes theorem"
```

Or point at a wiki explicitly:

```sh
awiki suggest -root ~/wiki
awiki lint -root ~/wiki -recursive
```

## Commands

- `lint` — fail on mechanical quality problems such as orphans, islands, and
  context-free link-only lines
- `format` — rewrite Markdown files with awiki's default style
- `suggest` — print non-failing refactoring hints such as sampled long graph
  paths, missing-page pressure, long pages, short stubs, and near-duplicates
- `wanted` — rank unresolved links by how much the wiki already points at them
- `links` — inspect inbound, outbound, and missing links for one document
- `path` — print the shortest path between two documents
- `rename` — rename a document and rewrite links that point to it

Most output is line-oriented and grep-friendly. Document lines usually look like
`[[Page Name]]: First visible line`, so the result is useful both for humans and
for coding agents.

## Documentation

- [Agent Guide](AGENTS.md) — project doc index and codebase guide
- [Overview](docs/overview.md) — user-oriented tour and command selection guide
- [CLI Reference](docs/cli.md) — command behavior, flags, output, and examples
- [Wiki Model](docs/wiki-model.md) — document identity, link parsing, and graph rules
- [Install](docs/install.md) — Homebrew, install script, Cargo, and source builds

## License

Released under the [MIT](LICENSE) license.
