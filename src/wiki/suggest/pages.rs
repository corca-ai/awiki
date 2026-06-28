use rayon::prelude::*;
use std::fs;

use crate::text::{fence_start, next_fence_state, normalize_preview_line, trim_line};
use crate::wiki::Vault;

use super::{PageLengthHit, PageLengthSuggestion, SuggestOptions};

pub(super) struct PageStats {
    pub(super) doc: usize,
    pub(super) visible_lines: usize,
    pub(super) words: usize,
    pub(super) norm: String,
}

pub(super) fn collect_page_stats(vault: &Vault) -> Result<Vec<PageStats>, String> {
    vault
        .documents
        .par_iter()
        .enumerate()
        .map(|(doc, d)| {
            let content =
                fs::read_to_string(&d.path).map_err(|e| format!("{}: {e}", d.path.display()))?;
            Ok(page_stats_for(doc, &content, d.front_matter.body_offset))
        })
        .collect()
}

pub(super) fn long_pages(stats: &[PageStats], opts: &SuggestOptions) -> PageLengthSuggestion {
    let mut pages: Vec<_> = stats
        .iter()
        .filter(|s| s.visible_lines >= opts.long_lines || s.words >= opts.long_words)
        .map(|s| PageLengthHit {
            doc: s.doc,
            lines: s.visible_lines,
            words: s.words,
        })
        .collect();
    let hit_count = pages.len();
    pages.sort_by(|a, b| b.words.cmp(&a.words).then(b.lines.cmp(&a.lines)));
    pages.truncate(opts.limit);
    PageLengthSuggestion {
        line_threshold: Some(opts.long_lines),
        word_threshold: opts.long_words,
        rate: ratio_count(hit_count, stats.len()),
        pages,
    }
}

pub(super) fn short_stubs(stats: &[PageStats], opts: &SuggestOptions) -> PageLengthSuggestion {
    let mut pages: Vec<_> = stats
        .iter()
        .filter(|s| s.words > 0 && s.words <= opts.short_words)
        .map(|s| PageLengthHit {
            doc: s.doc,
            lines: s.visible_lines,
            words: s.words,
        })
        .collect();
    let hit_count = pages.len();
    pages.sort_by(|a, b| a.words.cmp(&b.words).then(a.lines.cmp(&b.lines)));
    pages.truncate(opts.limit);
    PageLengthSuggestion {
        line_threshold: None,
        word_threshold: opts.short_words,
        rate: ratio_count(hit_count, stats.len()),
        pages,
    }
}

fn page_stats_for(doc: usize, content: &str, body_offset: usize) -> PageStats {
    let visible = visible_markdown_text(&content[body_offset..]);
    PageStats {
        doc,
        visible_lines: visible
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        words: visible.split_whitespace().count(),
        norm: normalize_duplicate_text(&visible),
    }
}

fn visible_markdown_text(content: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    let mut fence_marker = '\0';
    let mut fence_width = 0usize;
    for (line, _, _) in crate::text::scan_lines(content) {
        let trimmed = trim_line(line).trim();
        if let Some((marker, width)) = fence_start(trimmed) {
            (in_fence, fence_marker, fence_width) =
                next_fence_state(in_fence, fence_marker, fence_width, marker, width);
            continue;
        }
        if in_fence || trimmed.is_empty() {
            continue;
        }
        out.push_str(&normalize_preview_line(trimmed));
        out.push('\n');
    }
    out
}

fn normalize_duplicate_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_code = false;
    for ch in value.chars() {
        match ch {
            '`' => in_code = !in_code,
            _ if in_code => {}
            '[' | ']' | '(' | ')' | '*' | '_' | '#' | '>' | '!' | '|' | '-' => out.push(' '),
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            c if c.is_whitespace() => out.push(' '),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn ratio_count(count: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        count as f64 / total as f64
    }
}
