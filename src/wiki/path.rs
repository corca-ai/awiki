use std::path::{Component, Path};

use unicode_normalization::UnicodeNormalization;

use crate::wiki::links::split_target_suffix;

pub(crate) fn normalize_document_name(value: &str) -> String {
    let value = value.trim().trim_matches(['<', '>']);
    if value.is_empty() {
        return String::new();
    }
    let (base, _) = split_target_suffix(value);
    let base = base.strip_prefix("./").unwrap_or(base);
    let clean = clean_path(base);
    let Some(last) = clean.rsplit('/').next() else {
        return String::new();
    };
    let mut name = last.to_string();
    if name.to_ascii_lowercase().ends_with(".md") {
        name.truncate(name.len() - 3);
    }
    name.trim().nfc().collect::<String>()
}

pub(crate) fn document_key(value: &str) -> String {
    normalize_document_name(value).to_lowercase()
}

pub(crate) fn document_path_key(rel_path: &str) -> String {
    let mut cleaned = clean_rel_path(rel_path);
    if cleaned.to_ascii_lowercase().ends_with(".md") {
        cleaned.truncate(cleaned.len() - 3);
    }
    cleaned.nfc().collect::<String>().to_lowercase()
}

pub(crate) fn clean_rel_path(p: &str) -> String {
    let p = p.trim().strip_prefix("./").unwrap_or(p.trim());
    let cleaned = clean_path(p);
    if cleaned == "." || cleaned == "/" {
        String::new()
    } else {
        cleaned
    }
}

pub(crate) fn clean_path(p: &str) -> String {
    let mut parts = Vec::new();
    for comp in Path::new(p).components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.last().is_some_and(|last: &&str| *last != "..") {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            Component::Normal(s) => parts.push(s.to_str().unwrap_or("")),
            Component::RootDir => parts.clear(),
            Component::Prefix(_) => {}
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

pub(crate) fn resolve_target_rel(source_dir: &str, raw_target: &str) -> String {
    if source_dir.is_empty() || source_dir == "." {
        clean_rel_path(raw_target)
    } else {
        clean_rel_path(&join_slash(source_dir, raw_target))
    }
}

pub(crate) fn join_slash(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else {
        format!("{a}/{b}")
    }
}

pub(crate) fn last_segment(rel_path: &str) -> String {
    clean_rel_path(rel_path)
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

pub(crate) fn dir_segment(rel_path: &str) -> String {
    let cleaned = clean_rel_path(rel_path);
    cleaned
        .rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .unwrap_or_default()
}

pub(crate) fn trim_md_ext(rel_file: &str) -> String {
    if rel_file.to_ascii_lowercase().ends_with(".md") {
        rel_file[..rel_file.len() - 3].to_string()
    } else {
        rel_file.to_string()
    }
}
