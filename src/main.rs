use rand::{rngs::StdRng, Rng, SeedableRng};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    collections::VecDeque,
    env, fs,
    path::{Component, Path, PathBuf},
    process,
};
use unicode_normalization::UnicodeNormalization;

const PREVIEW_LIMIT: usize = 140;
const WANTED_SOURCE_PREVIEW_LIMIT: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinkKind {
    Wiki,
    Markdown,
}

#[derive(Clone, Debug)]
struct Link {
    kind: LinkKind,
    display_target: String,
    target_key: String,
    raw_target: String,
    resolved: Option<usize>,
    context: String,
}

#[derive(Clone, Debug, Default)]
struct FrontMatter {
    present: bool,
    body_offset: usize,
    title: String,
    aliases: Vec<String>,
}

#[derive(Clone, Debug)]
struct LinkOnlyLine {
    line: usize,
    text: String,
}

#[derive(Clone, Debug)]
struct Document {
    name: String,
    key: String,
    path: PathBuf,
    rel_path: String,
    excerpt: String,
    front_matter: FrontMatter,
    links: Vec<Link>,
    link_only: Vec<LinkOnlyLine>,
}

struct Vault {
    root: PathBuf,
    recursive: bool,
    documents: Vec<Document>,
    docs_by_key: FxHashMap<String, usize>,
    identifiers: FxHashMap<String, Vec<usize>>,
    basenames: FxHashMap<String, Vec<usize>>,
    directed: Vec<FxHashSet<usize>>,
    inbound: Vec<FxHashSet<usize>>,
    undirected: Vec<FxHashSet<usize>>,
}

#[derive(Clone, Copy)]
struct Options {
    recursive: bool,
}

struct LintReport {
    document_count: usize,
    largest_component_size: usize,
    covered_documents: usize,
    orphans: Vec<usize>,
    islands: Vec<Vec<usize>>,
    link_only_lines: Vec<(usize, LinkOnlyLine)>,
}

impl LintReport {
    fn has_issues(&self) -> bool {
        !self.orphans.is_empty() || !self.islands.is_empty() || !self.link_only_lines.is_empty()
    }
    fn largest_component_ratio(&self) -> f64 {
        ratio(self.largest_component_size, self.document_count)
    }
    fn orphan_rate(&self) -> f64 {
        ratio(self.orphans.len(), self.document_count)
    }
    fn content_coverage(&self) -> f64 {
        ratio(self.covered_documents, self.document_count)
    }
}

#[derive(Clone)]
struct WantedSource {
    document: String,
    context: String,
    mentions: usize,
}

struct WantedPage {
    name: String,
    mentions: usize,
    source_documents: usize,
    sources: Vec<WantedSource>,
}

type AvgPathReport = (usize, usize, f64, Vec<Vec<usize>>);

#[derive(Default)]
struct Args {
    root: String,
    recursive: bool,
    rest: Vec<String>,
}

fn main() {
    let mut argv: Vec<String> = env::args().skip(1).collect();
    if argv.is_empty() {
        usage();
        process::exit(2);
    }
    let cmd = argv.remove(0);
    let result = match cmd.as_str() {
        "help" | "--help" | "-help" | "-h" => {
            usage();
            Ok(())
        }
        "lint" => lint_cmd(&argv),
        "links" => links_cmd(&argv),
        "path" => path_cmd(&argv),
        "wanted" => wanted_cmd(&argv),
        "avg-shortest-path" => avg_shortest_path_cmd(&argv),
        "rename" => rename_cmd(&argv),
        "version" | "--version" | "-version" => {
            println!("{}", version());
            Ok(())
        }
        other => {
            if argv.iter().any(|a| a == "--version" || a == "-version") {
                println!("{}", version());
                Ok(())
            } else {
                eprintln!("awiki: unknown command {other:?}\n");
                usage();
                process::exit(2);
            }
        }
    };

    if let Err(err) = result {
        if err != "lint issues found" {
            eprintln!("awiki: {err}");
        }
        process::exit(1);
    }
}

