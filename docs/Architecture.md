awiki separates command orchestration, Markdown parsing, graph analysis, lint policy, and code-reference validation into focused Rust modules that share the in-memory wiki model.

## Command flow

[`main()`](../src/main.rs#main) calls the command dispatcher
[`run()`](../src/cli.rs#run). Commands load the wiki, call model operations, and
translate results into terminal output and exit status. Lint orchestration lives
in [`lint_cmd()`](../src/cli/lint.rs#lint_cmd); configurable policy lives in
[`Vault::configured_lint()`](../src/wiki/lint.rs#Vault::configured_lint).

The [Command-line interface](<Command-line interface.md>) describes observable
behavior; [Lint configuration](<Lint configuration.md>) describes rule policy.

## Wiki loading

[`Vault`](../src/wiki/model.rs#Vault) stores documents, identity indexes, and
directed, inbound, and undirected graphs.
[`Vault::load()`](../src/wiki/load.rs#Vault::load) discovers files deterministically,
parses documents with Rayon, resolves links, and builds index-based adjacency.

[`parse_front_matter()`](../src/wiki/frontmatter.rs#parse_front_matter) extracts
lookup metadata. [`Markdown::parse()`](../src/wiki/markdown.rs#Markdown::parse)
collects links and source ranges with pulldown-cmark, including wikilinks and
reference definitions. [`parse_document_links()`](../src/wiki/links.rs#parse_document_links)
selects graph links and link-only lines from that common syntax. Lint, rename,
and format consume the same ranges; comments, code, and images are excluded. Normalized identity and resolution
follow [Wiki model](<Wiki model.md>) and [Link resolution](<Link resolution.md>).

## Analysis and changes

[`Vault::lint()`](../src/wiki/analysis.rs#Vault::lint) computes graph issues;
[`Vault::shortest_path_indices()`](../src/wiki/analysis.rs#Vault::shortest_path_indices)
finds routes. [`Vault::suggest()`](../src/wiki/suggest.rs#Vault::suggest) delegates
to graph sampling, page-length, and duplicate-detection helpers.

[`Vault::format()`](../src/wiki/format.rs#Vault::format) and
[`Vault::rename()`](../src/wiki/rename.rs#Vault::rename) prepare changes before
[`ChangePlan::apply()`](../src/wiki/changes.rs#ChangePlan::apply) stages and saves
them. [File changes](<File changes.md>) explains dry runs and failure recovery.

## Code validation and output

[`SourceCache::check()`](../src/wiki/source.rs#SourceCache::check) uses Tree-sitter's
TypeScript and Rust grammars to collect file-local declarations once per source
file per lint run. Serde/TOML and compiled glob sets implement lint policy. [Code references](<Code references.md>) defines the supported scope.

[`format_lint_report()`](../src/output.rs#format_lint_report) formats legacy lint
reports; [`format_graph_json()`](../src/wiki/graph.rs#format_graph_json) produces
machine-readable graph output. Configured lint uses structured
[`Diagnostic`](../src/wiki/lint/diagnostic.rs#Diagnostic) values for text and JSON. Shared text scanning starts with
[`scan_lines()`](../src/text.rs#scan_lines).

Use [Testing](Testing.md) for validation commands and
[Build and run](<Build and run.md>) for the pinned toolchain.

See [Performance](Performance.md) for the repeated-reference benchmark and cache lifetime.
