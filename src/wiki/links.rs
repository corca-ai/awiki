use super::{
    markdown::{local_target, Markdown},
    path::{document_key, is_document, normalize_document_name},
    Link, LinkKind, LinkOnlyLine,
};
use crate::text::{
    contains_letter_or_digit, normalize_preview_line, scan_lines, strip_line_only_markdown,
    trim_line,
};

pub(crate) fn parse_document_links(
    content: &str,
    parsed: &Markdown,
) -> (Vec<Link>, Vec<LinkOnlyLine>) {
    let mut links = Vec::new();
    let mut issues = Vec::new();
    let mut by_line = std::collections::BTreeMap::<usize, Vec<&super::markdown::SyntaxLink>>::new();
    let lines = scan_lines(content);
    let body_offset = super::frontmatter::parse_front_matter(content).body_offset;
    for syntax in &parsed.links {
        if !local_target(&syntax.target) {
            continue;
        }
        let (base, suffix) = split_target_suffix(&syntax.target);
        if syntax.kind == LinkKind::Markdown && !is_document(base) {
            continue;
        }
        let key = document_key(base);
        if key.is_empty() {
            continue;
        }
        let index = lines.partition_point(|(_, _, end)| *end <= syntax.span.start);
        let (line, start, end) = lines[index];
        if start >= body_offset && syntax.span.end <= end {
            by_line.entry(index).or_default().push(syntax);
        }
        links.push(Link {
            line: index + 1,
            column: content[start..syntax.span.start].chars().count() + 1,
            kind: syntax.kind,
            display_target: format!("{}{}", display_target(base), suffix),
            target_key: key,
            raw_target: raw_link_target(base),
            resolved: None,
            context: normalize_preview_line(trim_line(line)),
        });
    }
    for (index, spans) in by_line {
        let (line, start, end) = lines[index];
        if spans.len() != 1 {
            continue;
        }
        let span = &spans[0].span;
        let rest = format!(
            "{} {}",
            &content[start..span.start],
            &content[span.end..end]
        );
        if !contains_letter_or_digit(&strip_line_only_markdown(&rest)) {
            issues.push(LinkOnlyLine {
                line: index + 1,
                text: trim_line(line).trim().into(),
            });
        }
    }
    (links, issues)
}

pub(crate) fn split_wiki_link_parts(inner: &str) -> (&str, &str, bool, bool) {
    let bytes = inner.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'|' {
            if has_escaped_pipe(bytes, i) {
                return (&inner[..i - 1], &inner[i + 1..], true, true);
            }
            return (&inner[..i], &inner[i + 1..], true, false);
        }
    }
    (inner, "", false, false)
}

fn has_escaped_pipe(bytes: &[u8], pipe: usize) -> bool {
    if pipe == 0 || bytes[pipe - 1] != b'\\' {
        return false;
    }
    let mut slashes = 0;
    let mut i = pipe;
    while i > 0 && bytes[i - 1] == b'\\' {
        slashes += 1;
        i -= 1;
    }
    slashes % 2 == 1
}

pub(crate) fn split_target_suffix(target: &str) -> (&str, &str) {
    if let Some(idx) = target.find('#') {
        (&target[..idx], &target[idx..])
    } else {
        (target, "")
    }
}

fn display_target(base: &str) -> String {
    let name = normalize_document_name(base);
    if name.is_empty() {
        base.trim().to_string()
    } else {
        name
    }
}

fn raw_link_target(base: &str) -> String {
    let value = base.trim().trim_matches(['<', '>']).trim();
    let (base, _) = split_target_suffix(value);
    base.trim().to_string()
}