fn usage() {
    eprintln!("awiki helps maintain the quality of a flat-file Markdown wiki.");
    eprintln!("Quality here means keeping notes well connected, reducing orphans and disconnected islands, keeping most pages inside one large component,");
    eprintln!("avoiding empty stubs, and making long paths and heavily linked missing pages easy to inspect.");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  awiki lint [flags]");
    eprintln!("      Validate the wiki graph");
    eprintln!("  awiki avg-shortest-path [flags]");
    eprintln!("      Estimate average shortest path length and print sampled long paths");
    eprintln!("  awiki path [flags] <from> <to>");
    eprintln!("      Print the shortest path between two documents");
    eprintln!("  awiki rename [flags] <old> <new>");
    eprintln!("      Rename a document and update links to it");
    eprintln!("  awiki links [flags] <document>");
    eprintln!("      Show inbound and outbound links for a document");
    eprintln!("  awiki wanted [flags]");
    eprintln!("      Show the most-linked missing pages");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  awiki path \"The China study (book)\" \"What to Eat\"");
    eprintln!("  awiki links \"Books Ive read\"");
    eprintln!();
    eprintln!("Use `awiki <command> -h` for command-specific help.");
}

fn version() -> &'static str {
    option_env!("AWIKI_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

fn parse_common(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        root: ".".to_string(),
        recursive: false,
        rest: Vec::new(),
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "-help" | "--help" => return Err("__help__".to_string()),
            "-root" => {
                i += 1;
                if i >= args.len() {
                    return Err("flag needs an argument: -root".to_string());
                }
                parsed.root = args[i].clone();
            }
            "-recursive" | "-r" => parsed.recursive = true,
            value if value.starts_with('-') => {
                return Err(format!("flag provided but not defined: {value}"))
            }
            value => parsed.rest.push(value.to_string()),
        }
        i += 1;
    }
    Ok(parsed)
}

fn lint_cmd(args: &[String]) -> Result<(), String> {
    let parsed = match parse_common(args) {
        Ok(p) => p,
        Err(e) if e == "__help__" => {
            eprintln!("Usage: awiki lint [flags]");
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let vault = Vault::load(
        &parsed.root,
        Options {
            recursive: parsed.recursive,
        },
    )?;
    let report = vault.lint();
    if report.has_issues() {
        println!("{}", format_lint_report(&vault, &report));
        return Err("lint issues found".to_string());
    }
    println!(
        "// ok connected_graph documents={} largest_component_ratio={:.4} orphan_rate={:.4} content_coverage={:.4}",
        report.document_count,
        report.largest_component_ratio(),
        report.orphan_rate(),
        report.content_coverage()
    );
    Ok(())
}

fn links_cmd(args: &[String]) -> Result<(), String> {
    let parsed = parse_common(args)?;
    if parsed.rest.len() != 1 {
        return Err("links requires exactly one document argument".to_string());
    }
    let vault = Vault::load(
        &parsed.root,
        Options {
            recursive: parsed.recursive,
        },
    )?;
    let doc = vault.resolve_document(&parsed.rest[0])?;
    println!("// this page");
    println!("{}", vault.document_line(doc));
    println!("// incoming links");
    let mut incoming: Vec<_> = vault.inbound[doc].iter().copied().collect();
    vault.sort_doc_indices(&mut incoming);
    if incoming.is_empty() {
        println!("// none");
    } else {
        for idx in incoming {
            println!("{}", vault.document_line(idx));
        }
    }
    println!("// outgoing links");
    let outgoing = vault.outbound_summaries(doc);
    if outgoing.is_empty() {
        println!("// none");
    } else {
        for (name, missing, idx) in outgoing {
            if missing {
                println!("[[{name}]]: (missing)");
            } else if let Some(i) = idx {
                println!("{}", vault.document_line(i));
            }
        }
    }
    Ok(())
}

fn path_cmd(args: &[String]) -> Result<(), String> {
    let parsed = parse_common(args)?;
    if parsed.rest.len() != 2 {
        return Err("path requires exactly two document arguments".to_string());
    }
    let vault = Vault::load(
        &parsed.root,
        Options {
            recursive: parsed.recursive,
        },
    )?;
    let from = vault.resolve_document(&parsed.rest[0])?;
    let to = vault.resolve_document(&parsed.rest[1])?;
    for idx in vault.shortest_path_indices(from, to)? {
        println!("{}", vault.document_line(idx));
    }
    Ok(())
}

fn wanted_cmd(args: &[String]) -> Result<(), String> {
    let mut common = Vec::new();
    let mut limit = 10usize;
    let mut sources = WANTED_SOURCE_PREVIEW_LIMIT;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-n" => {
                i += 1;
                if i >= args.len() {
                    return Err("flag needs an argument: -n".to_string());
                }
                limit = args[i]
                    .parse()
                    .map_err(|_| "wanted limit must not be negative".to_string())?;
            }
            "-sources" => {
                i += 1;
                if i >= args.len() {
                    return Err("flag needs an argument: -sources".to_string());
                }
                sources = args[i]
                    .parse()
                    .map_err(|_| "wanted sources must not be negative".to_string())?;
            }
            other => common.push(other.to_string()),
        }
        i += 1;
    }
    let parsed = parse_common(&common)?;
    if !parsed.rest.is_empty() {
        return Err("wanted does not accept positional arguments".to_string());
    }
    let vault = Vault::load(
        &parsed.root,
        Options {
            recursive: parsed.recursive,
        },
    )?;
    let mut pages = vault.all_wanted_pages();
    if pages.len() > limit {
        pages.truncate(limit);
    }
    if pages.is_empty() {
        println!("_ none");
        return Ok(());
    }
    for (page_idx, page) in pages.iter().enumerate() {
        if page_idx > 0 {
            println!();
        }
        let label = if page.mentions == 1 { "link" } else { "links" };
        println!("[[{}]] ({} {label})", page.name, page.mentions);
        println!();
        let shown = if sources > 0 {
            sources.min(page.sources.len())
        } else {
            page.sources.len()
        };
        for source in page.sources.iter().take(shown) {
            println!("- [[{}]]: {}", source.document, source.context);
        }
        if page.sources.len() > shown {
            println!("_ ...");
        }
    }
    Ok(())
}

