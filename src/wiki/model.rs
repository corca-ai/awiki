use rustc_hash::{FxHashMap, FxHashSet};
use std::path::PathBuf;

use crate::text::{lower, ratio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinkKind {
    Wiki,
    Markdown,
}

#[derive(Clone, Debug)]
pub(crate) struct Link {
    pub(crate) kind: LinkKind,
    pub(crate) display_target: String,
    pub(crate) target_key: String,
    pub(crate) raw_target: String,
    pub(crate) resolved: Option<usize>,
    pub(crate) context: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FrontMatter {
    pub(crate) present: bool,
    pub(crate) body_offset: usize,
    pub(crate) title: String,
    pub(crate) aliases: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct LinkOnlyLine {
    pub(crate) line: usize,
    pub(crate) text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Document {
    pub(crate) name: String,
    pub(crate) key: String,
    pub(crate) path: PathBuf,
    pub(crate) rel_path: String,
    pub(crate) excerpt: String,
    pub(crate) front_matter: FrontMatter,
    pub(crate) links: Vec<Link>,
    pub(crate) link_only: Vec<LinkOnlyLine>,
}

pub(crate) struct Vault {
    pub(crate) root: PathBuf,
    pub(crate) recursive: bool,
    pub(crate) documents: Vec<Document>,
    pub(crate) docs_by_key: FxHashMap<String, usize>,
    pub(crate) identifiers: FxHashMap<String, Vec<usize>>,
    pub(crate) basenames: FxHashMap<String, Vec<usize>>,
    pub(crate) directed: Vec<FxHashSet<usize>>,
    pub(crate) inbound: Vec<FxHashSet<usize>>,
    pub(crate) undirected: Vec<FxHashSet<usize>>,
    pub(crate) neighbors: Vec<Vec<usize>>,
}

#[derive(Clone, Copy)]
pub(crate) struct Options {
    pub(crate) recursive: bool,
}

pub(crate) struct LintReport {
    pub(crate) document_count: usize,
    pub(crate) largest_component_size: usize,
    pub(crate) covered_documents: usize,
    pub(crate) orphans: Vec<usize>,
    pub(crate) islands: Vec<Vec<usize>>,
    pub(crate) link_only_lines: Vec<(usize, LinkOnlyLine)>,
}

impl LintReport {
    pub(crate) fn has_issues(&self) -> bool {
        !self.orphans.is_empty() || !self.islands.is_empty() || !self.link_only_lines.is_empty()
    }
    pub(crate) fn largest_component_ratio(&self) -> f64 {
        ratio(self.largest_component_size, self.document_count)
    }
    pub(crate) fn orphan_rate(&self) -> f64 {
        ratio(self.orphans.len(), self.document_count)
    }
    pub(crate) fn content_coverage(&self) -> f64 {
        ratio(self.covered_documents, self.document_count)
    }
}

#[derive(Clone)]
pub(crate) struct WantedSource {
    pub(crate) document: String,
    pub(crate) context: String,
    pub(crate) mentions: usize,
}

pub(crate) struct WantedPage {
    pub(crate) name: String,
    pub(crate) mentions: usize,
    pub(crate) source_documents: usize,
    pub(crate) sources: Vec<WantedSource>,
}

pub(crate) type AvgPathReport = (usize, usize, f64, Vec<Vec<usize>>);

pub(crate) struct RenameResult {
    pub(crate) old_name: String,
    pub(crate) new_name: String,
    pub(crate) files_touched: usize,
    pub(crate) links_updated: usize,
    pub(crate) title_updated: bool,
}

impl Vault {
    pub(crate) fn sort_doc_indices(&self, docs: &mut [usize]) {
        docs.sort_by(|&a, &b| lower(&self.documents[a].name).cmp(&lower(&self.documents[b].name)));
    }
}
