use rustc_hash::FxHashSet;
use std::fs;

use super::{RenameResult, Vault};
use crate::{
    text::{
        fence_start, find_byte, find_bytes, mask_inline_code, next_fence_state, scan_lines,
        trim_line,
    },
    wiki::{
        frontmatter::{parse_front_matter, update_front_matter_title},
        links::{
            parse_markdown_target, split_target_suffix, split_wiki_link_parts, wrap_wiki_link,
        },
        path::{clean_rel_path, dir_segment, document_key, join_slash, normalize_document_name},
    },
};

impl Vault {
    pub(crate) fn rename(&mut self, old_id: &str, new_id: &str) -> Result<RenameResult, String> {
        let doc_idx = self.resolve_document(old_id)?;
        let old_name = if self.recursive {
            self.documents[doc_idx].rel_path.clone()
        } else {
            self.documents[doc_idx].name.clone()
        };
        let new_base = validate_rename_target(new_id)?;
        let new_name = if self.recursive {
            if new_id.contains('/') {
                clean_rel_path(new_id).trim_end_matches(".md").to_string()
            } else {
                let dir = dir_segment(&self.documents[doc_idx].rel_path);
                clean_rel_path(&join_slash(&dir, &new_base))
            }
        } else {
            new_base.clone()
        };
        let old_path = self.documents[doc_idx].path.clone();
        let new_path = if self.recursive {
            self.root.join(format!("{new_name}.md"))
        } else {
            self.root.join(format!("{new_base}.md"))
        };
        if new_path.exists() && new_path != old_path {
            return Err(format!("document {new_name:?} already exists"));
        }
        let old_key = document_key(&self.documents[doc_idx].name);
        let mut updated = Vec::new();
        let mut links_updated = 0usize;
        let mut title_updated = false;
        for doc in &self.documents {
            let content = fs::read_to_string(&doc.path).map_err(|e| e.to_string())?;
            let (mut rewritten, count) = rewrite_document_links_flat(&content, &old_key, &new_base);
            let changed_title = if doc.key == self.documents[doc_idx].key {
                let (next, changed) =
                    update_front_matter_title(&rewritten, &self.documents[doc_idx].name, &new_base);
                rewritten = next;
                changed
            } else {
                false
            };
            if count > 0 || changed_title {
                links_updated += count;
                title_updated |= changed_title;
                updated.push((doc.path.clone(), rewritten));
            }
        }
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::rename(&old_path, &new_path).map_err(|e| e.to_string())?;
        for (path, content) in &mut updated {
            if *path == old_path {
                *path = new_path.clone();
            }
            fs::write(path, content).map_err(|e| e.to_string())?;
        }
        let files_touched = updated
            .iter()
            .map(|(p, _)| p)
            .collect::<FxHashSet<_>>()
            .len()
            .max(1);
        Ok(RenameResult {
            old_name,
            new_name,
            files_touched,
            links_updated,
            title_updated,
        })
    }
}

fn rewrite_document_links_flat(content: &str, old_key: &str, new_name: &str) -> (String, usize) {
    let mut changes = 0;
    let mut out = String::new();
    let fm = parse_front_matter(content);
    let mut in_fence = false;
    let mut fence_marker = '\0';
    let mut fence_width = 0usize;
    for (line, _, end) in scan_lines(content) {
        let body = end > fm.body_offset;
        let trimmed = trim_line(line).trim();
        if body {
            if let Some((marker, width)) = fence_start(trimmed) {
                (in_fence, fence_marker, fence_width) =
                    next_fence_state(in_fence, fence_marker, fence_width, marker, width);
                out.push_str(line);
                continue;
            }
            if in_fence {
                out.push_str(line);
                continue;
            }
        }
        let (line, c1) = rewrite_wiki_line(line, old_key, new_name);
        let (line, c2) = rewrite_markdown_line(&line, old_key, new_name);
        changes += c1 + c2;
        out.push_str(&line);
    }
    (out, changes)
}

fn rewrite_wiki_line(line: &str, old_key: &str, new_name: &str) -> (String, usize) {
    let masked = mask_inline_code(line);
    let bytes = masked.as_bytes();
    let mut out = String::new();
    let mut last = 0;
    let mut changes = 0;
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = find_bytes(bytes, i + 2, b"]]") {
                let full = &line[i..end + 2];
                let inner = &line[i + 2..end];
                let (target, label, has_label, escaped) = split_wiki_link_parts(inner);
                let (base, suffix) = split_target_suffix(target.trim());
                if document_key(base) == old_key {
                    out.push_str(&line[last..i]);
                    let mut new_target = new_name.to_string();
                    if base.trim().to_ascii_lowercase().ends_with(".md") {
                        new_target.push_str(".md");
                    }
                    new_target.push_str(suffix);
                    out.push_str(&wrap_wiki_link(&new_target, label, has_label, escaped));
                    last = end + 2;
                    changes += 1;
                } else {
                    let _ = full;
                }
                i = end + 2;
                continue;
            }
        }
        i += 1;
    }
    if changes == 0 {
        return (line.to_string(), 0);
    }
    out.push_str(&line[last..]);
    (out, changes)
}

fn rewrite_markdown_line(line: &str, old_key: &str, new_name: &str) -> (String, usize) {
    let masked = mask_inline_code(line);
    let bytes = masked.as_bytes();
    let mut out = String::new();
    let mut last = 0;
    let mut changes = 0;
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
                        let dest = &line[label_end + 2..dest_end];
                        if let Some(target) = parse_markdown_target(dest) {
                            if document_key(target) == old_key {
                                out.push_str(&line[last..label_end + 2]);
                                out.push_str(&replace_path_base(target, new_name));
                                out.push_str(&line[label_end + 2 + target.len()..dest_end + 1]);
                                last = dest_end + 1;
                                changes += 1;
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
    if changes == 0 {
        return (line.to_string(), 0);
    }
    out.push_str(&line[last..]);
    (out, changes)
}

fn replace_path_base(raw_path: &str, new_name: &str) -> String {
    let (base, suffix) = split_target_suffix(raw_path);
    let (dir, file) = base
        .rsplit_once('/')
        .map(|(d, f)| (format!("{d}/"), f))
        .unwrap_or((String::new(), base));
    let mut replacement = new_name.to_string();
    if file.to_ascii_lowercase().ends_with(".md") {
        replacement.push_str(".md");
    }
    format!("{dir}{replacement}{suffix}")
}

fn validate_rename_target(value: &str) -> Result<String, String> {
    let name = normalize_document_name(value);
    if name.is_empty() {
        return Err("new document name must not be empty".to_string());
    }
    if name
        .chars()
        .any(|c| matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return Err(format!(
            "new document name {name:?} contains invalid path characters"
        ));
    }
    Ok(name)
}
