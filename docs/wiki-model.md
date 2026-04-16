# Wiki Model

## Scope

`awiki` reads Markdown files directly under one wiki root directory.
Subdirectories are ignored on purpose.

## Document Identity

- the canonical document name is the filename without the `.md` suffix
- names are matched case-insensitively
- Unicode-equivalent names are matched after normalization, so composed and decomposed forms resolve to the same document
- front matter `title` is also treated as an identifier
- front matter `aliases` are also treated as identifiers
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

## Links That Do Not Count

- links to missing documents
- links inside fenced code blocks
- image links such as `![alt](image.png)`
- external URLs
- `mailto:` links
- heading-only links such as `[jump](#section)`
- self-links for graph connectivity

Broken links are preserved because they are useful placeholders in a wiki
gardening workflow.

## Graph Rules

- `lint` checks an undirected graph built from resolved links
- an orphan has no resolved inbound or outbound links
- an island is a connected component outside the largest component
- `path` uses the same undirected graph
- `links` shows resolved links first, then missing outbound links

## Rename Rules

`rename` updates:

- the document filename
- wikilinks pointing to the document in front matter or body text
- local Markdown links pointing to the document in front matter or body text
- front matter `title` when it exactly matches the old document name

`rename` does not rewrite:

- image links
- external links
- links inside fenced code blocks

## Related Docs

- [CLI Guide](cli.md) — command reference
- [Architecture](architecture.md) — where parsing and graph rules live in code
