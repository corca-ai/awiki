use std::fs;

use super::Vault;
use crate::{
    text::{fence_start, next_fence_state, scan_lines, trim_line},
    wiki::links::split_wiki_link_parts,
};

pub(crate) struct FormatReport {
    pub(crate) documents: usize,
    pub(crate) changed: Vec<String>,
}

impl Vault {
    pub(crate) fn format(&self) -> Result<FormatReport, String> {
        let mut changed = Vec::new();
        for doc in &self.documents {
            let content = fs::read_to_string(&doc.path)
                .map_err(|e| format!("{}: {e}", doc.path.display()))?;
            let formatted = format_markdown_document(&content);
            if formatted != content {
                fs::write(&doc.path, formatted)
                    .map_err(|e| format!("{}: {e}", doc.path.display()))?;
                changed.push(doc.rel_path.clone());
            }
        }
        Ok(FormatReport {
            documents: self.documents.len(),
            changed,
        })
    }
}

pub(super) fn format_markdown_document(content: &str) -> String {
    let (frontmatter, body) = split_frontmatter(content);
    let mut out = String::new();
    if let Some(lines) = frontmatter {
        out.push_str(&format_frontmatter(lines));
    }
    out.push_str(&format_body(body));
    ensure_final_newline(out)
}

fn split_frontmatter(content: &str) -> (Option<Vec<&str>>, &str) {
    let lines = scan_lines(content);
    if lines.first().map(|l| trim_line(l.0)) != Some("---") {
        return (None, content);
    }
    let Some(end_idx) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| trim_line(l.0) == "---")
        .map(|(i, _)| i)
    else {
        return (None, content);
    };
    (
        Some(lines[1..end_idx].iter().map(|line| line.0).collect()),
        &content[lines[end_idx].2..],
    )
}

fn format_frontmatter(lines: Vec<&str>) -> String {
    let entries = parse_frontmatter_entries(lines);
    let mut title = None;
    let mut aliases = Vec::new();
    let mut tags = Vec::new();
    let mut other = Vec::new();
    for entry in entries {
        match entry.key.as_deref() {
            Some("title") => {
                if let Some(value) = entry.scalar_value() {
                    title = Some(trim_yaml_scalar(value).to_string());
                } else {
                    other.push(entry.lines);
                }
            }
            Some("aliases") => aliases.extend(entry.list_values()),
            Some("tags") => tags.extend(entry.list_values()),
            _ => other.push(entry.lines),
        }
    }

    let mut out = String::from("---\n");
    if let Some(title) = title.filter(|v| !v.trim().is_empty()) {
        out.push_str(&format!("title: {}\n", format_yaml_scalar(title.trim())));
    }
    append_yaml_list(&mut out, "aliases", aliases);
    append_yaml_list(&mut out, "tags", tags);
    for lines in other {
        for line in lines {
            out.push_str(trim_line(&line));
            out.push('\n');
        }
    }
    out.push_str("---\n\n");
    out
}

#[derive(Default)]
struct FrontmatterEntry {
    key: Option<String>,
    lines: Vec<String>,
}

impl FrontmatterEntry {
    fn scalar_value(&self) -> Option<&str> {
        split_key_value(self.lines.first()?.trim()).map(|(_, value)| value)
    }

    fn list_values(&self) -> Vec<String> {
        let Some((_, value)) = self
            .lines
            .first()
            .and_then(|line| split_key_value(line.trim()))
        else {
            return Vec::new();
        };
        parse_yaml_list(value, &self.lines[1..])
    }
}

fn parse_frontmatter_entries(lines: Vec<&str>) -> Vec<FrontmatterEntry> {
    let mut entries = Vec::new();
    let mut current = FrontmatterEntry::default();
    for line in lines {
        let text = trim_line(line).trim_end().to_string();
        let key = top_level_key(&text);
        if key.is_some() && !current.lines.is_empty() {
            entries.push(current);
            current = FrontmatterEntry::default();
        }
        if current.lines.is_empty() {
            current.key = key.map(str::to_string);
        }
        current.lines.push(text);
    }
    if !current.lines.is_empty() {
        entries.push(current);
    }
    entries
}

fn top_level_key(line: &str) -> Option<&str> {
    if line.trim().is_empty() || line.starts_with([' ', '\t']) || line.trim_start().starts_with('#')
    {
        return None;
    }
    split_key_value(line).map(|(key, _)| key)
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let idx = line.find(':')?;
    Some((line[..idx].trim(), line[idx + 1..].trim()))
}

fn parse_yaml_list(value: &str, rest: &[String]) -> Vec<String> {
    let value = value.trim();
    if value.is_empty() {
        return rest
            .iter()
            .filter_map(|line| line.trim().strip_prefix("- "))
            .map(trim_yaml_scalar)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect();
    }
    if value.starts_with('[') && value.ends_with(']') {
        return parse_inline_yaml_list(value);
    }
    let value = trim_yaml_scalar(value).trim();
    if value.is_empty() {
        Vec::new()
    } else {
        vec![value.to_string()]
    }
}

