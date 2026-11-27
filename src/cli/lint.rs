use super::{parse_common_or_help, Args};
use crate::{
    output::format_lint_report,
    wiki::{Options, Vault},
};

pub(super) fn lint_cmd(args: &[String]) -> Result<(), String> {
    let json = args.iter().any(|arg| arg == "-json" || arg == "--json");
    let mut common = Vec::new();
    let mut includes = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-json" | "--json" => {}
            "-include" | "--include" => includes.push(
                iter.next()
                    .ok_or("flag needs an argument: -include")?
                    .clone(),
            ),
            _ => common.push(arg.clone()),
        }
    }
    let Some(parsed) = parse_common_or_help(
        &common,
        "Usage: awiki lint [flags] [directory] [-json] [-include glob]",
    )?
    else {
        return Ok(());
    };
    let parsed = select_root(parsed, args)?;
    let vault = Vault::load_selected(
        &parsed.root,
        Options {
            recursive: parsed.recursive,
        },
        &includes,
    )?;
    let configured = vault.configured_lint()?;
    if json {
        let diagnostics = match configured {
            Some(value) => value,
            None => vault.default_diagnostics()?,
        };
        println!("{}", diagnostics.render_json()?);
        return if diagnostics.has_errors() {
            Err("lint issues found".into())
        } else {
            Ok(())
        };
    }
    if let Some(diagnostics) = configured {
        let text = diagnostics.render_text();
        if !text.is_empty() {
            println!("{text}");
        }
        if diagnostics.has_errors() {
            return Err("lint issues found".into());
        }
        println!(
            "// ok lint documents={} diagnostics={}",
            vault.documents.len(),
            diagnostics.items.len()
        );
        return Ok(());
    }
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

fn select_root(mut parsed: Args, args: &[String]) -> Result<Args, String> {
    if parsed.rest.len() > 1 {
        return Err("lint accepts at most one directory argument".into());
    }
    if let Some(root) = parsed.rest.pop() {
        if args.iter().any(|arg| arg == "-root") {
            return Err("lint accepts either a directory argument or -root, not both".into());
        }
        parsed.root = root;
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::parse_common;

    fn root(args: &[&str]) -> Result<Args, String> {
        let args = args
            .iter()
            .map(|arg| (*arg).to_string())
            .collect::<Vec<_>>();
        select_root(parse_common(&args)?, &args)
    }

    #[test]
    fn directory_argument_and_existing_flags_select_the_wiki_root() {
        assert_eq!(root(&[]).unwrap().root, ".");
        assert_eq!(root(&["docs/"]).unwrap().root, "docs/");
        assert_eq!(root(&["-root", "docs/"]).unwrap().root, "docs/");
        let parsed = root(&["docs with spaces/", "-recursive"]).unwrap();
        assert_eq!(parsed.root, "docs with spaces/");
        assert!(parsed.recursive);
    }

    #[test]
    fn multiple_or_conflicting_roots_are_rejected() {
        assert!(root(&["docs/", "other/"]).is_err());
        assert!(root(&["-root", ".", "docs/"]).is_err());
        assert!(root(&["docs/", "-root", "docs/"]).is_err());
    }
}
