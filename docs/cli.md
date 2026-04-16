# CLI Guide

`awiki` operates on one wiki root directory at a time. Use `-root <dir>` to
target a directory explicitly; otherwise the current directory is used.

## Output Style

Command output is line-oriented and grep-friendly.

- most document lines use `[[Page Name]]: First visible line`
- most meta lines start with `// `
- previews come from the document's first visible content line
- if a document has no printable body content, it is shown as `[[Page Name]]: (empty)`
- when stdout is an interactive terminal, `awiki` colors the `[[Page Name]]`
  part and `// ` comments; piped output stays plain text
- `grep '^\[\['` extracts only document lines
- `grep -v '^//'` drops metadata while keeping document lines
- `wanted` uses a report layout with `[[Missing Page]] (N links)` headers and bullet source lines

## Commands

### `lint`

Checks the undirected document graph.

- Orphan: a document with no resolved inbound or outbound links
- Island: any disconnected component outside the main connected component
- `largest_component_ratio`: size of the largest connected component divided by all documents
- `orphan_rate`: orphan documents divided by all documents
- `content_coverage`: documents with a first visible content line divided by all documents

If either exists, `lint` exits non-zero.

On failure, stderr prints:

- `// lint_failed documents=<n> orphans=<n> islands=<n> largest_component_ratio=<r> orphan_rate=<r> content_coverage=<r>`
- `// orphan`
- `[[Page Name]]: First visible line`
- `// island=<component>`
- `[[Page Name]]: First visible line`

Example:

```sh
awiki lint
awiki lint -root ~/wiki
```

On success, stdout prints:

- `// ok connected_graph documents=<n> largest_component_ratio=<r> orphan_rate=<r> content_coverage=<r>`

### `path <from> <to>`

Prints the shortest undirected path between two documents.

Each step is printed on its own line as `[[Page Name]]: First visible line`.
`path` does not print edge directions or path lengths.

Example:

```sh
awiki path "Probability" "Bayes theorem"
```

Example output:

```text
[[Probability]]: Measures how likely an event is.
[[Bayes theorem]]: Relates prior belief to updated evidence.
```

### `rename <old> <new>`

Renames `<old>.md` to `<new>.md` and rewrites links pointing to that document.

If the renamed file has front matter with `title: Old`, that title is updated to
`title: New`.

The command prints:

- `// rename old=Old.md new=New.md`
- `// links_updated=<n> files_touched=<n> title_updated=<bool>`

Example:

```sh
awiki rename "Old Title" "New Title"
```

### `links <document>`

Shows:

- inbound links from existing documents
- outbound links to existing documents
- outbound links to missing documents, marked as `(missing)`

The requested document is printed first, preceded by `// this page`. Incoming
and outgoing sections are labeled `// incoming links` and `// outgoing links`.
Each page line uses the same `[[Page Name]]: First visible line` format.

Example:

```sh
awiki links "Graph theory"
```

Example output:

```text
// this page
[[Graph theory]]: Studies graphs as mathematical objects.
// incoming links
[[Combinatorics]]: Studies discrete structures.
// outgoing links
[[Vertex]]: A node in a graph.
[[Missing note]]: (missing)
```

### `wanted`

Shows the most-linked missing pages.

- ranks missing pages by unresolved link mentions
- prints up to 10 items by default
- prints each missing page as `[[Page Name]] (N links)`
- under each page, prints bullet lines for linking documents and the exact line or front matter line that contains the link
- if a page has many incoming references, only the first few are shown, followed by `_ ...`

Example:

```sh
awiki wanted
awiki wanted -n 20
```

Example output:

```text
[[Missing note]] (5 links)

- [[Books Ive read]]: I should read more about [[Missing note]] soon.
- [[Graph theory]]: Compare this with [[Missing note]].
_ ...
```

### `avg-shortest-path`

Estimates the average shortest path length on the largest connected component
and prints sampled longer-than-average paths.

- samples document pairs instead of enumerating every pair
- prints one summary line for component size, sample count, and estimated average
- prints the pages in each sampled path as `[[Page Name]]: First visible line`
- if multiple example paths are requested, they are separated by blank lines
- the printed paths are informative long samples, not the exact graph diameter

Example:

```sh
awiki avg-shortest-path
awiki avg-shortest-path -samples 1000 -examples 3 -seed 7
```

Example output:

```text
// largest_component_size=8042 samples=500 average_shortest_path=4.9780
[[35th century BCE]]: ...
[[34th century BCE]]: ...
[[Lycurgus]]: [[9th century BCE|기원전 9세기]] 경의 전설상의 인물.
```

### `version`

Prints the build version string.

## Help

`awiki` prints the top-level command list with each command's argument shape.

When a page name contains spaces, quote it in the shell. For example:

```sh
awiki path "The China study (book)" "What to Eat"
awiki links "Books Ive read"
```

Every command supports `-h`, `-help`, and `--help`.

## Related Docs

- [Overview](overview.md) — quick start and command summary
- [Wiki Model](wiki-model.md) — document identity, link syntax, and graph rules
