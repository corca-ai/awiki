An awiki.toml file in a wiki root selects lint rules, diagnostic severities, path exclusions, and per-file overrides; only errors cause configured lint runs to fail.

## Configuration file

`awiki lint docs/` reads `docs/awiki.toml`. With `-root`, it reads that root's
configuration; with neither, it uses the current directory. There is no parent
config search or configuration merging. Only `lint` reads this file.

```toml
[lint.rules]
orphan = "warn"
island = "error"
link-only-line = "error"
broken-document-link = "error"
first-block-paragraph = "error"
broken-code-reference = "error"

[[lint.overrides]]
files = ["README.md", "**/README.md"]
[lint.overrides.rules]
first-block-paragraph = "off"
```

Each rule accepts `off`, `warn`, or `error`. Unspecified rules retain their
defaults. Unknown keys, rule names, severity values, and invalid globs are errors.
[`Vault::configured_lint()`](../src/wiki/lint.rs#Vault::configured_lint) loads
and validates the policy before checking documents.

## Rules

| Rule | Default | Check |
| --- | --- | --- |
| `orphan` | `error` | A document has no resolved links to or from other documents. |
| `island` | `error` | A connected component lies outside the main component. |
| `link-only-line` | `error` | A body line contains only one document link and no meaningful context. |
| `broken-document-link` | `off` | A document link cannot resolve to a page in the loaded wiki. |
| `first-block-paragraph` | `off` | The first body block is not a paragraph. |
| `broken-code-reference` | `off` | A local code fragment cannot resolve to a supported source declaration. |

Lines with two or more document links are allowed by `link-only-line`.
`first-block-paragraph` ignores front matter, blank lines, and HTML comments;
empty documents, headings, lists, quotes, code, and HTML blocks fail. This rule
checks structure, not whether a paragraph is a good summary.

Document-link checking uses [Link resolution](<Link resolution.md>) and checks
page targets, not heading or block anchors. Source and asset links are not
missing wiki pages. [Code references](<Code references.md>) describes source
languages and declaration boundaries.

## Path exceptions

To suppress all diagnostics for generated documents, place this table before
`[lint.rules]`:

```toml
[lint]
exclude = ["generated/**"]
```

Override `files` and `exclude` globs match actual root-relative filenames,
including `.md`. Later matching overrides win per rule; exclusions take
precedence. Excluded documents remain in the graph and can be link targets.

## Output and exit status

Configured text diagnostics use `path.md:line:column: error[rule]: message` or `warn` in
stable order. A run with errors exits nonzero. A successful run ends with
`// ok lint documents=<n> diagnostics=<n>`, including warning-only runs.

Without configuration, lint preserves the legacy report: graph and link-only
issues are errors, failures start with `// lint_failed`, and successful runs
start with `// ok connected_graph`. Reports include document count,
`largest_component_ratio`, `orphan_rate`, and `content_coverage`. These mean the
largest component's share, the orphan share, and the share with body previews.
[`LintReport`](../src/wiki/model.rs#LintReport) holds the metrics and issues;
[`format_lint_report()`](../src/output.rs#format_lint_report) prints legacy
sections with explanations, fixes, and page context.

Run checks through the [Command-line interface](<Command-line interface.md>).
[Testing](Testing.md) describes the policy used by this repository.

## JSON diagnostics

Use `awiki lint docs/ -json` (or `--json`) for one JSON report on stdout,
including when diagnostics cause a nonzero exit. It contains `schema_version`
(currently `1`), `documents`, `errors`, `warnings`, and a `diagnostics` array.
Each diagnostic has `path`, `line`, `column`, `rule`, `severity`, and `message`.
Paths are root-relative with `/` separators. Lines and columns are 1-based;
columns count Unicode scalar values. Results sort by path and numeric position.

JSON also works without configuration, using default rule severities.
Configuration, loading, and usage failures go to stderr with nonzero status.
[`Diagnostics::render_json()`](../src/wiki/lint/diagnostic.rs#Diagnostics::render_json)
serializes the report from the same structured data used for text output.
