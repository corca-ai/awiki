//! Configurable lint policy; graph construction remains independent of lint rules.
use super::{
    links::split_target_suffix, markdown::local_target, path::is_document, source::SourceCache,
    Vault,
};
use diagnostic::{Diagnostic, Diagnostics, Severity};
use policy::{CompiledPolicy, Config, Policy};
use std::fs;
mod diagnostic;
mod local;
mod policy;

impl Vault {
    /// No config preserves the existing text report format and policy.
    pub(crate) fn configured_lint(&self) -> Result<Option<Diagnostics>, String> {
        let path = self.root.join("awiki.toml");
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let config: Config =
            toml::from_str(&source).map_err(|e| format!("{}: {e}", path.display()))?;
        self.lint_with_policy(&config.lint.validate()?).map(Some)
    }
    pub(crate) fn default_diagnostics(&self) -> Result<Diagnostics, String> {
        self.lint_with_policy(&Policy::default().validate()?)
    }
    fn lint_with_policy(&self, policy: &CompiledPolicy) -> Result<Diagnostics, String> {
        let mut result = Diagnostics {
            items: Vec::new(),
            documents: self.documents.len(),
        };
        let report = self.lint();
        let mut emit = |idx: usize, line: usize, column: usize, rule: &str, message: String| {
            let path = self.documents[idx]
                .path
                .strip_prefix(&self.root)
                .expect("document in root")
                .to_string_lossy()
                .replace('\\', "/");
            let severity = policy.severity(&path, rule);
            if severity != Severity::Off {
                result.items.push(Diagnostic {
                    path,
                    line,
                    column,
                    rule: rule.into(),
                    severity,
                    message,
                });
            }
        };
        for idx in report.orphans {
            emit(idx, 1, 1, "orphan", "no resolved document links".into());
        }
        for island in report.islands {
            for idx in island {
                emit(
                    idx,
                    1,
                    1,
                    "island",
                    "disconnected from main component".into(),
                );
            }
        }
        for (idx, issue) in report.link_only_lines {
            emit(idx, issue.line, 1, "link-only-line", issue.text);
        }
        let mut sources = SourceCache::default();
        let mut resources = local::ResourceChecker::default();
        for (idx, doc) in self.documents.iter().enumerate() {
            let path = doc
                .path
                .strip_prefix(&self.root)
                .expect("document in root")
                .to_string_lossy()
                .replace('\\', "/");
            let parsed = &doc.markdown;
            if policy.severity(&path, "broken-reference-link") != Severity::Off {
                for reference in &parsed.broken_references {
                    let (line, column) = position(&doc.content, reference.span.start);
                    emit(
                        idx,
                        line,
                        column,
                        "broken-reference-link",
                        format!("undefined reference: {}", reference.label),
                    );
                }
            }
            if policy.severity(&path, "first-block-paragraph") != Severity::Off
                && !parsed.first_paragraph
            {
                let (line, column) = position(&doc.content, parsed.first_offset);
                emit(
                    idx,
                    line,
                    column,
                    "first-block-paragraph",
                    "body must start with a paragraph".into(),
                );
            }
            if policy.severity(&path, "broken-document-link") != Severity::Off {
                for link in &doc.links {
                    let local_severity = policy.severity(&path, "broken-local-link");
                    let covered = link.kind == super::LinkKind::Markdown
                        && at_least(
                            local_severity,
                            policy.severity(&path, "broken-document-link"),
                        )
                        && parsed.local_resources.iter().any(|resource| {
                            resource.code_checked
                                && position(&doc.content, resource.span.start)
                                    == (link.line, link.column)
                        });
                    if link.resolved.is_none() && !covered {
                        emit(
                            idx,
                            link.line,
                            link.column,
                            "broken-document-link",
                            format!("unresolved document: {}", link.display_target),
                        );
                    }
                }
            }
            if policy.severity(&path, "broken-local-link") != Severity::Off {
                for resource in &parsed.local_resources {
                    let (file, fragment) = split_target_suffix(&resource.target);
                    let file = file.split('?').next().unwrap_or("");
                    let code_target = resource.code_checked
                        && !fragment.is_empty()
                        && local::is_source(std::path::Path::new(file));
                    if code_target
                        && at_least(
                            policy.severity(&path, "broken-code-reference"),
                            policy.severity(&path, "broken-local-link"),
                        )
                    {
                        continue;
                    }
                    let target = if code_target { file } else { &resource.target };
                    if let Err(message) = resources.check(&doc.path, target) {
                        let (line, column) = position(&doc.content, resource.span.start);
                        emit(
                            idx,
                            line,
                            column,
                            "broken-local-link",
                            format!("{}: {message}", resource.target),
                        );
                    }
                }
            }
            if policy.severity(&path, "broken-code-reference") == Severity::Off {
                continue;
            }
            for link in &parsed.links {
                if link.kind != super::LinkKind::Markdown
                    || link.span.start < doc.front_matter.body_offset
                {
                    continue;
                }
                if !local_target(&link.target) {
                    continue;
                }
                let (file, fragment) = split_target_suffix(&link.target);
                let file = file.split('?').next().unwrap_or("");
                if fragment.is_empty() || is_document(file) {
                    continue;
                }
                if at_least(
                    policy.severity(&path, "broken-local-link"),
                    policy.severity(&path, "broken-code-reference"),
                ) && local::is_markup(std::path::Path::new(file))
                {
                    continue;
                }
                let target = doc.path.parent().expect("document parent").join(file);
                if let Err(message) = sources.check(&target, &fragment[1..]) {
                    let (line, column) = position(&doc.content, link.span.start);
                    emit(
                        idx,
                        line,
                        column,
                        "broken-code-reference",
                        format!("{}: {message}", link.target),
                    );
                }
            }
        }
        result.sort();
        Ok(result)
    }
}
fn at_least(candidate: Severity, required: Severity) -> bool {
    candidate != Severity::Off && (candidate == Severity::Error || candidate == required)
}
fn position(content: &str, offset: usize) -> (usize, usize) {
    let prefix = &content[..offset];
    (
        prefix.bytes().filter(|b| *b == b'\n').count() + 1,
        prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
    )
}
#[cfg(test)]
mod tests;