fn avg_shortest_path_cmd(args: &[String]) -> Result<(), String> {
    let mut common = Vec::new();
    let mut samples = 500usize;
    let mut examples = 1usize;
    let mut seed = 1u64;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-samples" => {
                i += 1;
                samples = args
                    .get(i)
                    .ok_or("flag needs an argument: -samples")?
                    .parse()
                    .map_err(|_| "sample count must be positive".to_string())?;
            }
            "-examples" => {
                i += 1;
                examples = args
                    .get(i)
                    .ok_or("flag needs an argument: -examples")?
                    .parse()
                    .map_err(|_| "invalid examples".to_string())?;
            }
            "-seed" => {
                i += 1;
                seed = args
                    .get(i)
                    .ok_or("flag needs an argument: -seed")?
                    .parse()
                    .map_err(|_| "invalid seed".to_string())?;
            }
            other => common.push(other.to_string()),
        }
        i += 1;
    }
    let parsed = parse_common(&common)?;
    let vault = Vault::load(
        &parsed.root,
        Options {
            recursive: parsed.recursive,
        },
    )?;
    let report = vault.approx_avg_shortest_path(samples, examples, seed)?;
    println!(
        "// largest_component_size={} samples={} average_shortest_path={:.4}",
        report.0, report.1, report.2
    );
    for (i, path) in report.3.iter().enumerate() {
        if i > 0 {
            println!();
        }
        for &idx in path {
            println!("{}", vault.document_line(idx));
        }
    }
    Ok(())
}

fn rename_cmd(args: &[String]) -> Result<(), String> {
    let parsed = parse_common(args)?;
    if parsed.rest.len() != 2 {
        return Err("rename requires exactly two document arguments".to_string());
    }
    let mut vault = Vault::load(
        &parsed.root,
        Options {
            recursive: parsed.recursive,
        },
    )?;
    let result = vault.rename(&parsed.rest[0], &parsed.rest[1])?;
    println!(
        "// rename old={}.md new={}.md",
        result.old_name, result.new_name
    );
    println!(
        "// links_updated={} files_touched={} title_updated={}",
        result.links_updated, result.files_touched, result.title_updated
    );
    Ok(())
}

