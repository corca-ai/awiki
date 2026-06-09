# Link Resolution

How `awiki` resolves a link to a document. In a flat vault every rule below
reduces to "match the basename", so flat behavior is unchanged. The path rules
apply when a vault is loaded with `-recursive`.

## Common Rules

- Resolution is case-insensitive and Unicode-NFC normalized across the whole path.
- A trailing `.md` is optional for wikilinks and ignored when comparing.
- A `#heading` or `#^block` fragment never affects which file is chosen; it is
  split off before resolution.
- Front matter `title` and `aliases` are **not** consulted for link resolution
  (they only help direct CLI lookups). This matches Obsidian.

## Wikilinks `[[…]]`

- **Bare** `[[Note]]` (no slash) resolves source-independently: if the basename
  is unique in the vault it resolves to that file from anywhere. On a collision
  the **shortest path** (closest to the root) wins, deterministically; qualify
  the link to disambiguate.
- **Path-qualified** `[[folder/Note]]` is **vault-absolute** from the root and
  has the highest precedence.
- **Relative** `[[./Note]]` and `[[../folder/Note]]` resolve against the source
  document's directory.

This reproduces Obsidian's `getFirstLinkpathDest` precedence.

## Markdown Links `[label](path.md)`

Markdown link destinations are always **source-relative**: the path is resolved
against the source document's directory (`../` segments included), then matched
by repo-relative path. `[label](<path.md>)` angle-bracket forms and `#fragment`
suffixes are handled the same way.

## Flat Vaults

With no subdirectories every document's path is its basename and the source
directory is empty, so the path rules collapse to a basename lookup — exactly
the original flat behavior.

## Related Docs

- [Wiki Model](wiki-model.md) — identity, front matter, and graph rules
- [CLI Guide](cli.md) — the `-recursive` flag and command behavior
