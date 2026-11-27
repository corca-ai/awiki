use super::{
    changes::{Change, ChangePlan},
    LinkKind, RenameResult, Vault,
};
use crate::{
    text::lower,
    wiki::{
        frontmatter::update_front_matter_title,
        links::split_target_suffix,
        markdown::{label_end, local_target, replace_ranges, write_destination},
        path::{
            clean_rel_path, dir_segment, document_key, document_path_key, join_slash, last_segment,
            normalize_document_name, resolve_target_rel,
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
        let (plan, report) = self.plan_rename(old_id, new_id)?;
        plan.apply()?;
        Ok(report)
    }

    pub(crate) fn plan_rename(
        &self,
        old_id: &str,
        new_id: &str,
    ) -> Result<(ChangePlan, RenameResult), String> {
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
        if std::path::Path::new(&new_name)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("new document path must stay inside wiki root".into());
        }
        let new_key = if self.recursive {
            document_path_key(&new_name)
        } else {
            document_key(&new_name)
        };
        if self
            .docs_by_key
            .get(&new_key)
            .is_some_and(|index| *index != doc_idx)
        {
            return Err(format!("document {new_name:?} already exists"));
        }
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
        let mut plan = ChangePlan::new(&self.root);
        let mut links_updated = 0usize;
        let mut title_updated = false;
        for (source, doc) in self.documents.iter().enumerate() {
            let content = &doc.content;
            let (mut rewritten, count) = self.rewrite_document_links(source, &target);
            let changed_title = if source == doc_idx {
                let (next, changed) =
                    update_front_matter_title(&rewritten, &old_base, &target.new_base);
                rewritten = next;
                changed
            } else {
                false
            };
            if count > 0 || changed_title || source == doc_idx {
                links_updated += count;
                title_updated |= changed_title;
                plan.changes.push(Change {
                    from: doc.path.clone(),
                    to: if source == doc_idx {
                        new_path.clone()
                    } else {
                        doc.path.clone()
                    },
                    before: content.clone(),
                    after: rewritten,
                });
            }
        }
        let files_touched = plan.changes.len();
        Ok((
            plan,
            RenameResult {
                old_name,
                new_name,
                files_touched,
                links_updated,
                title_updated,
            },
        ))
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
                let source_dir = if source == target.doc {
                    dir_segment(&target.new_rel)
                } else {
                    dir_segment(&self.documents[source].rel_path)
                };
                relative_path(&source_dir, &target.new_rel)
            }
        }
    }

    fn rewrite_document_links(&self, source: usize, target: &RenameTarget) -> (String, usize) {
        let doc = &self.documents[source];
        let content = &doc.content;
        let mut edits = Vec::new();
        let mut count = 0;
        for link in &doc.markdown.links {
            if !local_target(&link.target) {
                continue;
            }
            let (base, suffix) = split_target_suffix(&link.target);
            if link.kind == LinkKind::Markdown && !super::path::is_document(base) {
                if self.recursive
                    && source == target.doc
                    && !std::path::Path::new(base).is_absolute()
                {
                    let original = resolve_target_rel(&dir_segment(&doc.rel_path), base);
                    let value = format!(
                        "{}{suffix}",
                        relative_path(&dir_segment(&target.new_rel), &original)
                    );
                    if value != link.target {
                        count += 1;
                        edits.push((
                            link.destination.clone(),
                            write_destination(content, &link.destination, &value),
                        ));
                    }
                }
                continue;
            }
            let points_to_target = self.links_to(source, link.kind, base.trim(), target);
            let form = if base.starts_with("./") || base.starts_with("../") {
                TargetForm::Relative
            } else if base.contains('/') || base.contains('\\') {
                TargetForm::Absolute
            } else {
                TargetForm::Bare
            };
            let next = if points_to_target {
                if link.kind == LinkKind::Markdown {
                    self.new_markdown_target(source, base, suffix, target)
                } else {
                    let mut value = self.new_link_base(source, form, target);
                    if self.recursive && form == TargetForm::Relative && !value.starts_with("../") {
                        value.insert_str(0, "./");
                    }
                    if base.to_ascii_lowercase().ends_with(".md") {
                        value.push_str(".md");
                    }
                    format!("{value}{suffix}")
                }
            } else if self.recursive
                && source == target.doc
                && (link.kind == LinkKind::Markdown || form == TargetForm::Relative)
            {
                let source_dir = dir_segment(&doc.rel_path);
                let Some(resolved) =
                    self.resolve_recursive(&source_dir, link.kind, base, &document_key(base))
                else {
                    continue;
                };
                let mut value = relative_path(
                    &dir_segment(&target.new_rel),
                    &self.documents[resolved].rel_path,
                );
                if (base.starts_with("./") || link.kind == LinkKind::Wiki)
                    && !value.starts_with("../")
                {
                    value.insert_str(0, "./");
                }
                if base.to_ascii_lowercase().ends_with(".md") {
                    value.push_str(".md");
                }
                format!("{value}{suffix}")
            } else {
                continue;
            };
            if next == link.target {
                continue;
            }
            count += 1;
            if doc.markdown.image_destinations.contains(&link.destination) {
                let raw = &content[link.span.clone()];
                let end = label_end(raw).expect("parsed reference label");
                let title = if link.title.is_empty() {
                    String::new()
                } else {
                    format!(
                        " \"{}\"",
                        link.title.replace('\\', "\\\\").replace('\"', "\\\"")
                    )
                };
                edits.push((
                    link.span.clone(),
                    format!("{}(<{next}>{title})", &raw[..end + 1]),
                ));
            } else {
                edits.push((
                    link.destination.clone(),
                    if link.kind == LinkKind::Wiki {
                        next
                    } else {
                        write_destination(content, &link.destination, &next)
                    },
                ));
            }
        }
        (replace_ranges(content, edits), count)
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
            &if source == target.doc {
                dir_segment(&target.new_rel)
            } else {
                dir_segment(&self.documents[source].rel_path)
            },
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
