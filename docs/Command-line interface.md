awiki commands inspect wiki graphs, format and rename documents, and validate document structure and references; each command runs against one wiki root and prints terminal-friendly output.

## Common options

The default root is the current directory. All wiki commands accept `-root <dir>`
and `-recursive` (`-r`); recursion is opt-in. Every command supports `-h`, `-help`,
and `--help`. Quote filenames containing spaces.

The command dispatcher is [`run()`](../src/cli.rs#run), while
[`Vault::load()`](../src/wiki/load.rs#Vault::load) loads the selected wiki.
[Link resolution](<Link resolution.md>) describes recursive path semantics.

## Lint

`awiki lint [flags] [directory] [-json] [-include glob]` validates graph connectivity and configured
rules. A directory argument and `-root` are alternatives; supplying both is an
error.

```sh
awiki lint docs/
awiki lint -root ~/wiki -recursive
awiki lint -root . -recursive -include 'docs/**/*.md' -include 'AGENTS.md'
```

Repeat `-include` to select the union of root-relative Markdown glob patterns; each pattern must match at least one document.
Use `-recursive` to discover documents in subdirectories.
Only selected documents are loaded into the wiki graph, so an excluded document is not a graph target.

Without configuration, orphans, disconnected islands, and lines containing only
one document link fail lint. Configured runs fail only on `error` diagnostics;
`warn` diagnostics are printed without failing. [Lint configuration](<Lint configuration.md>)
covers all rules and output. [`lint_cmd()`](../src/cli/lint.rs#lint_cmd) owns
command execution and exit behavior.

## Format

`awiki format [flags] [--dry-run]` rewrites Markdown in place with no style options:

```sh
awiki format -root ~/wiki
```

[`format_markdown_document()`](../src/wiki/format.rs#format_markdown_document)
normalizes final newlines, trailing whitespace, repeated blank lines, ATX
headings, unordered lists, link spacing, and front matter ordering and lists.
Fenced code contents stay unchanged. Formatting does not reflow prose, add
context, or change link targets to resolved document names.

Use `--dry-run` to preview files through [File changes](<File changes.md>) without writing.
Output starts with `// format documents=<n> changed=<n>` and lists changed names.

## Path

`awiki path [flags] <from> <to>` prints the shortest undirected route, one page
per line, without directions or a path length:

```sh
awiki path "Probability" "Bayes theorem"
```

[`Vault::shortest_path_indices()`](../src/wiki/analysis.rs#Vault::shortest_path_indices)
returns an error if the pages are disconnected.

## Rename

`awiki rename [flags] [--dry-run] <old> <new>` changes a document filename and links that
resolve to it. Recursive mode also supports moving pages between directories.

```sh
awiki rename "Old title" "New title"
```

[`Vault::rename()`](../src/wiki/rename.rs#Vault::rename) updates wikilinks,
Markdown links, and an exactly matching front matter title, preserving
fragments and excluding code, images, and external links. See the rename
semantics in [Wiki model](<Wiki model.md>).

Use `--dry-run` to review the [change plan](<File changes.md>) without writing.
Output includes `// rename old=<file> new=<file>` and
`// links_updated=<n> files_touched=<n> title_updated=<bool>`.

## Links

`awiki links [flags] <document>` prints the requested page, inbound links, and
outbound links. Missing outbound targets are marked `(missing)`.

```sh
awiki links "Graph theory"
```

[`Vault::outbound_summaries()`](../src/output.rs#Vault::outbound_summaries)
prepares the outgoing page summaries. Sections use `// this page`,
`// incoming links`, and `// outgoing links` headers.

## Graph

`awiki graph [flags] [-edges] [-json]` reports the entire wiki:

```sh
awiki graph | grep ' inbound=0 '
awiki graph -edges
awiki graph -json | jq '.documents | sort_by(-.inbound)'
```

Default lines use `[[Page]] inbound=<n> outbound=<n> missing=<n>`.
Counts describe distinct other documents or distinct missing targets; self-links
are excluded. `-edges` prints `[[Source]] -> [[Target]]`, adding `(missing)` for
unresolved targets. Empty text reports print `_ none`.

[`format_graph_json()`](../src/wiki/graph.rs#format_graph_json) returns an object
with `documents` and `edges` arrays. Documents have `name`, `path`, `inbound`,
`outbound`, and `missing` fields; edges have `source`, `target`, and `missing`.

## Wanted

`awiki wanted [flags] [-n <limit>] [-sources <limit>]` ranks missing pages by
unresolved link mentions and prints source context:

```sh
awiki wanted -n 20 -sources 0
```

Defaults are 10 pages and 10 source lines per page; `-sources 0` removes the
source limit. Reports use `[[Missing page]] (N links)` headers, source bullets,
and `_ ...` for truncated sources.
[`Vault::all_wanted_pages()`](../src/wiki/analysis.rs#Vault::all_wanted_pages)
aggregates the mentions.

## Suggest

`awiki suggest [flags]` prints cleanup candidates without failing because of
those candidates. Default filters are `sampled-diameter`, `wanted-pressure`,
`long-pages`, `short-stubs`, and `near-duplicates`.

```sh
awiki suggest --filter=sampled-diameter,wanted-pressure
awiki suggest --filter near-duplicates --duplicate-threshold 0.9
awiki suggest -n 20 --long-lines 160 --short-words 30
```

Use `--samples`, `--paths`, and `--seed` for graph sampling; `-n` or `--limit`
for result limits; `--long-lines`, `--long-words`, and `--short-words` for page
size thresholds; and `--duplicate-threshold` for similarity. Comma-separated
`--filter` values select sections. See
[`SuggestOptions`](../src/wiki/suggest.rs#SuggestOptions) for defaults and
[`suggest_cmd()`](../src/cli/suggest.rs#suggest_cmd) for argument validation.

## Average shortest path

`awiki avg-shortest-path [flags] [-samples <n>] [-examples <n>] [-seed <n>]`
estimates average distance inside the largest component and prints sampled
longer-than-average paths:

```sh
awiki avg-shortest-path -samples 1000 -examples 3 -seed 7
```

[`Vault::approx_avg_shortest_path()`](../src/wiki/analysis.rs#Vault::approx_avg_shortest_path)
samples pairs rather than computing the exact graph diameter.

## Version

`awiki version` prints the build version. `awiki --version` and `awiki -version`
are equivalent; [`version()`](../src/cli.rs#version) supplies the value.

## Output conventions

Most page lines use `[[Page name]]: First visible line`; empty bodies use
`(empty)`. Metadata generally starts with `// `. Interactive output colors page
names and metadata, while piped output stays plain. Use `grep '^\[\['` to
extract page lines or `grep -v '^//'` to drop metadata.

[`Vault::document_line()`](../src/output.rs#Vault::document_line) formats page
previews. Refer to [Lint configuration](<Lint configuration.md>) for configured
diagnostic output and to [Overview](Overview.md) for command selection.
