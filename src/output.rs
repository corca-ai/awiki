use rustc_hash::FxHashMap;

use crate::{
    text::{lower, truncate_runes},
    wiki::{LintReport, Vault},
};

const PREVIEW_LIMIT: usize = 140;

impl Vault {
    pub(crate) fn document_line(&self, idx: usize) -> String {
        format_document_line(
            &self.documents[idx].name,
            &truncate_runes(self.documents[idx].excerpt.trim(), PREVIEW_LIMIT),
        )
    }

    pub(crate) fn outbound_summaries(&self, idx: usize) -> Vec<(String, bool, Option<usize>)> {
        let mut seen: FxHashMap<String, (String, bool, Option<usize>)> = FxHashMap::default();
        for link in &self.documents[idx].links {
            if let Some(target) = link.resolved {
                let name = self.documents[target].name.clone();
                seen.insert(format!("doc:{}", lower(&name)), (name, false, Some(target)));
            } else {
                let name = link.display_target.clone();
                seen.insert(format!("missing:{}", lower(&name)), (name, true, None));
            }
        }
        let mut out: Vec<_> = seen.into_values().collect();
        out.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| lower(&a.0).cmp(&lower(&b.0))));
        out
    }
}

pub(crate) fn format_lint_report(vault: &Vault, report: &LintReport) -> String {
    let mut out = format!(
        "// lint_failed documents={} orphans={} islands={} link_only_lines={} largest_component_ratio={:.4} orphan_rate={:.4} content_coverage={:.4}",
        report.document_count,
        report.orphans.len(),
        report.islands.len(),
        report.link_only_lines.len(),
        report.largest_component_ratio(),
        report.orphan_rate(),
        report.content_coverage()
    );
    if !report.orphans.is_empty() {
        out.push_str("\n// orphan");
        for &idx in &report.orphans {
            out.push('\n');
            out.push_str(&vault.document_line(idx));
        }
    }
    for (i, island) in report.islands.iter().enumerate() {
        out.push_str(&format!("\n// island={}", i + 1));
        for &idx in island {
            out.push('\n');
            out.push_str(&vault.document_line(idx));
        }
    }
    if !report.link_only_lines.is_empty() {
        out.push_str("\n// link_only_line");
        for (idx, issue) in &report.link_only_lines {
            out.push_str(&format!(
                "\n[[{}]]:{}: {}",
                vault.documents[*idx].name, issue.line, issue.text
            ));
        }
    }
    out
}

fn format_document_line(name: &str, preview: &str) -> String {
    let preview = preview.trim();
    if preview.is_empty() {
        format!("[[{name}]]: (empty)")
    } else {
        format!("[[{name}]]: {preview}")
    }
}
