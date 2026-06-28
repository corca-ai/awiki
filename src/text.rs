pub(crate) fn scan_lines(content: &str) -> Vec<(&str, usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = content.as_bytes();
    while start < bytes.len() {
        let mut end = start;
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }
        if end < bytes.len() {
            end += 1;
        }
        lines.push((&content[start..end], start, end));
        start = end;
    }
    lines
}

pub(crate) fn trim_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}

pub(crate) fn normalize_preview_line(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn mask_inline_code(line: &str) -> String {
    if !line.contains('`') {
        return line.to_string();
    }
    let mut bytes = line.as_bytes().to_vec();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let content_start = backtick_run_end(&bytes, i);
        let width = content_start - i;
        if let Some((close_start, close_end)) = find_closing_run(&bytes, content_start, width) {
            for b in bytes.iter_mut().take(close_start).skip(content_start) {
                *b = b' ';
            }
            i = close_end;
        } else {
            i = content_start;
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| line.to_string())
}

fn backtick_run_end(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i] == b'`' {
        i += 1;
    }
    i
}

fn find_closing_run(bytes: &[u8], mut pos: usize, width: usize) -> Option<(usize, usize)> {
    while pos < bytes.len() {
        if bytes[pos] != b'`' {
            pos += 1;
            continue;
        }
        let start = pos;
        pos = backtick_run_end(bytes, pos);
        if pos - start == width {
            return Some((start, pos));
        }
    }
    None
}

pub(crate) fn fence_start(line: &str) -> Option<(char, usize)> {
    if line.starts_with("```") {
        Some(('`', line.bytes().take_while(|&b| b == b'`').count()))
    } else if line.starts_with("~~~") {
        Some(('~', line.bytes().take_while(|&b| b == b'~').count()))
    } else {
        None
    }
}

pub(crate) fn next_fence_state(
    in_fence: bool,
    fence_marker: char,
    fence_width: usize,
    marker: char,
    width: usize,
) -> (bool, char, usize) {
    if in_fence && marker == fence_marker && width >= fence_width {
        (false, fence_marker, fence_width)
    } else if !in_fence {
        (true, marker, width)
    } else {
        (in_fence, fence_marker, fence_width)
    }
}

pub(crate) fn starts_indented(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

pub(crate) fn lower(value: &str) -> String {
    value.to_lowercase()
}

pub(crate) fn ratio(count: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        count as f64 / total as f64
    }
}

pub(crate) fn truncate_runes(value: &str, limit: usize) -> String {
    let count = value.chars().count();
    if count <= limit {
        return value.to_string();
    }
    if limit <= 3 {
        return ".".repeat(limit);
    }
    value
        .chars()
        .take(limit - 3)
        .collect::<String>()
        .trim()
        .to_string()
        + "..."
}

pub(crate) fn find_byte(bytes: &[u8], start: usize, needle: u8) -> Option<usize> {
    bytes
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(i, &b)| (b == needle).then_some(i))
}

pub(crate) fn find_bytes(bytes: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    bytes[start..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + start)
}

pub(crate) fn strip_line_only_markdown(value: &str) -> String {
    let mut value = value.trim().to_string();
    loop {
        let before = value.clone();
        value = value.trim().to_string();
        if let Some(rest) = value.strip_prefix('>') {
            value = rest.trim().to_string();
        }
        if value.len() >= 2
            && matches!(value.as_bytes()[0], b'-' | b'*' | b'+')
            && value.as_bytes()[1].is_ascii_whitespace()
        {
            value = value[2..].trim().to_string();
        }
        if let Some(rest) = strip_ordered_list_prefix(&value) {
            value = rest.trim().to_string();
        }
        if value.len() >= 4
            && value.starts_with('[')
            && value.as_bytes()[2] == b']'
            && value.as_bytes()[3].is_ascii_whitespace()
            && matches!(value.as_bytes()[1], b' ' | b'x' | b'X')
        {
            value = value[4..].trim().to_string();
        }
        let hashes = value.bytes().take_while(|&b| b == b'#').count();
        if (1..=6).contains(&hashes)
            && (value.len() == hashes || value.as_bytes()[hashes].is_ascii_whitespace())
        {
            value = value[hashes..].trim().to_string();
        }
        if value == before.trim() {
            return value;
        }
    }
}

fn strip_ordered_list_prefix(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0
        && i + 1 < bytes.len()
        && (bytes[i] == b'.' || bytes[i] == b')')
        && bytes[i + 1].is_ascii_whitespace()
    {
        Some(&value[i + 2..])
    } else {
        None
    }
}

pub(crate) fn contains_letter_or_digit(value: &str) -> bool {
    value.chars().any(|c| c.is_alphanumeric())
}