impl Vault {
    fn load(root: &str, opts: Options) -> Result<Self, String> {
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

    fn resolve_document(&self, identifier: &str) -> Result<usize, String> {
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

    fn document_line(&self, idx: usize) -> String {
        format_document_line(
            &self.documents[idx].name,
            &truncate_runes(self.documents[idx].excerpt.trim(), PREVIEW_LIMIT),
        )
    }

    fn sort_doc_indices(&self, docs: &mut [usize]) {
        docs.sort_by(|&a, &b| lower(&self.documents[a].name).cmp(&lower(&self.documents[b].name)));
    }

    fn outbound_summaries(&self, idx: usize) -> Vec<(String, bool, Option<usize>)> {
        let mut seen: FxHashMap<String, (String, bool, Option<usize>)> = FxHashMap::default();
        for link in &self.documents[idx].links {
            if let Some(target) = link.resolved {
                let name = self.documents[target].name.clone();
                seen.insert(format!("doc:{}", lower(&name)), (name, false, Some(target)));
            } else {
                let name = link.display_target.clone();
                seen.insert(format!("missing:{}", lower(&name)), (name, true, None));
            }
        }
        let mut out: Vec<_> = seen.into_values().collect();
        out.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| lower(&a.0).cmp(&lower(&b.0))));
        out
    }

    fn lint(&self) -> LintReport {
        let mut report = LintReport {
            document_count: self.documents.len(),
            largest_component_size: 0,
            covered_documents: 0,
            orphans: Vec::new(),
            islands: Vec::new(),
            link_only_lines: Vec::new(),
        };
        let mut visited = vec![false; self.documents.len()];
        let mut components = Vec::new();
        for (idx, doc) in self.documents.iter().enumerate() {
            if !doc.excerpt.trim().is_empty() {
                report.covered_documents += 1;
            }
            for issue in &doc.link_only {
                report.link_only_lines.push((idx, issue.clone()));
            }
            if self.undirected[idx].is_empty() {
                report.orphans.push(idx);
                visited[idx] = true;
                continue;
            }
            if visited[idx] {
                continue;
            }
            let mut component = self.collect_component(idx, &mut visited);
            self.sort_doc_indices(&mut component);
            components.push(component);
        }
        self.sort_doc_indices(&mut report.orphans);
        report.link_only_lines.sort_by(|(a, ia), (b, ib)| {
            lower(&self.documents[*a].name)
                .cmp(&lower(&self.documents[*b].name))
                .then(ia.line.cmp(&ib.line))
        });
        components.sort_by(|a, b| {
            b.len().cmp(&a.len()).then_with(|| {
                lower(&self.documents[a[0]].name).cmp(&lower(&self.documents[b[0]].name))
            })
        });
        if components.len() > 1 {
            report.islands = components[1..].to_vec();
        }
        if let Some(first) = components.first() {
            report.largest_component_size = first.len();
        } else if !report.orphans.is_empty() {
            report.largest_component_size = 1;
        }
        report
    }

    fn collect_component(&self, start: usize, visited: &mut [bool]) -> Vec<usize> {
        let mut q = VecDeque::from([start]);
        visited[start] = true;
        let mut component = Vec::new();
        while let Some(cur) = q.pop_front() {
            component.push(cur);
            let mut neighbors: Vec<_> = self.undirected[cur].iter().copied().collect();
            self.sort_doc_indices(&mut neighbors);
            for next in neighbors {
                if !visited[next] {
                    visited[next] = true;
                    q.push_back(next);
                }
            }
        }
        component
    }

    fn shortest_path_indices(&self, from: usize, to: usize) -> Result<Vec<usize>, String> {
        if from == to {
            return Ok(vec![from]);
        }
        let mut prev = vec![usize::MAX; self.documents.len()];
        let mut q = VecDeque::from([from]);
        prev[from] = from;
        while let Some(cur) = q.pop_front() {
            let mut neighbors: Vec<_> = self.undirected[cur].iter().copied().collect();
            self.sort_doc_indices(&mut neighbors);
            for next in neighbors {
                if prev[next] != usize::MAX {
                    continue;
                }
                prev[next] = cur;
                if next == to {
                    let mut rev = Vec::new();
                    let mut at = to;
                    while at != from {
                        rev.push(at);
                        at = prev[at];
                    }
                    rev.push(from);
                    rev.reverse();
                    return Ok(rev);
                }
                q.push_back(next);
            }
        }
        Err(format!(
            "no path between {:?} and {:?}",
            self.documents[from].name, self.documents[to].name
        ))
    }

