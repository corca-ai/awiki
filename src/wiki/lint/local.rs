use github_slugger::Slugger;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct ResourceChecker {
    headings: HashMap<PathBuf, Result<HashSet<String>, String>>,
}

impl ResourceChecker {
    pub(super) fn check(&mut self, source: &Path, target: &str) -> Result<(), String> {
        if target.is_empty() || target.starts_with("//") {
            return Ok(());
        }
        let (without_fragment, fragment) = target.split_once('#').unwrap_or((target, ""));
        let path = without_fragment.split('?').next().unwrap_or("");
        if path.split('/').next().unwrap_or("").contains(':') {
            return Ok(());
        }
        let destination = if path.is_empty() {
            source.to_path_buf()
        } else if path.starts_with('/') {
            return Err("root-relative links are not supported; use a relative path".into());
        } else {
            source
                .parent()
                .ok_or("document has no parent")?
                .join(decode(path)?)
        };
        let metadata =
            fs::metadata(&destination).map_err(|e| format!("cannot access local target: {e}"))?;
        if !metadata.is_file() && !metadata.is_dir() {
            return Err("local target is not a file or directory".into());
        }
        if fragment.is_empty() || !metadata.is_file() {
            return Ok(());
        }
        let extension = destination
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        let markdown = extension.eq_ignore_ascii_case("md");
        if !markdown {
            return Err("fragment checks support Markdown documents only".into());
        }
        let fragment = decode(fragment)?;
        if fragment == "top" {
            return Ok(());
        }
        let key = destination.canonicalize().unwrap_or(destination.clone());
        let headings = self
            .headings
            .entry(key)
            .or_insert_with(|| document_ids(&destination));
        if headings.as_ref().map_err(Clone::clone)?.contains(&fragment) {
            Ok(())
        } else {
            Err(format!("heading fragment not found: {fragment}"))
        }
    }
}

pub(super) fn is_markup(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case("html")
                || ext.eq_ignore_ascii_case("htm")
                || ext.eq_ignore_ascii_case("svg")
        })
}

pub(super) fn is_source(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext, "ts" | "tsx" | "rs"))
}

fn document_ids(path: &Path) -> Result<HashSet<String>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read Markdown target: {e}"))?;
    let mut slugger = Slugger::default();
    let mut ids = HashSet::new();
    let mut heading = None::<String>;
    for event in Parser::new_ext(
        &text,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    ) {
        match event {
            Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(text) = heading.take() {
                    ids.insert(slugger.slug(&text));
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(current) = &mut heading {
                    current.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(current) = &mut heading {
                    current.push(' ');
                }
            }
            _ => {}
        }
    }
    Ok(ids)
}

fn decode(value: &str) -> Result<String, String> {
    let mut decoded = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = bytes.get(index + 1).and_then(|b| (*b as char).to_digit(16));
            let low = bytes.get(index + 2).and_then(|b| (*b as char).to_digit(16));
            let (Some(high), Some(low)) = (high, low) else {
                return Err("invalid percent encoding".into());
            };
            decoded.push((high * 16 + low) as u8);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| "local target is not UTF-8".into())
}
