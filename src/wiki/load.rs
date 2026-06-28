use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{Document, LinkKind, Options, Vault};
use crate::{
    text::lower,
    wiki::{
        frontmatter::{first_preview_line, parse_front_matter},
        links::{find_link_only_lines, parse_links},
        path::{
            clean_rel_path, dir_segment, document_key, document_path_key, last_segment,
            resolve_target_rel, trim_md_ext,
        },
    },
};

impl Vault {
    pub(crate) fn load(root: &str, opts: Options) -> Result<Self, String> {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let mut files = discover_files(&root, opts.recursive)?;
        files.sort_by_key(|a| lower(&a.1));
        let recursive = opts.recursive;

        let mut docs: Vec<Document> = files
            .par_iter()
            .map(|(path, rel_file)| load_document(path, rel_file, recursive))
            .collect::<Result<Vec<_>, _>>()?;
        docs.sort_by_key(|a| lower(&a.rel_path));

        let mut docs_by_key = FxHashMap::default();
        for (i, doc) in docs.iter().enumerate() {
            if docs_by_key.insert(doc.key.clone(), i).is_some() {
                return Err(format!("duplicate document names {:?}", doc.rel_path));
            }
        }

        let n = docs.len();
        let mut vault = Self {
            root,
            recursive,
            documents: docs,
            docs_by_key,
            identifiers: FxHashMap::default(),
            basenames: FxHashMap::default(),
            directed: (0..n).map(|_| FxHashSet::default()).collect(),
            inbound: (0..n).map(|_| FxHashSet::default()).collect(),
            undirected: (0..n).map(|_| FxHashSet::default()).collect(),
        };
        vault.build_identifiers();
        vault.build_basenames();
        vault.build_graph();
        Ok(vault)
    }

    fn document_key_for(&self, rel_path: &str) -> String {
        if self.recursive {
            document_path_key(rel_path)
        } else {
            document_key(rel_path)
        }
    }

    fn build_identifiers(&mut self) {
        for i in 0..self.documents.len() {
            let mut ids = vec![self.documents[i].name.clone()];
            if !self.documents[i].front_matter.title.is_empty() {
                ids.push(self.documents[i].front_matter.title.clone());
            }
            ids.extend(self.documents[i].front_matter.aliases.clone());
            for id in ids {
                let key = document_key(&id);
                if !key.is_empty() {
                    self.identifiers.entry(key).or_default().push(i);
                }
            }
        }
    }

    fn build_basenames(&mut self) {
        for (i, doc) in self.documents.iter().enumerate() {
            let key = document_key(&last_segment(&doc.rel_path));
            if !key.is_empty() {
                self.basenames.entry(key).or_default().push(i);
            }
        }
        for docs in self.basenames.values_mut() {
            docs.sort_by(|&a, &b| {
                let da = self.documents[a].rel_path.matches('/').count();
                let db = self.documents[b].rel_path.matches('/').count();
                da.cmp(&db).then_with(|| {
                    lower(&self.documents[a].rel_path).cmp(&lower(&self.documents[b].rel_path))
                })
            });
        }
    }

    fn build_graph(&mut self) {
        for source in 0..self.documents.len() {
            let source_dir = dir_segment(&self.documents[source].rel_path);
            let link_count = self.documents[source].links.len();
            for li in 0..link_count {
                if let Some(target) = self.resolve_link_target(source, li, &source_dir) {
                    self.documents[source].links[li].resolved = Some(target);
                    if source == target || self.directed[source].contains(&target) {
                        continue;
                    }
                    self.directed[source].insert(target);
                    self.inbound[target].insert(source);
                    self.undirected[source].insert(target);
                    self.undirected[target].insert(source);
                }
            }
        }
    }

    fn resolve_link_target(
        &self,
        source: usize,
        link_idx: usize,
        source_dir: &str,
    ) -> Option<usize> {
        let link = &self.documents[source].links[link_idx];
        if !self.recursive {
            return self.docs_by_key.get(&link.target_key).copied();
        }
        self.resolve_recursive(source_dir, link.kind, &link.raw_target, &link.target_key)
    }

    fn resolve_recursive(
        &self,
        source_dir: &str,
        kind: LinkKind,
        raw_target: &str,
        base_key: &str,
    ) -> Option<usize> {
        if kind == LinkKind::Markdown {
            return self
                .docs_by_key
                .get(&document_path_key(&resolve_target_rel(
                    source_dir, raw_target,
                )))
                .copied();
        }
        if raw_target.starts_with("./") || raw_target.starts_with("../") {
            return self
                .docs_by_key
                .get(&document_path_key(&resolve_target_rel(
                    source_dir, raw_target,
                )))
                .copied();
        }
        if raw_target.contains('/') || raw_target.contains('\\') {
            return self
                .docs_by_key
                .get(&document_path_key(&clean_rel_path(raw_target)))
                .copied()
                .or_else(|| {
                    self.docs_by_key
                        .get(&document_path_key(&resolve_target_rel(
                            source_dir, raw_target,
                        )))
                        .copied()
                });
        }
        self.basenames
            .get(base_key)
            .and_then(|docs| docs.first())
            .copied()
    }

    pub(crate) fn resolve_document(&self, identifier: &str) -> Result<usize, String> {
        if let Some(&idx) = self.docs_by_key.get(&self.document_key_for(identifier)) {
            return Ok(idx);
        }
        let key = document_key(identifier);
        if key.is_empty() {
            return Err(format!("document {identifier:?} not found"));
        }
        if !self.recursive {
            if let Some(&idx) = self.docs_by_key.get(&key) {
                return Ok(idx);
            }
        }
        let mut docs = self.identifiers.get(&key).cloned().unwrap_or_default();
        docs.sort_unstable();
        docs.dedup();
        match docs.len() {
            0 => Err(format!("document {identifier:?} not found")),
            1 => Ok(docs[0]),
            _ => Err(format!("document identifier {identifier:?} is ambiguous")),
        }
    }
}

fn discover_files(root: &Path, recursive: bool) -> Result<Vec<(PathBuf, String)>, String> {
    if recursive {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if path.is_dir() {
                    if path != root && is_ignored_dir(&name) {
                        continue;
                    }
                    stack.push(path);
                } else if is_markdown(&path) {
                    let rel = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push((path, rel));
                }
            }
        }
        Ok(out)
    } else {
        let mut out = Vec::new();
        for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() && is_markdown(&path) {
                let rel = entry.file_name().to_string_lossy().to_string();
                out.push((path, rel));
            }
        }
        Ok(out)
    }
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("md"))
}

fn is_ignored_dir(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules" || name == "vendor"
}

fn load_document(path: &Path, rel_file: &str, recursive: bool) -> Result<Document, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let rel_path = trim_md_ext(rel_file);
    let name = if recursive {
        rel_path.clone()
    } else {
        last_segment(&rel_path)
    };
    let key = if recursive {
        document_path_key(&rel_path)
    } else {
        document_key(&rel_path)
    };
    Ok(Document {
        name,
        key,
        path: path.to_path_buf(),
        rel_path,
        excerpt: first_preview_line(&content),
        front_matter: parse_front_matter(&content),
        links: parse_links(&content),
        link_only: find_link_only_lines(&content),
    })
}
