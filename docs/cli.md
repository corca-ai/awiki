# CLI Guide

`awiki` operates on one wiki root directory at a time. Use `-root <dir>` to
target a directory explicitly; otherwise the current directory is used.

## Output Style

Command output is line-oriented and grep-friendly.

- document lines use `[[Page Name]]: First visible line`
- meta lines start with `// `
- previews come from the document's first visible content line
- if a document has no printable body content, it is shown as `[[Page Name]]: (empty)`
- when stdout is an interactive terminal, `awiki` colors the `[[Page Name]]`
  part and `// ` comments; piped output stays plain text
- `grep '^\[\['` extracts only document lines
- `grep -v '^//'` drops metadata while keeping document lines

## Commands

### `lint`

Checks the undirected document graph.

- Orphan: a document with no resolved inbound or outbound links
- Island: any disconnected component outside the main connected component

If either exists, `lint` exits non-zero.

On failure, stderr prints:

- `// lint_failed orphans=<n> islands=<n>`
- `// orphan`
- `[[Page Name]]: First visible line`
- `// island=<component>`
- `[[Page Name]]: First visible line`

Example:

```sh
awiki lint
awiki lint -root ~/wiki
```

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

### `avg-shortest-path`

Estimates the average shortest path length on the largest connected component.

- samples document pairs instead of enumerating every pair
- prints one summary line for component size, sample count, and estimated average
- prints the pages in each sampled path as `[[Page Name]]: First visible line`
- if multiple example paths are requested, they are separated by blank lines

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

`awiki` and `awiki help` print the top-level command list with each command's
argument shape.

Every command supports `-h`, `-help`, and `--help`.

## Related Docs

- [Overview](overview.md) — quick start and command summary
- [Wiki Model](wiki-model.md) — document identity, link syntax, and graph rules