    fn largest_component_keys(&self) -> Vec<usize> {
        let mut visited = vec![false; self.documents.len()];
        let mut best: Vec<usize> = Vec::new();
        for idx in 0..self.documents.len() {
            if visited[idx] {
                continue;
            }
            if self.undirected[idx].is_empty() {
                visited[idx] = true;
                continue;
            }
            let mut component = self.collect_component(idx, &mut visited);
            self.sort_doc_indices(&mut component);
            if component.len() > best.len()
                || (component.len() == best.len()
                    && !component.is_empty()
                    && lower(&self.documents[component[0]].name)
                        < lower(&self.documents[best[0]].name))
            {
                best = component;
            }
        }
        best
    }

    fn approx_avg_shortest_path(
        &self,
        sample_count: usize,
        example_count: usize,
        seed: u64,
    ) -> Result<AvgPathReport, String> {
        let component = self.largest_component_keys();
        if component.len() < 2 {
            return Err(
                "largest connected component must contain at least two documents".to_string(),
            );
        }
        if sample_count == 0 {
            return Err("sample count must be positive".to_string());
        }
        let pairs = sample_component_pairs(component.len(), sample_count, seed);
        let samples: Vec<Vec<usize>> = pairs
            .par_iter()
            .map(|&(a, b)| {
                self.shortest_path_indices(component[a], component[b])
                    .unwrap_or_default()
            })
            .collect();
        let total: usize = samples.iter().map(|p| p.len().saturating_sub(1)).sum();
        let avg = total as f64 / samples.len() as f64;
        let mut longer: Vec<_> = samples
            .into_iter()
            .filter(|p| (p.len().saturating_sub(1) as f64) > avg)
            .collect();
        longer.sort_by(|a, b| {
            b.len()
                .cmp(&a.len())
                .then_with(|| path_sort_key(self, a).cmp(&path_sort_key(self, b)))
        });
        longer.truncate(example_count);
        Ok((component.len(), pairs.len(), avg, longer))
    }

    fn all_wanted_pages(&self) -> Vec<WantedPage> {
        #[derive(Default)]
        struct Agg {
            name: String,
            mentions: usize,
            docs: FxHashSet<usize>,
            source_index: FxHashMap<String, usize>,
            sources: Vec<WantedSource>,
        }
        let mut aggs: FxHashMap<String, Agg> = FxHashMap::default();
        for (doc_idx, doc) in self.documents.iter().enumerate() {
            for link in &doc.links {
                if link.resolved.is_some() || link.target_key.is_empty() {
                    continue;
                }
                let mut name = normalize_document_name(&link.display_target);
                if name.is_empty() {
                    name = link.display_target.trim().to_string();
                }
                if name.is_empty() {
                    continue;
                }
                let agg = aggs.entry(link.target_key.clone()).or_insert_with(|| Agg {
                    name,
                    ..Default::default()
                });
                agg.mentions += 1;
                agg.docs.insert(doc_idx);
                let context = if !link.context.trim().is_empty() {
                    link.context.trim().to_string()
                } else if !doc.excerpt.trim().is_empty() {
                    doc.excerpt.trim().to_string()
                } else {
                    "(empty)".to_string()
                };
                let key = format!("{}\0{}", doc.key, context);
                if let Some(&idx) = agg.source_index.get(&key) {
                    agg.sources[idx].mentions += 1;
                } else {
                    agg.source_index.insert(key, agg.sources.len());
                    agg.sources.push(WantedSource {
                        document: doc.name.clone(),
                        context,
                        mentions: 1,
                    });
                }
            }
        }
        let mut pages: Vec<_> = aggs
            .into_values()
            .map(|mut a| {
                a.sources.sort_by(|x, y| {
                    y.mentions
                        .cmp(&x.mentions)
                        .then_with(|| lower(&x.document).cmp(&lower(&y.document)))
                        .then_with(|| lower(&x.context).cmp(&lower(&y.context)))
                });
                WantedPage {
                    name: a.name,
                    mentions: a.mentions,
                    source_documents: a.docs.len(),
                    sources: a.sources,
                }
            })
            .collect();
        pages.sort_by(|a, b| {
            b.mentions
                .cmp(&a.mentions)
                .then(b.source_documents.cmp(&a.source_documents))
                .then_with(|| lower(&a.name).cmp(&lower(&b.name)))
        });
        pages
    }

