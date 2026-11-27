awiki is a CLI for exploring and maintaining flat-file Markdown wikis; this guide indexes contributor documentation and source entry points.

## Read first

Read the [Documentation guide](<docs/Documentation guide.md>) before editing docs.
The [Overview](docs/Overview.md) introduces the product. Use the
[Command-line interface](<docs/Command-line interface.md>) for commands,
[Lint configuration](<docs/Lint configuration.md>) for policy, and
[Code references](<docs/Code references.md>) for source links.

Use [Wiki model](<docs/Wiki model.md>) and
[Link resolution](<docs/Link resolution.md>) for identity and graph semantics.
Follow [Installation](docs/Installation.md),
[Build and run](<docs/Build and run.md>), [Testing](docs/Testing.md), and
[Release](docs/Release.md) for development and publishing.

## Source entry points

[`main()`](src/main.rs#main) starts the CLI dispatcher
[`run()`](src/cli.rs#run). [`Vault`](src/wiki/model.rs#Vault) holds the wiki model;
[Architecture](docs/Architecture.md) connects its parsing, analysis, lint,
formatting, rename, and output modules.

`CLAUDE.md` is a symlink to this guide. Always run `awiki lint docs/` after
changing documentation and fix every issue.
