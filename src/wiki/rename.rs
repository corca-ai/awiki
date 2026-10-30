use rustc_hash::FxHashSet;
use std::fs;

use super::{LinkKind, RenameResult, Vault};
use crate::{
    text::{
        fence_start, find_byte, find_bytes, lower, mask_inline_code, next_fence_state, scan_lines,
        trim_line,
    },
    wiki::{
        frontmatter::{parse_front_matter, update_front_matter_title},
        links::{
            parse_markdown_target, split_target_suffix, split_wiki_link_parts, wrap_wiki_link,
        },
        path::{
            clean_rel_path, dir_segment, document_key, join_slash, last_segment,
            normalize_document_name,
        },
    },
};

/// What a rename rewrites links to.
struct RenameTarget {
    doc: usize,
    old_key: String,
    new_base: String,
    new_rel: String,
    /// Whether a bare `[[new_base]]` resolves to the renamed document afterwards.
    bare_resolves: bool,
}

/// How one link names its target, which decides how the new target is written.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetForm {
    Bare,
    Absolute,
    Relative,
}

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
        let old_base = last_segment(&self.documents[doc_idx].rel_path);
        let target = RenameTarget {
            doc: doc_idx,
            old_key: document_key(&old_base),
            bare_resolves: self.bare_name_resolves_to(doc_idx, &new_name),
            new_base,
            new_rel: new_name.clone(),
        };
        let mut updated = Vec::new();
        let mut links_updated = 0usize;
        let mut title_updated = false;
        for (source, doc) in self.documents.iter().enumerate() {
            let content = fs::read_to_string(&doc.path).map_err(|e| e.to_string())?;
            let (mut rewritten, count) = self.rewrite_document_links(&content, source, &target);
            let changed_title = if source == doc_idx {
                let (next, changed) =
                    update_front_matter_title(&rewritten, &old_base, &target.new_base);
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

    /// Whether, once renamed to `new_rel`, the document wins a bare-name lookup:
    /// the shortest path, then the first path, among documents with its basename.
    fn bare_name_resolves_to(&self, doc_idx: usize, new_rel: &str) -> bool {
        let key = document_key(&last_segment(new_rel));
        let rank = |rel: &str| (rel.matches('/').count(), lower(rel));
        let renamed = rank(new_rel);
        self.documents.iter().enumerate().all(|(idx, doc)| {
            idx == doc_idx
                || document_key(&last_segment(&doc.rel_path)) != key
                || rank(&doc.rel_path) > renamed
        })
    }

    /// Whether a link written in `source` points at the renamed document.
    fn links_to(
        &self,
        source: usize,
        kind: LinkKind,
        raw_target: &str,
        target: &RenameTarget,
    ) -> bool {
        let key = document_key(raw_target);
        if !self.recursive {
            return key == target.old_key;
        }
        let source_dir = dir_segment(&self.documents[source].rel_path);
        self.resolve_recursive(&source_dir, kind, raw_target, &key) == Some(target.doc)
    }

    /// The new text for a link base that pointed at the renamed document.
    fn new_link_base(&self, source: usize, form: TargetForm, target: &RenameTarget) -> String {
        if !self.recursive {
            return target.new_base.clone();
        }
        match form {
            TargetForm::Bare if target.bare_resolves => target.new_base.clone(),
            TargetForm::Bare | TargetForm::Absolute => target.new_rel.clone(),
            TargetForm::Relative => {
                let source_dir = dir_segment(&self.documents[source].rel_path);
                relative_path(&source_dir, &target.new_rel)
            }
        }
    }

    fn rewrite_document_links(
        &self,
        content: &str,
        source: usize,
        target: &RenameTarget,
    ) -> (String, usize) {
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
            let (line, c1) = self.rewrite_wiki_line(line, source, target);
            let (line, c2) = self.rewrite_markdown_line(&line, source, target);
            changes += c1 + c2;
            out.push_str(&line);
        }
        (out, changes)
    }

    fn rewrite_wiki_line(
        &self,
        line: &str,
        source: usize,
        target: &RenameTarget,
    ) -> (String, usize) {
        let masked = mask_inline_code(line);
        let bytes = masked.as_bytes();
        let mut out = String::new();
        let mut last = 0;
        let mut changes = 0;
        let mut i = 0;
        while i + 3 < bytes.len() {
            if bytes[i] == b'[' && bytes[i + 1] == b'[' {
                if let Some(end) = find_bytes(bytes, i + 2, b"]]") {
                    let inner = &line[i + 2..end];
                    let (link_target, label, has_label, escaped) = split_wiki_link_parts(inner);
                    let (base, suffix) = split_target_suffix(link_target.trim());
                    let raw = base.trim().trim_matches(['<', '>']).trim();
                    if self.links_to(source, LinkKind::Wiki, raw, target) {
                        let form = if raw.starts_with("./") || raw.starts_with("../") {
                            TargetForm::Relative
                        } else if raw.contains('/') || raw.contains('\\') {
                            TargetForm::Absolute
                        } else {
                            TargetForm::Bare
                        };
                        let mut new_target = self.new_link_base(source, form, target);
                        if self.recursive
                            && form == TargetForm::Relative
                            && !new_target.starts_with("../")
                        {
                            new_target.insert_str(0, "./");
                        }
                        if base.trim().to_ascii_lowercase().ends_with(".md") {
                            new_target.push_str(".md");
                        }
                        new_target.push_str(suffix);
                        out.push_str(&line[last..i]);
                        out.push_str(&wrap_wiki_link(&new_target, label, has_label, escaped));
                        last = end + 2;
                        changes += 1;
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

    fn rewrite_markdown_line(
        &self,
        line: &str,
        source: usize,
        target: &RenameTarget,
    ) -> (String, usize) {
        let masked = mask_inline_code(line);
        let bytes = masked.as_bytes();
        let mut out = String::new();
        let mut last = 0;
        let mut changes = 0;
        let mut i = 0;
        while i < bytes.len() {
            // Images are not document links: skip `![...](...)` whole.
            let image = bytes[i] == b'!' && i + 1 < bytes.len() && bytes[i + 1] == b'[';
            let start = if image { i + 1 } else { i };
            let Some((label_end, dest_end)) = markdown_link_at(bytes, start) else {
                i += 1;
                continue;
            };
            let dest = &line[label_end + 2..dest_end];
            if let Some(link_target) = parse_markdown_target(dest).filter(|_| !image) {
                let (base, suffix) = split_target_suffix(link_target);
                if self.links_to(source, LinkKind::Markdown, base.trim(), target) {
                    // The target is a slice of `dest`; splice at its own offset,
                    // which leading blanks or `<` move away from the `(`.
                    let offset = link_target.as_ptr() as usize - dest.as_ptr() as usize;
                    let target_start = label_end + 2 + offset;
                    out.push_str(&line[last..target_start]);
                    out.push_str(&self.new_markdown_target(source, base, suffix, target));
                    out.push_str(&line[target_start + link_target.len()..dest_end + 1]);
                    last = dest_end + 1;
                    changes += 1;
                }
            }
            i = dest_end + 1;
        }
        if changes == 0 {
            return (line.to_string(), 0);
        }
        out.push_str(&line[last..]);
        (out, changes)
    }

    fn new_markdown_target(
        &self,
        source: usize,
        base: &str,
        suffix: &str,
        target: &RenameTarget,
    ) -> String {
        if !self.recursive {
            return format!("{}{suffix}", replace_path_base(base, &target.new_base));
        }
        let mut path = relative_path(
            &dir_segment(&self.documents[source].rel_path),
            &target.new_rel,
        );
        if base.starts_with("./") && !path.starts_with("../") {
            path.insert_str(0, "./");
        }
        if base.to_ascii_lowercase().ends_with(".md") {
            path.push_str(".md");
        }
        format!("{path}{suffix}")
    }
}

/// `(label_end, dest_end)` of a `[label](dest)` starting at `start`.
fn markdown_link_at(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let label_end = find_byte(bytes, start + 1, b']')?;
    if bytes.get(label_end + 1) != Some(&b'(') {
        return None;
    }
    let dest_end = find_byte(bytes, label_end + 2, b')')?;
    Some((label_end, dest_end))
}

/// `to` (a repo-relative path) written relative to the directory `from_dir`.
fn relative_path(from_dir: &str, to: &str) -> String {
    let from: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    let to: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let directories = to.len().saturating_sub(1);
    let common = from
        .iter()
        .zip(&to[..directories])
        .take_while(|(a, b)| lower(a) == lower(b))
        .count();
    let mut parts = vec![".."; from.len() - common];
    parts.extend(&to[common..]);
    parts.join("/")
}

fn replace_path_base(raw_path: &str, new_name: &str) -> String {
    let (dir, file) = raw_path
        .rsplit_once('/')
        .map(|(d, f)| (format!("{d}/"), f))
        .unwrap_or((String::new(), raw_path));
    let mut replacement = new_name.to_string();
    if file.to_ascii_lowercase().ends_with(".md") {
        replacement.push_str(".md");
    }
    format!("{dir}{replacement}")
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
