//! Configurable lint policy; graph construction remains independent of lint rules.
use std::{collections::BTreeMap, fs, path::Path};

use globset::Glob;
use pulldown_cmark::{Event, Parser, Tag};
use serde::Deserialize;

use super::{frontmatter::parse_front_matter, path::is_document, Vault};

const RULES: &[&str] = &[
    "orphan",
    "island",
    "link-only-line",
    "broken-document-link",
    "first-block-paragraph",
    "broken-code-reference",
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Severity {
    Off,
    Warn,
    Error,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default)]
    lint: Policy,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    #[serde(default)]
    rules: BTreeMap<String, Severity>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    overrides: Vec<Override>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Override {
    files: Vec<String>,
    rules: BTreeMap<String, Severity>,
}

impl Policy {
    fn validate(&self) -> Result<(), String> {
        for key in self
            .rules
            .keys()
            .chain(self.overrides.iter().flat_map(|o| o.rules.keys()))
        {
            if !RULES.contains(&key.as_str()) {
                return Err(format!("unknown lint rule: {key}"));
            }
        }
        for pattern in self
            .exclude
            .iter()
            .chain(self.overrides.iter().flat_map(|o| &o.files))
        {
            Glob::new(pattern).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn severity(&self, path: &str, rule: &str) -> Severity {
        if matches_any(&self.exclude, path) {
            return Severity::Off;
        }
        let mut severity = self.rules.get(rule).copied().unwrap_or_else(|| {
            if ["orphan", "island", "link-only-line"].contains(&rule) {
                Severity::Error
            } else {
                Severity::Off
            }
        });
        for entry in &self.overrides {
            if matches_any(&entry.files, path) {
                if let Some(value) = entry.rules.get(rule) {
                    severity = *value;
                }
            }
        }
        severity
    }
}

fn matches_any(patterns: &[String], path: &str) -> bool {
    patterns.iter().any(|p| {
        Glob::new(p)
            .expect("validated glob")
            .compile_matcher()
            .is_match(path)
    })
}

pub(crate) struct Diagnostics {
    pub(crate) lines: Vec<String>,
    pub(crate) failed: bool,
}

impl Vault {
    /// No config preserves the existing report format and policy.
    pub(crate) fn configured_lint(&self) -> Result<Option<Diagnostics>, String> {
        let path = self.root.join("awiki.toml");
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let config: Config =
            toml::from_str(&source).map_err(|e| format!("{}: {e}", path.display()))?;
        config.lint.validate()?;
        self.lint_with_policy(&config.lint).map(Some)
    }

    fn lint_with_policy(&self, policy: &Policy) -> Result<Diagnostics, String> {
        let mut result = Diagnostics {
            lines: Vec::new(),
            failed: false,
        };
        let report = self.lint();
        let mut emit = |idx: usize, line: usize, rule: &str, message: String| {
            let path = self.documents[idx]
                .path
                .strip_prefix(&self.root)
                .expect("document in root")
                .to_string_lossy();
            let severity = policy.severity(&path, rule);
            if severity == Severity::Off {
                return;
            }
            result.failed |= severity == Severity::Error;
            let level = if severity == Severity::Error {
                "error"
            } else {
                "warn"
            };
            result
                .lines
                .push(format!("{path}:{line}: {level}[{rule}]: {message}"));
        };
        for idx in report.orphans {
            emit(idx, 1, "orphan", "no resolved document links".into());
        }
        for island in report.islands {
            for idx in island {
                emit(idx, 1, "island", "disconnected from main component".into());
            }
        }
        for (idx, issue) in report.link_only_lines {
            emit(idx, issue.line, "link-only-line", issue.text);
        }
        for (idx, doc) in self.documents.iter().enumerate() {
            let rel_path = doc
                .path
                .strip_prefix(&self.root)
                .expect("document in root")
                .to_string_lossy();
            if [
                "broken-document-link",
                "first-block-paragraph",
                "broken-code-reference",
            ]
            .iter()
            .all(|r| policy.severity(&rel_path, r) == Severity::Off)
            {
                continue;
            }
            let content = fs::read_to_string(&doc.path).map_err(|e| e.to_string())?;
            let offset = parse_front_matter(&content).body_offset;
            let body = &content[offset..];
            if policy.severity(&rel_path, "first-block-paragraph") != Severity::Off {
                let visible = mask_comments(body);
                let first = Parser::new(&visible).into_offset_iter().next();
                if !matches!(first, Some((Event::Start(Tag::Paragraph), _))) {
                    let start = first.map_or(offset, |(_, range)| offset + range.start);
                    emit(
                        idx,
                        line_at(&content, start),
                        "first-block-paragraph",
                        "body must start with a paragraph".into(),
                    );
                }
            }
            for link in &doc.links {
                if link.resolved.is_none() {
                    emit(
                        idx,
                        link.line,
                        "broken-document-link",
                        format!("unresolved document: {}", link.display_target),
                    );
                }
            }
            if policy.severity(&rel_path, "broken-code-reference") == Severity::Off {
                continue;
            }
            for (event, range) in Parser::new(body).into_offset_iter() {
                let Event::Start(Tag::Link { dest_url, .. }) = event else {
                    continue;
                };
                let Some((file, symbol)) = dest_url.split_once('#') else {
                    continue;
                };
                if file.is_empty()
                    || file.contains(':')
                    || file.starts_with("//")
                    || is_document(file)
                {
                    continue;
                }
                let target = doc.path.parent().expect("document parent").join(file);
                if let Err(message) = check_symbol(&target, symbol) {
                    emit(
                        idx,
                        line_at(&content, offset + range.start),
                        "broken-code-reference",
                        format!("{dest_url}: {message}"),
                    );
                }
            }
        }
        result.lines.sort();
        Ok(result)
    }
}

fn line_at(content: &str, offset: usize) -> usize {
    content[..offset].bytes().filter(|b| *b == b'\n').count() + 1
}

fn check_symbol(path: &Path, symbol: &str) -> Result<(), String> {
    let source = fs::read_to_string(path).map_err(|e| format!("cannot read source: {e}"))?;
    let language = match path.extension().and_then(|s| s.to_str()) {
        Some("ts") => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        Some("tsx") => tree_sitter_typescript::LANGUAGE_TSX,
        _ => return Err("unsupported source language (supported: ts, tsx)".into()),
    };
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language.into())
        .map_err(|e| e.to_string())?;
    let tree = parser.parse(&source, None).ok_or("source parsing failed")?;
    if tree.root_node().has_error() {
        return Err("source contains syntax errors".into());
    }
    let mut names = Vec::new();
    collect_declarations(tree.root_node(), source.as_bytes(), "", &mut names);
    if names.iter().any(|name| name == symbol) {
        Ok(())
    } else {
        Err(format!("declaration not found: {symbol}"))
    }
}

fn collect_declarations(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    prefix: &str,
    out: &mut Vec<String>,
) {
    let kind = node.kind();
    let declaration = matches!(
        kind,
        "function_declaration"
            | "function_signature"
            | "generator_function_declaration"
            | "class_declaration"
            | "abstract_class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration"
            | "internal_module"
            | "variable_declarator"
            | "method_definition"
            | "method_signature"
            | "public_field_definition"
            | "property_signature"
            | "enum_assignment"
    );
    let mut scope = prefix.to_string();
    if declaration {
        if let Some(name) = node.child_by_field_name("name") {
            if !matches!(
                name.kind(),
                "identifier" | "type_identifier" | "property_identifier"
            ) {
                return;
            }
            let name = name.utf8_text(source).unwrap_or("");
            scope = if prefix.is_empty() {
                name.into()
            } else {
                format!("{prefix}.{name}")
            };
            out.push(scope.clone());
        }
    }
    // Do not expose locals, inherited members, or inferred object members.
    if matches!(
        kind,
        "function_declaration"
            | "generator_function_declaration"
            | "method_definition"
            | "variable_declarator"
            | "arrow_function"
            | "function_expression"
    ) {
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_declarations(child, source, &scope, out);
    }
}

// Preserve byte positions and newlines so diagnostics still point into the source.
fn mask_comments(body: &str) -> String {
    let mut bytes = body.as_bytes().to_vec();
    let mut start = 0;
    while let Some(at) = body[start..].find("<!--") {
        let at = start + at;
        let end = body[at + 4..]
            .find("-->")
            .map_or(body.len(), |n| at + 4 + n + 3);
        for byte in &mut bytes[at..end] {
            if *byte != b'\n' && *byte != b'\r' {
                *byte = b' ';
            }
        }
        start = end;
    }
    String::from_utf8(bytes).expect("comment bytes replaced with ASCII")
}

#[cfg(test)]
mod tests;
