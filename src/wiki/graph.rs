use rustc_hash::FxHashSet;

use super::Vault;
use crate::{
    text::lower,
    wiki::path::{normalize_document_name, trim_md_ext},
};

/// One document's place in the link graph.
pub(crate) struct GraphNode {
    pub(crate) doc: usize,
    /// Distinct other documents linking here.
    pub(crate) inbound: usize,
    /// Distinct other documents linked from here.
    pub(crate) outbound: usize,
    /// Distinct missing pages linked from here, sorted by name.
    pub(crate) missing: Vec<String>,
}

impl Vault {
    /// Every document with its link counts, in name order.
    pub(crate) fn graph_nodes(&self) -> Vec<GraphNode> {
        let mut docs: Vec<usize> = (0..self.documents.len()).collect();
        self.sort_doc_indices(&mut docs);
        docs.into_iter()
            .map(|doc| GraphNode {
                doc,
                inbound: self.inbound[doc].len(),
                outbound: self.directed[doc].len(),
                missing: self.missing_targets(doc),
            })
            .collect()
    }

    /// The documents `doc` links to, in name order.
    pub(crate) fn outbound_documents(&self, doc: usize) -> Vec<usize> {
        let mut targets: Vec<usize> = self.directed[doc].iter().copied().collect();
        self.sort_doc_indices(&mut targets);
        targets
    }

    fn missing_targets(&self, doc: usize) -> Vec<String> {
        let mut seen = FxHashSet::default();
        let mut names = Vec::new();
        for link in &self.documents[doc].links {
            if link.resolved.is_some() || !seen.insert(link.target_key.clone()) {
                continue;
            }
            let name = normalize_document_name(&link.display_target);
            names.push(if name.is_empty() {
                link.display_target.trim().to_string()
            } else {
                name
            });
        }
        names.sort_by_key(|name| lower(name));
        names
    }

    /// The repo-relative file path of a document, with its extension.
    fn file_path(&self, doc: usize) -> String {
        let path = &self.documents[doc].path;
        path.strip_prefix(&self.root)
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| format!("{}.md", trim_md_ext(&self.documents[doc].rel_path)))
    }
}

/// `awiki graph`: one line per document with its link counts.
pub(crate) fn format_graph_nodes(vault: &Vault) -> String {
    let lines: Vec<String> = vault
        .graph_nodes()
        .iter()
        .map(|node| {
            format!(
                "[[{}]] inbound={} outbound={} missing={}",
                vault.documents[node.doc].name,
                node.inbound,
                node.outbound,
                node.missing.len()
            )
        })
        .collect();
    if lines.is_empty() {
        "_ none".to_string()
    } else {
        lines.join("\n")
    }
}

/// `awiki graph -edges`: one line per distinct link, resolved before missing.
pub(crate) fn format_graph_edges(vault: &Vault) -> String {
    let mut lines = Vec::new();
    for node in vault.graph_nodes() {
        let source = &vault.documents[node.doc].name;
        for target in vault.outbound_documents(node.doc) {
            lines.push(format!(
                "[[{source}]] -> [[{}]]",
                vault.documents[target].name
            ));
        }
        for missing in &node.missing {
            lines.push(format!("[[{source}]] -> [[{missing}]] (missing)"));
        }
    }
    if lines.is_empty() {
        "_ none".to_string()
    } else {
        lines.join("\n")
    }
}

/// `awiki graph -json`: documents and links as one JSON object.
pub(crate) fn format_graph_json(vault: &Vault) -> String {
    let nodes = vault.graph_nodes();
    let documents: Vec<String> = nodes
        .iter()
        .map(|node| {
            format!(
                "{{\"name\":{},\"path\":{},\"inbound\":{},\"outbound\":{},\"missing\":{}}}",
                json_string(&vault.documents[node.doc].name),
                json_string(&vault.file_path(node.doc)),
                node.inbound,
                node.outbound,
                node.missing.len()
            )
        })
        .collect();
    let mut edges = Vec::new();
    for node in &nodes {
        let source = json_string(&vault.documents[node.doc].name);
        for target in vault.outbound_documents(node.doc) {
            edges.push(format!(
                "{{\"source\":{source},\"target\":{},\"missing\":false}}",
                json_string(&vault.documents[target].name)
            ));
        }
        for missing in &node.missing {
            edges.push(format!(
                "{{\"source\":{source},\"target\":{},\"missing\":true}}",
                json_string(missing)
            ));
        }
    }
    format!(
        "{{\"documents\":[{}],\"edges\":[{}]}}",
        documents.join(","),
        edges.join(",")
    )
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
