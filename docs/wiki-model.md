# Wiki Model

## Scope

By default `awiki` reads Markdown files directly under one wiki root directory
and ignores subdirectories — the recommended folderless layout.

Passing `-recursive` (`-r`) walks subdirectories too. Hidden directories
(`.git`, `.obsidian`, …) and `node_modules`/`vendor` are skipped. A flat
single-directory vault behaves identically under either mode, so recursion is a
strict superset you opt into.

## Document Identity

- the canonical document name is the filename without the `.md` suffix in a flat
  vault, and the repo-relative path without the `.md` suffix (e.g. `goals/login`)
  in a recursive vault
- in a recursive vault, two files with the same basename in different folders are
  distinct documents, addressable by path
- names are matched case-insensitively
- Unicode-equivalent names are matched after normalization, so composed and decomposed forms resolve to the same document
- front matter `title` may be used for direct document lookup
- front matter `aliases` may be used for direct document lookup
- ambiguous identifiers fail instead of guessing

## Front Matter

Supported keys:

- `title`
- `aliases`

`aliases` may be written as:

- a YAML list
- an inline YAML list
- a single scalar value

## Links That Count

Resolved links contribute to graph connectivity only when they point to an
existing document.

Link resolution matches canonical document identity, not front matter `title`
or `aliases`. In a flat vault that identity is the basename; in a recursive
vault links resolve by repo-relative path, following the Obsidian-aligned rules
in [Link Resolution](link-resolution.md) (bare `[[Note]]` resolves to a unique
basename and prefers the shortest path; `[[folder/Note]]` is vault-absolute;
markdown and `[[../Note]]` links are source-relative).

Supported link forms:

- `[[Note]]`
- `[[Note|Label]]`
- `[[Note\|Label]]` when the alias separator must be escaped inside a Markdown table cell
- `[[Note#Heading]]`
- `[[Note#Heading|Label]]`
- `[label](Note.md)`
- `[label](./Note.md#Heading)`
- `[label](<Note.md>)`

The same forms are recognized in front matter values and in the Markdown body.

### specdown trace links

A specdown trace link is an ordinary Markdown link whose text carries an
`<edge>::` prefix, e.g. `[covers::Login](login.md)`. `awiki` ignores the prefix
and resolves the destination only, so trace links count as plain undirected
edges and keep the graph connected. The prefix never appears in resolved page
names; it shows up only where `awiki` echoes a raw source line verbatim (the
`wanted` source context and first-line previews), the same as any other raw
Markdown token.

## Links That Do Not Count

- links to missing documents
- links inside fenced code blocks
- image links such as `![alt](image.png)`
- external URLs
- `mailto:` links
- heading-only links such as `[jump](#section)`
- self-links for graph connectivity

Broken links are preserved because they are useful placeholders in a wiki
gardening workflow and for ranking missing pages with `wanted`.

## Graph Rules

- `lint` checks an undirected graph built from resolved links
- an orphan has no resolved inbound or outbound links
- an island is a connected component outside the largest component
- `lint` also flags body lines whose only meaningful content is one document
  link, because link-only lines lack reading context; lines with two or more
  links are allowed by this rule
- `path` uses the same undirected graph
- `links` shows resolved links first, then missing outbound links
- `wanted` ranks unresolved links by missing target page
- `suggest` uses graph samples, unresolved-link pressure, page length, and
  near-duplicate text fingerprints to point at refactoring candidates without
  failing the command

## Rename Rules

`rename` updates:

- the document filename
- wikilinks pointing to the document in front matter or body text
- local Markdown links pointing to the document in front matter or body text
- front matter `title` when it exactly matches the old document basename

In a recursive vault, `rename` also accepts a repo-relative path target, keeps
the document in its directory (or moves it across directories, creating parents
as needed), recomputes relative link text (`../`) from each referencing
document, and rewrites only links that resolve to the renamed document.

`rename` does not rewrite:

- image links
- external links
- links inside fenced code blocks

## Related Docs

- [Link Resolution](link-resolution.md) — Obsidian-aligned resolution rules for both link forms
- [CLI Guide](cli.md) — command reference
- [Architecture](architecture.md) — where parsing and graph rules live in code
