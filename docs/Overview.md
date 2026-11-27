awiki explores and maintains flat-file Markdown wikis through graph navigation, formatting, and configurable checks for document structure and document/code references.

## Quick start

Follow [Installation](Installation.md) to install awiki, then run these commands
from a wiki directory:

```sh
awiki lint
awiki suggest
awiki wanted
awiki links "Graph theory"
awiki path "Probability" "Bayes theorem"
```

To check this repository's own documentation, run `awiki lint docs/` from the
repository root. Contributors can follow [Build and run](<Build and run.md>)
to build the current CLI.

## Wiki conventions

The default layout keeps Markdown pages together in one directory. Filenames
identify pages, and links provide structure. Front matter may supply titles
and aliases for direct lookups. [Wiki model](<Wiki model.md>) describes these
identities and graph rules.

Nested wikis are supported with `-recursive` (`-r`). Their documents use paths
relative to the wiki root; [Link resolution](<Link resolution.md>) explains
how bare wikilinks and relative Markdown links select documents.

## Choosing a command

Use `lint` for pass/fail checks, `format` for consistent Markdown, and `suggest`
for cleanup candidates that need judgment. Use `wanted` to rank missing pages,
`links` for a page's neighborhood, `graph` for a whole-wiki audit, `path` for a
route between pages, and `rename` to change a filename while updating links.
The [Command-line interface](<Command-line interface.md>) covers arguments and
output for each command.

## Project policy

An `awiki.toml` file in the wiki root selects lint severities and per-file
exceptions. [Lint configuration](<Lint configuration.md>) explains the rules;
[Code references](<Code references.md>) describes links to source declarations.
