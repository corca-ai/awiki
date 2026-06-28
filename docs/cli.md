# CLI Guide

`awiki` operates on one wiki root directory at a time. Use `-root <dir>` to
target a directory explicitly; otherwise the current directory is used.

Every command accepts `-recursive` (`-r`) to walk subdirectories. By default
only top-level `.md` files are read. In recursive mode documents are identified
and printed by their repo-relative path (e.g. `[[goals/login]]`), and `rename`
accepts a path target. See [Link Resolution](link-resolution.md).

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
- `suggest` prints non-failing refactoring hints; it is meant to point you at
  pages worth inspecting, not to enforce a rule

## Choosing a Command

- Use `lint` for automation. It exits non-zero when it finds mechanical quality
  problems.
- Use `suggest` for cleanup planning. It prints candidates that should be
  reviewed by a human.
- Use `wanted` to decide which missing pages, redirects, or spelling fixes would
  remove the most unresolved links.
- Use `links` to understand one page's local neighborhood.
- Use `path` to inspect how two topics are connected through the graph.
- Use `rename` to change a page name while preserving links.

## Commands

### `lint`

Checks the undirected document graph.

- Orphan: a document with no resolved inbound or outbound links
- Island: any disconnected component outside the main connected component
- `largest_component_ratio`: size of the largest connected component divided by all documents
- `orphan_rate`: orphan documents divided by all documents
- `content_coverage`: documents with a first visible content line divided by all documents
- Link-only line: a body line whose only meaningful content is one document
  link, such as `- [[Page]]`, `**[[Page]]**`, `[Page](Page.md)`, or
  `[[Page|Label]]`. Lines with two or more links are allowed by this rule.

If any lint issue exists, `lint` exits non-zero.

On failure, stdout prints:

- `// lint_failed documents=<n> orphans=<n> islands=<n> link_only_lines=<n> largest_component_ratio=<r> orphan_rate=<r> content_coverage=<r>`
- `// orphan`
- `// why: ...`
- `// fix: ...`
- `// example: ...`
- `[[Page Name]]: First visible line`
- `// island=<component>`
- `// why: ...`
- `// fix: ...`
- `// example: ...`
- `[[Page Name]]: First visible line`
- `// link_only_line`
- `// why: ...`
- `// fix: ...`
- `// example: ...`
- `[[Page Name]]:<line>: <source line>`

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
- prints up to 10 items by default; override with `-n`
- prints each missing page as `[[Page Name]] (N links)`
- under each page, prints bullet lines for linking documents and the exact line or front matter line that contains the link
- caps the per-page referencing lines at 10 by default; override with `-sources` (use `0` for no cap)
- when truncated, the page is followed by `_ ...`

Example:

```sh
awiki wanted
awiki wanted -n 20
awiki wanted -sources 0
```

Example output:

```text
[[Missing note]] (5 links)

- [[Books Ive read]]: I should read more about [[Missing note]] soon.
- [[Graph theory]]: Compare this with [[Missing note]].
_ ...
```

### `suggest`

Shows refactoring candidates and graph-quality hints. Unlike `lint`, this
command exits zero when it finds candidates.

Default sections:

- `sampled-diameter`: samples shortest paths inside the largest connected
  component and prints the longest sampled paths with page previews. Use these
  paths to find remote topic clusters that may need bridge notes or extra links.
- `wanted-pressure`: ranks missing pages by unresolved link pressure and prints
  source context. These are good candidates for new hub pages, redirects, or
  corrected links.
- `long-pages`: lists pages above the line or word threshold. These are split,
  outline, or summary-link candidates.
- `short-stubs`: lists very short non-empty pages. These are merge, expand, or
  delete candidates.
- `near-duplicates`: finds likely duplicate pages using normalized Markdown
  text fingerprints, winnowed candidates, and n-gram similarity scores. Review
  the listed page pair before merging.

The command intentionally prints the concrete documents to inspect. For long
paths, read the endpoints and the middle bridge pages. For wanted pressure, read
the source lines before creating a page. For near-duplicates, compare both page
previews and merge only when the pages really carry the same concept.

Filters are comma-separated:

```sh
awiki suggest
awiki suggest --filter=sampled-diameter,wanted-pressure
awiki suggest --filter near-duplicates --duplicate-threshold 0.9
awiki suggest -n 20 --long-lines 160 --short-words 30
```

Common output shape:

```text
// suggest documents=8042 filters=sampled-diameter,wanted-pressure
// sampled_diameter samples=2000 sampled_diameter=9 paths=5
// why: these sampled paths are long, so related topics may only connect through many weak hops.
// fix: inspect the endpoints and middle bridge pages; add contextual links or a bridge note where the relationship is real.
// example: if two endpoints are clearly related, add a sentence-level link from the stronger overview page.
// path distance=9
[[Source page]]: First visible line.
[[Bridge page]]: First visible line.
[[Target page]]: First visible line.
// wanted_pressure pages=10
// why: many notes point at these missing pages, so the wiki already depends on absent concepts.
// fix: create the page, correct misspelled links, or consolidate equivalent names with a rename/redirect note.
// example: if [[Vector database]] is linked from many pages, create it as a hub or correct links to an existing page.
[[Missing note]] mentions=5 source_documents=3
- [[Referencing page]]: Local line that mentions [[Missing note]].
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
