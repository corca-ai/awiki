awiki resolves document links by normalized filenames, using source-relative Markdown paths and Obsidian-aligned wikilink rules; fragments identify locations within the selected file without affecting page selection.

## Shared rules

Paths match case-insensitively after Unicode NFC normalization. Wikilinks may
omit `.md`. Heading and block fragments such as `#Heading` and `#^block` are
removed before selecting a page. Front matter titles and aliases help CLI
lookups but do not participate in link resolution.

[`Vault::resolve_recursive()`](../src/wiki/load.rs#Vault::resolve_recursive)
applies recursive precedence. [Wiki model](<Wiki model.md>) explains document
identity and which resolved links enter the graph.

## Wikilinks

A bare `[[Note]]` searches basenames from any source document. If multiple pages
share that basename, the shortest root-relative path wins deterministically;
qualify the path to select another page.

`[[folder/Note]]` prefers a path from the wiki root. If absent, the current
implementation also tries the source-relative path.
`[[./Note]]` and `[[../folder/Note]]` resolve from the source directory.

## Markdown links

Markdown destinations resolve from the source document's directory, including
`../` segments. Spaces require angle brackets:

```md
[Wiki model](<Wiki model.md>)
[Loading](../src/wiki/load.rs#Vault::load)
```

The first link selects a wiki page. The second is a
[code reference](<Code references.md>) and is checked separately from the graph.

## Flat wikis

Without recursion, document links match basenames. Nested layouts require
`-recursive`; see the [Command-line interface](<Command-line interface.md>).