    fn rename(&mut self, old_id: &str, new_id: &str) -> Result<RenameResult, String> {
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

struct RenameResult {
    old_name: String,
    new_name: String,
    files_touched: usize,
    links_updated: usize,
    title_updated: bool,
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

fn parse_front_matter(content: &str) -> FrontMatter {
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

fn first_preview_line(content: &str) -> String {
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

fn parse_links(content: &str) -> Vec<Link> {
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

fn find_link_only_lines(content: &str) -> Vec<LinkOnlyLine> {
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

fn format_lint_report(vault: &Vault, report: &LintReport) -> String {
    let mut out = format!(
        "// lint_failed documents={} orphans={} islands={} link_only_lines={} largest_component_ratio={:.4} orphan_rate={:.4} content_coverage={:.4}",
        report.document_count,
        report.orphans.len(),
        report.islands.len(),
        report.link_only_lines.len(),
        report.largest_component_ratio(),
        report.orphan_rate(),
        report.content_coverage()
    );
    if !report.orphans.is_empty() {
        out.push_str("\n// orphan");
        for &idx in &report.orphans {
            out.push('\n');
            out.push_str(&vault.document_line(idx));
        }
    }
    for (i, island) in report.islands.iter().enumerate() {
        out.push_str(&format!("\n// island={}", i + 1));
        for &idx in island {
            out.push('\n');
            out.push_str(&vault.document_line(idx));
        }
    }
    if !report.link_only_lines.is_empty() {
        out.push_str("\n// link_only_line");
        for (idx, issue) in &report.link_only_lines {
            out.push_str(&format!(
                "\n[[{}]]:{}: {}",
                vault.documents[*idx].name, issue.line, issue.text
            ));
        }
    }
    out
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

fn update_front_matter_title(content: &str, old_title: &str, new_title: &str) -> (String, bool) {
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

fn sample_component_pairs(
    node_count: usize,
    sample_count: usize,
    seed: u64,
) -> Vec<(usize, usize)> {
    let total_pairs = node_count * (node_count - 1) / 2;
    if sample_count >= total_pairs {
        let mut pairs = Vec::with_capacity(total_pairs);
        for i in 0..node_count {
            for j in i + 1..node_count {
                pairs.push((i, j));
            }
        }
        return pairs;
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let mut seen = FxHashSet::default();
    let mut pairs = Vec::with_capacity(sample_count);
    while pairs.len() < sample_count {
        let mut i = rng.gen_range(0..node_count);
        let mut j = rng.gen_range(0..node_count - 1);
        if j >= i {
            j += 1;
        }
        if i > j {
            std::mem::swap(&mut i, &mut j);
        }
        let key = ((i as u64) << 32) | j as u64;
        if seen.insert(key) {
            pairs.push((i, j));
        }
    }
    pairs
}

fn path_sort_key(vault: &Vault, path: &[usize]) -> String {
    path.iter()
        .map(|&idx| lower(&vault.documents[idx].name))
        .collect::<Vec<_>>()
        .join("\0")
}

fn scan_lines(content: &str) -> Vec<(&str, usize, usize)> {
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

fn trim_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}

fn normalize_preview_line(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn mask_inline_code(line: &str) -> String {
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

fn split_wiki_link_parts(inner: &str) -> (&str, &str, bool, bool) {
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

fn parse_markdown_target(dest: &str) -> Option<&str> {
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

fn split_target_suffix(target: &str) -> (&str, &str) {
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

fn normalize_document_name(value: &str) -> String {
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

fn document_key(value: &str) -> String {
    normalize_document_name(value).to_lowercase()
}

fn raw_link_target(base: &str) -> String {
    let value = base.trim().trim_matches(['<', '>']).trim();
    let (base, _) = split_target_suffix(value);
    base.trim().to_string()
}

fn document_path_key(rel_path: &str) -> String {
    let mut cleaned = clean_rel_path(rel_path);
    if cleaned.to_ascii_lowercase().ends_with(".md") {
        cleaned.truncate(cleaned.len() - 3);
    }
    cleaned.nfc().collect::<String>().to_lowercase()
}

fn clean_rel_path(p: &str) -> String {
    let p = p.trim().strip_prefix("./").unwrap_or(p.trim());
    let cleaned = clean_path(p);
    if cleaned == "." || cleaned == "/" {
        String::new()
    } else {
        cleaned
    }
}

fn clean_path(p: &str) -> String {
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

fn resolve_target_rel(source_dir: &str, raw_target: &str) -> String {
    if source_dir.is_empty() || source_dir == "." {
        clean_rel_path(raw_target)
    } else {
        clean_rel_path(&join_slash(source_dir, raw_target))
    }
}

fn join_slash(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else {
        format!("{a}/{b}")
    }
}

fn last_segment(rel_path: &str) -> String {
    clean_rel_path(rel_path)
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

fn dir_segment(rel_path: &str) -> String {
    let cleaned = clean_rel_path(rel_path);
    cleaned
        .rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .unwrap_or_default()
}

fn trim_md_ext(rel_file: &str) -> String {
    if rel_file.to_ascii_lowercase().ends_with(".md") {
        rel_file[..rel_file.len() - 3].to_string()
    } else {
        rel_file.to_string()
    }
}

fn fence_start(line: &str) -> Option<(char, usize)> {
    if line.starts_with("```") {
        Some(('`', line.bytes().take_while(|&b| b == b'`').count()))
    } else if line.starts_with("~~~") {
        Some(('~', line.bytes().take_while(|&b| b == b'~').count()))
    } else {
        None
    }
}

fn next_fence_state(
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

fn starts_indented(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

fn lower(value: &str) -> String {
    value.to_lowercase()
}

fn ratio(count: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        count as f64 / total as f64
    }
}

fn truncate_runes(value: &str, limit: usize) -> String {
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

fn format_document_line(name: &str, preview: &str) -> String {
    let preview = preview.trim();
    if preview.is_empty() {
        format!("[[{name}]]: (empty)")
    } else {
        format!("[[{name}]]: {preview}")
    }
}

fn find_byte(bytes: &[u8], start: usize, needle: u8) -> Option<usize> {
    bytes
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(i, &b)| (b == needle).then_some(i))
}

fn find_bytes(bytes: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    bytes[start..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + start)
}

fn strip_line_only_markdown(value: &str) -> String {
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

fn contains_letter_or_digit(value: &str) -> bool {
    value.chars().any(|c| c.is_alphanumeric())
}

fn wrap_wiki_link(target: &str, label: &str, has_label: bool, escaped_separator: bool) -> String {
    if !has_label {
        format!("[[{target}]]")
    } else if escaped_separator {
        format!("[[{target}\\|{label}]]")
    } else {
        format!("[[{target}|{label}]]")
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn lint_finds_link_only_lines_but_allows_two_links() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Alpha.md"),
            "Alpha summary.\n\n- [[Beta]]\n- **[[Beta]]**\n- [Beta](Beta.md)\n- [[Beta|B]]\n- [[Beta]] [[Gamma]]\n- [[Beta]] explains Beta.\n```\n- [[Beta]]\n```\n",
        )
        .unwrap();
        fs::write(dir.path().join("Beta.md"), "Beta summary.\n").unwrap();
        fs::write(dir.path().join("Gamma.md"), "Gamma summary.\n").unwrap();

        let vault =
            Vault::load(dir.path().to_str().unwrap(), Options { recursive: false }).unwrap();
        let report = vault.lint();
        let lines: Vec<_> = report
            .link_only_lines
            .iter()
            .map(|(_, issue)| (issue.line, issue.text.as_str()))
            .collect();
        assert_eq!(
            lines,
            vec![
                (3, "- [[Beta]]"),
                (4, "- **[[Beta]]**"),
                (5, "- [Beta](Beta.md)"),
                (6, "- [[Beta|B]]")
            ]
        );
    }

    #[test]
    fn wanted_ranks_missing_links() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Doc1.md"),
            "First with [[Wanted A]].\n\nAnother with [[Wanted B]].\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("Doc2.md"),
            "Again [[Wanted A]] and [[Wanted A]].\n",
        )
        .unwrap();
        let vault =
            Vault::load(dir.path().to_str().unwrap(), Options { recursive: false }).unwrap();
        let pages = vault.all_wanted_pages();
        assert_eq!(pages[0].name, "Wanted A");
        assert_eq!(pages[0].mentions, 3);
        assert_eq!(pages[1].name, "Wanted B");
    }
}
