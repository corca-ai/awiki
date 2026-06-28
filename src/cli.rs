use std::{env, process};

use crate::{
    output::format_lint_report,
    wiki::{Options, Vault},
};

const WANTED_SOURCE_PREVIEW_LIMIT: usize = 10;

struct Args {
    root: String,
    recursive: bool,
    rest: Vec<String>,
}

pub(crate) fn run() {
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
