mod analysis;
mod frontmatter;
mod links;
mod load;
mod model;
mod path;
mod rename;
mod suggest;

pub(crate) use model::{
    AvgPathReport, Document, FrontMatter, Link, LinkKind, LinkOnlyLine, LintReport, Options,
    RenameResult, Vault, WantedPage, WantedSource,
};
pub(crate) use suggest::{format_suggest_report, SuggestFilter, SuggestOptions};

#[cfg(test)]
mod tests;