fn parse_inline_yaml_list(value: &str) -> Vec<String> {
    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Vec::new();
    }
    split_quoted_commas(inner)
        .into_iter()
        .map(|item| trim_yaml_scalar(item.trim()).trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn split_quoted_commas(value: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut quote = None;
    for (idx, ch) in value.char_indices() {
        if quote.is_none() && matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if quote == Some(ch) {
            quote = None;
        } else if quote.is_none() && ch == ',' {
            out.push(&value[start..idx]);
            start = idx + 1;
        }
    }
    out.push(&value[start..]);
    out
}

fn append_yaml_list(out: &mut String, key: &str, values: Vec<String>) {
    let mut seen = Vec::<String>::new();
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !seen.iter().any(|v| v == value) {
            seen.push(value.to_string());
        }
    }
    if seen.is_empty() {
        return;
    }
    out.push_str(key);
    out.push_str(":\n");
    for value in seen {
        out.push_str("  - ");
        out.push_str(&format_yaml_scalar(&value));
        out.push('\n');
    }
}

fn trim_yaml_scalar(value: &str) -> &str {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn format_yaml_scalar(value: &str) -> String {
    if value.is_empty()
        || value.starts_with([
            '-', '{', '}', '[', ']', '&', '*', '#', '!', '|', '>', '@', '`',
        ])
        || value.contains(": ")
        || value.contains(" #")
        || value.contains(['[', ']', ','])
    {
        format!("{value:?}")
    } else {
        value.to_string()
    }
}

fn format_body(content: &str) -> String {
    let mut lines = Vec::new();
    let mut in_fence = false;
    let mut fence_marker = '\0';
    let mut fence_width = 0usize;
    let mut after_heading = false;
    for (line, _, _) in scan_lines(content) {
        let raw = trim_line(line);
        if let Some((marker, width)) = fence_start(raw.trim()) {
            push_body_line(&mut lines, raw.to_string(), &mut after_heading);
            (in_fence, fence_marker, fence_width) =
                next_fence_state(in_fence, fence_marker, fence_width, marker, width);
            continue;
        }
        if in_fence {
            lines.push(raw.to_string());
            continue;
        }
        let formatted = format_body_line(raw.trim_end());
        if formatted.trim().is_empty() {
            push_blank(&mut lines);
        } else {
            push_body_line(&mut lines, formatted, &mut after_heading);
        }
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn push_body_line(lines: &mut Vec<String>, line: String, after_heading: &mut bool) {
    let heading = is_heading(&line);
    if ((heading && lines.last().is_some_and(|last| !last.is_empty()))
        || (*after_heading && lines.last().is_some_and(|last| !last.is_empty())))
        && !lines.is_empty()
    {
        lines.push(String::new());
    }
    lines.push(line);
    *after_heading = heading;
}

fn push_blank(lines: &mut Vec<String>) {
    if !lines.is_empty() && lines.last().is_some_and(|line| !line.is_empty()) {
        lines.push(String::new());
    }
}

fn format_body_line(line: &str) -> String {
    let mut line = normalize_heading(line).unwrap_or_else(|| normalize_list_marker(line));
    if !line.contains('`') {
        line = format_wiki_links(&format_markdown_links(&line));
    }
    line
}

fn normalize_heading(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes)
        || trimmed.len() == hashes
        || !trimmed.as_bytes()[hashes].is_ascii_whitespace()
    {
        return None;
    }
    let title = trimmed[hashes..].trim().trim_end_matches('#').trim();
    Some(format!("{indent}{} {title}", "#".repeat(hashes)))
}

fn normalize_list_marker(line: &str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    if trimmed.len() >= 2
        && matches!(trimmed.as_bytes()[0], b'*' | b'+')
        && trimmed.as_bytes()[1].is_ascii_whitespace()
    {
        format!("{indent}- {}", trimmed[2..].trim_start())
    } else {
        line.to_string()
    }
}

fn is_heading(line: &str) -> bool {
    normalize_heading(line).is_some()
}

fn format_wiki_links(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let mut last = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = crate::text::find_bytes(bytes, i + 2, b"]]") {
                out.push_str(&line[last..i]);
                out.push_str("[[");
                out.push_str(&format_wiki_link_inner(&line[i + 2..end]));
                out.push_str("]]");
                i = end + 2;
                last = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&line[last..]);
    out
}

fn format_wiki_link_inner(inner: &str) -> String {
    let (target, alias, has_alias, escaped) = split_wiki_link_parts(inner);
    if escaped {
        return inner.trim().to_string();
    }
    let target = normalize_wiki_target(target);
    let alias = alias.trim();
    if has_alias && !alias.is_empty() {
        format!("{target}|{alias}")
    } else {
        target
    }
}

fn normalize_wiki_target(target: &str) -> String {
    let target = target.trim();
    if let Some((base, suffix)) = target.split_once('#') {
        format!("{}#{}", base.trim(), suffix.trim())
    } else {
        target.to_string()
    }
}

fn format_markdown_links(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let mut last = 0;
    while i < bytes.len() {
        if bytes[i] == b'!' {
            i += 1;
            continue;
        }
        if bytes[i] == b'[' {
            if let Some(label_end) = crate::text::find_byte(bytes, i + 1, b']') {
                if label_end + 1 < bytes.len() && bytes[label_end + 1] == b'(' {
                    if let Some(dest_end) = crate::text::find_byte(bytes, label_end + 2, b')') {
                        out.push_str(&line[last..i]);
                        out.push('[');
                        out.push_str(line[i + 1..label_end].trim());
                        out.push_str("](");
                        out.push_str(line[label_end + 2..dest_end].trim());
                        out.push(')');
                        i = dest_end + 1;
                        last = i;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    out.push_str(&line[last..]);
    out
}

fn ensure_final_newline(mut content: String) -> String {
    while content.ends_with('\n') {
        content.pop();
    }
    content.push('\n');
    content
}
