An awiki wiki is a set of Markdown pages identified by filenames and connected by resolved document links; front matter supports direct lookups, while graph analysis and rename share canonical page identities.

## Files and identity

By default only top-level `.md` files under one root are loaded. Recursive mode
includes subdirectories, skipping hidden directories and generated/dependency
directories such as `target`, `node_modules`, and `vendor`.
[`Vault::load()`](../src/wiki/load.rs#Vault::load) discovers files and builds
the graph.

A flat page's identity is its filename without `.md`; a recursive page's
identity is its root-relative path without `.md`. Names match case-insensitively
after Unicode NFC normalization, implemented by
[`document_key()`](../src/wiki/path.rs#document_key) and
[`document_path_key()`](../src/wiki/path.rs#document_path_key).

Front matter `title` and `aliases` help direct CLI lookups, but do not resolve
links. Ambiguous direct lookups fail instead of guessing.

## Front matter

[`parse_front_matter()`](../src/wiki/frontmatter.rs#parse_front_matter) reads
`title` and `aliases`. Aliases may be a YAML list, inline list, or scalar:

```yaml
---
title: Graph theory
aliases:
  - Graphs
  - Network theory
---
```

## Document links

Recognized forms include wikilinks with optional labels and fragments, and
local Markdown links:

```md
[[Note]]
[[Note|Label]]
[[Note#Heading|Label]]
[[Note\|Label]]
[Note](Note.md)
[Note][page]

[page]: Note.md
[Wiki model](<Wiki model.md>)
```

The escaped alias separator is useful inside Markdown table cells. These forms
are recognized in front matter and body text by
[`parse_document_links()`](../src/wiki/links.rs#parse_document_links).
[Link resolution](<Link resolution.md>) defines which page each target selects.

A specdown trace label such as `[covers::Login](login.md)` is an ordinary link:
only its destination affects the graph; the prefix remains in raw source text.

## Graph membership

Only resolved links to other documents contribute graph edges. Missing pages,
self-links, fenced or indented code, HTML comments, images, external URLs, `mailto:` destinations,
and heading-only jumps do not connect pages. Markdown paths with a non-`.md`
extension are source/asset references, not wanted pages or graph nodes.

Broken links remain useful placeholders and appear in `wanted`; projects can
make them errors through [Lint configuration](<Lint configuration.md>).
[Code references](<Code references.md>) are validated separately.

## Graph checks

[`Vault::lint()`](../src/wiki/analysis.rs#Vault::lint) checks the undirected graph.
An orphan has no resolved inbound or outbound links. An island is a component
outside the largest component. The `path` command uses the same undirected
connectivity; `links`, `graph`, and `wanted` retain link direction for reports.

The [Command-line interface](<Command-line interface.md>) describes these
commands and the non-failing cleanup hints produced by `suggest`.

## Rename

[`Vault::rename()`](../src/wiki/rename.rs#Vault::rename) changes the filename,
updates wikilinks and Markdown links that resolve to the page, and updates
front matter `title` when it exactly matches the old basename.

Recursive renames can move files, create parent directories, and recompute
relative paths. A bare wikilink stays bare only when it still resolves to the
renamed page. Fragments and labels are preserved; images, external links, and
code-fence contents are not rewritten.

Both inline and reference-style Markdown links are resolved and rewritten.
When a reference definition is shared with an image, rename converts the page
reference to an inline link so the image destination remains unchanged. Moving
a page also adjusts its outgoing relative document and source/asset links.
[File changes](<File changes.md>) covers previews and recovery.
