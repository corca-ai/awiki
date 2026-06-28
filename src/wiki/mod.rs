mod analysis;
mod frontmatter;
mod links;
mod load;
mod model;
mod path;
mod rename;

pub(crate) use model::{
    AvgPathReport, Document, FrontMatter, Link, LinkKind, LinkOnlyLine, LintReport, Options,
    RenameResult, Vault, WantedPage, WantedSource,
};

#[cfg(test)]
mod tests;
