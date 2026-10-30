mod analysis;
mod format;
mod frontmatter;
mod graph;
mod links;
mod load;
mod model;
mod path;
mod rename;
mod suggest;

pub(crate) use graph::{format_graph_edges, format_graph_json, format_graph_nodes};
pub(crate) use model::{
    AvgPathReport, Document, FrontMatter, Link, LinkKind, LinkOnlyLine, LintReport, Options,
    RenameResult, Vault, WantedPage, WantedSource,
};
pub(crate) use suggest::{format_suggest_report, SuggestFilter, SuggestOptions};

#[cfg(test)]
mod tests;
