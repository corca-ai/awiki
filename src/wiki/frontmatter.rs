use super::FrontMatter;
use crate::text::{fence_start, normalize_preview_line, scan_lines, starts_indented, trim_line};

pub(crate) fn parse_front_matter(content: &str) -> FrontMatter {
    let lines = scan_lines(content);
    if lines.first().map(|l| trim_line(l.0)) != Some("---") {
        return FrontMatter::default();
    }
    let Some(end_idx) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| trim_line(l.0) == "---")
        .map(|(i, _)| i)
    else {
        return FrontMatter::default();
    };
    let mut fm = FrontMatter {
        present: true,
        body_offset: lines[end_idx].2,
        ..Default::default()
    };
    let mut i = 1;
    while i < end_idx {
        let line = trim_line(lines[i].0);
        if line.is_empty() || starts_indented(line) {
            i += 1;
            continue;
        }
        if let Some((key, value)) = split_key_value(line) {
            match key {
                "title" => fm.title = trim_yaml_scalar(value).to_string(),
                "aliases" => {
                    let (aliases, next) = parse_aliases(&lines, i, end_idx, value);
                    for alias in aliases {
                        if !alias.is_empty() && !fm.aliases.contains(&alias) {
                            fm.aliases.push(alias);
                        }
                    }
                    i = next;
                }
                _ => {}
            }
        }
        i += 1;
    }
    fm
}

fn parse_aliases(
    lines: &[(&str, usize, usize)],
    start: usize,
    end: usize,
    value: &str,
) -> (Vec<String>, usize) {
    let trimmed = value.trim();
    let mut next = start;
    if trimmed.is_empty() {
        let mut aliases = Vec::new();
        for (i, line) in lines.iter().enumerate().take(end).skip(start + 1) {
            let text = trim_line(line.0);
            if text.is_empty() {
                next = i;
                continue;
            }
            if !starts_indented(text) {
                break;
            }
            let item = text.trim();
            if let Some(rest) = item.strip_prefix("- ") {
                aliases.push(trim_yaml_scalar(rest.trim()).to_string());
            }
            next = i;
        }
        return (aliases, next);
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return (parse_inline_list(trimmed), next);
    }
    (vec![trim_yaml_scalar(trimmed).to_string()], next)
}

fn parse_inline_list(value: &str) -> Vec<String> {
    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Vec::new();
    }
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in inner.chars() {
        if quote.is_none() && (ch == '\'' || ch == '"') {
            quote = Some(ch);
            current.push(ch);
        } else if quote == Some(ch) {
            quote = None;
            current.push(ch);
        } else if quote.is_none() && ch == ',' {
            items.push(trim_yaml_scalar(current.trim()).to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        items.push(trim_yaml_scalar(current.trim()).to_string());
    }
    items
}

pub(crate) fn first_preview_line(content: &str) -> String {
    let fm = parse_front_matter(content);
    let mut in_fence = false;
    let mut fence_marker = '\0';
    let mut fence_width = 0usize;
    for (line, _, _) in scan_lines(&content[fm.body_offset..]) {
        let trimmed = trim_line(line).trim();
        if let Some((marker, width)) = fence_start(trimmed) {
            if in_fence && marker == fence_marker && width >= fence_width {
                in_fence = false;
            } else if !in_fence {
                in_fence = true;
                fence_marker = marker;
                fence_width = width;
            }
            continue;
        }
        if in_fence || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        return normalize_preview_line(trimmed);
    }
    String::new()
}

pub(crate) fn update_front_matter_title(
    content: &str,
    old_title: &str,
    new_title: &str,
) -> (String, bool) {
    let lines = scan_lines(content);
    if lines.first().map(|l| trim_line(l.0)) != Some("---") {
        return (content.to_string(), false);
    }
    let Some(end_idx) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| trim_line(l.0) == "---")
        .map(|(i, _)| i)
    else {
        return (content.to_string(), false);
    };
    let mut out = String::new();
    let mut changed = false;
    for (i, (line, _, _)) in lines.iter().enumerate() {
        if i > 0 && i < end_idx && !starts_indented(trim_line(line)) {
            if let Some((key, value)) = split_key_value(trim_line(line)) {
                if key == "title" && trim_yaml_scalar(value) == old_title {
                    out.push_str(&replace_line_text(
                        line,
                        &format!("title: {}", format_scalar_like(value, new_title)),
                    ));
                    changed = true;
                    continue;
                }
            }
        }
        out.push_str(line);
    }
    if changed {
        (out, true)
    } else {
        (content.to_string(), false)
    }
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let idx = line.find(':')?;
    Some((line[..idx].trim(), line[idx + 1..].trim()))
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

fn format_scalar_like(original: &str, replacement: &str) -> String {
    let original = original.trim();
    if original.len() >= 2 && original.starts_with('"') && original.ends_with('"') {
        format!("\"{replacement}\"")
    } else if original.len() >= 2 && original.starts_with('\'') && original.ends_with('\'') {
        format!("'{replacement}'")
    } else {
        replacement.to_string()
    }
}

fn replace_line_text(original: &str, replacement: &str) -> String {
    if original.ends_with("\r\n") {
        format!("{replacement}\r\n")
    } else if original.ends_with('\n') {
        format!("{replacement}\n")
    } else {
        replacement.to_string()
    }
}
