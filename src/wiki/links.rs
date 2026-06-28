use super::{Link, LinkKind, LinkOnlyLine};
use crate::{
    text::{
        contains_letter_or_digit, fence_start, find_byte, find_bytes, mask_inline_code,
        next_fence_state, normalize_preview_line, scan_lines, strip_line_only_markdown, trim_line,
    },
    wiki::{
        frontmatter::parse_front_matter,
        path::{document_key, normalize_document_name},
    },
};

pub(crate) fn parse_links(content: &str) -> Vec<Link> {
    let fm = parse_front_matter(content);
    let mut links = Vec::new();
    if fm.present {
        for (line, _, _) in scan_lines(&content[..fm.body_offset]) {
            let context = normalize_preview_line(trim_line(line));
            let mut parsed = parse_links_in_line(line);
            for link in &mut parsed {
                link.context = context.clone();
            }
            links.extend(parsed);
        }
    }
    let mut in_fence = false;
    let mut fence_marker = '\0';
    let mut fence_width = 0usize;
    for (line, _, _) in scan_lines(&content[fm.body_offset..]) {
        let trimmed = trim_line(line).trim();
        if let Some((marker, width)) = fence_start(trimmed) {
            (in_fence, fence_marker, fence_width) =
                next_fence_state(in_fence, fence_marker, fence_width, marker, width);
            continue;
        }
        if in_fence || trimmed.is_empty() {
            continue;
        }
        let context = normalize_preview_line(trimmed);
        let mut parsed = parse_links_in_line(line);
        for link in &mut parsed {
            link.context = context.clone();
        }
        links.extend(parsed);
    }
    links
}

fn parse_links_in_line(line: &str) -> Vec<Link> {
    let masked = mask_inline_code(line);
    let mut links = parse_wiki_links(&masked);
    links.extend(parse_markdown_links(&masked));
    links
}

fn parse_wiki_links(line: &str) -> Vec<Link> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = find_bytes(bytes, i + 2, b"]]") {
                let inner = &line[i + 2..end];
                let (target, _, _, _) = split_wiki_link_parts(inner);
                let target = target.trim();
                let (base, suffix) = split_target_suffix(target);
                let key = document_key(base);
                if !key.is_empty() {
                    out.push(Link {
                        kind: LinkKind::Wiki,
                        display_target: format!("{}{}", display_target(base), suffix),
                        target_key: key,
                        raw_target: raw_link_target(base),
                        resolved: None,
                        context: String::new(),
                    });
                }
                i = end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn parse_markdown_links(line: &str) -> Vec<Link> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let image = bytes[i] == b'!' && i + 1 < bytes.len() && bytes[i + 1] == b'[';
        let link_start = if image { i + 1 } else { i };
        if link_start < bytes.len() && bytes[link_start] == b'[' {
            if let Some(label_end) = find_byte(bytes, link_start + 1, b']') {
                if label_end + 1 < bytes.len() && bytes[label_end + 1] == b'(' {
                    if let Some(dest_end) = find_byte(bytes, label_end + 2, b')') {
                        if !image {
                            let dest = &line[label_end + 2..dest_end];
                            if let Some(target) = parse_markdown_target(dest) {
                                let (base, suffix) = split_target_suffix(target);
                                let key = document_key(base);
                                if !key.is_empty() {
                                    out.push(Link {
                                        kind: LinkKind::Markdown,
                                        display_target: format!(
                                            "{}{}",
                                            display_target(base),
                                            suffix
                                        ),
                                        target_key: key,
                                        raw_target: raw_link_target(base),
                                        resolved: None,
                                        context: String::new(),
                                    });
                                }
                            }
                        }
                        i = dest_end + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    out
}

pub(crate) fn find_link_only_lines(content: &str) -> Vec<LinkOnlyLine> {
    let fm = parse_front_matter(content);
    let mut issues = Vec::new();
    let mut in_fence = false;
    let mut fence_marker = '\0';
    let mut fence_width = 0usize;
    for (line_no, (line, _, end)) in scan_lines(content).into_iter().enumerate() {
        if end <= fm.body_offset {
            continue;
        }
        let trimmed = trim_line(line).trim();
        if let Some((marker, width)) = fence_start(trimmed) {
            (in_fence, fence_marker, fence_width) =
                next_fence_state(in_fence, fence_marker, fence_width, marker, width);
            continue;
        }
        if in_fence || trimmed.is_empty() {
            continue;
        }
        if is_link_only_line(trimmed) {
            issues.push(LinkOnlyLine {
                line: line_no + 1,
                text: trimmed.to_string(),
            });
        }
    }
    issues
}

fn is_link_only_line(line: &str) -> bool {
    let spans = document_link_spans(line);
    if spans.len() != 1 {
        return false;
    }
    let (start, end) = spans[0];
    let mut rest = String::with_capacity(line.len());
    rest.push_str(&line[..start]);
    rest.extend(std::iter::repeat_n(' ', end - start));
    rest.push_str(&line[end..]);
    !contains_letter_or_digit(&strip_line_only_markdown(&rest))
}

fn document_link_spans(line: &str) -> Vec<(usize, usize)> {
    let masked = mask_inline_code(line);
    let mut spans = Vec::new();
    let bytes = masked.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = find_bytes(bytes, i + 2, b"]]") {
                let (target, _, _, _) = split_wiki_link_parts(&masked[i + 2..end]);
                let (base, _) = split_target_suffix(target.trim());
                if !document_key(base).is_empty() {
                    spans.push((i, end + 2));
                }
                i = end + 2;
                continue;
            }
        }
        i += 1;
    }
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'!' {
            i += 1;
            continue;
        }
        if bytes[i] == b'[' {
            if let Some(label_end) = find_byte(bytes, i + 1, b']') {
                if label_end + 1 < bytes.len() && bytes[label_end + 1] == b'(' {
                    if let Some(dest_end) = find_byte(bytes, label_end + 2, b')') {
                        if let Some(target) =
                            parse_markdown_target(&masked[label_end + 2..dest_end])
                        {
                            let (base, _) = split_target_suffix(target);
                            if !document_key(base).is_empty() {
                                spans.push((i, dest_end + 1));
                            }
                        }
                        i = dest_end + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    spans.sort_unstable();
    spans
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

pub(crate) fn parse_markdown_target(dest: &str) -> Option<&str> {
    let dest = dest.trim_start_matches([' ', '\t']);
    if dest.is_empty() {
        return None;
    }
    let target = if let Some(rest) = dest.strip_prefix('<') {
        let end = rest.find('>')?;
        &rest[..end]
    } else {
        dest.split([' ', '\t']).next().unwrap_or("")
    }
    .trim();
    if target.is_empty()
        || target.starts_with('#')
        || target.contains("://")
        || target.starts_with("mailto:")
    {
        None
    } else {
        Some(target)
    }
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

pub(crate) fn wrap_wiki_link(
    target: &str,
    label: &str,
    has_label: bool,
    escaped_separator: bool,
) -> String {
    if !has_label {
        format!("[[{target}]]")
    } else if escaped_separator {
        format!("[[{target}\\|{label}]]")
    } else {
        format!("[[{target}|{label}]]")
    }
}
