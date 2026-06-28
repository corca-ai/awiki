use crate::wiki::{format_suggest_report, Options, SuggestFilter, SuggestOptions, Vault};

pub(super) fn suggest_cmd(args: &[String]) -> Result<(), String> {
    let mut common = Vec::new();
    let mut opts = SuggestOptions::default();
    let mut custom_filter = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "-help" | "--help" => {
                suggest_usage();
                return Ok(());
            }
            "-filter" | "--filter" => {
                i += 1;
                opts.filters = parse_suggest_filters(take_arg(args, i, "--filter")?)?;
                custom_filter = true;
            }
            value if value.starts_with("--filter=") => {
                opts.filters = parse_suggest_filters(&value["--filter=".len()..])?;
                custom_filter = true;
            }
            "-samples" | "--samples" => {
                i += 1;
                opts.samples = parse_usize(take_arg(args, i, "--samples")?, "sample count")?;
            }
            value if value.starts_with("--samples=") => {
                opts.samples = parse_usize(&value["--samples=".len()..], "sample count")?;
            }
            "-paths" | "--paths" => {
                i += 1;
                opts.paths = parse_usize(take_arg(args, i, "--paths")?, "path count")?;
            }
            value if value.starts_with("--paths=") => {
                opts.paths = parse_usize(&value["--paths=".len()..], "path count")?;
            }
            "-n" | "-limit" | "--limit" => {
                i += 1;
                opts.limit = parse_usize(take_arg(args, i, "--limit")?, "limit")?;
            }
            value if value.starts_with("--limit=") => {
                opts.limit = parse_usize(&value["--limit=".len()..], "limit")?;
            }
            "-seed" | "--seed" => {
                i += 1;
                opts.seed = parse_u64(take_arg(args, i, "--seed")?, "seed")?;
            }
            value if value.starts_with("--seed=") => {
                opts.seed = parse_u64(&value["--seed=".len()..], "seed")?;
            }
            "--long-lines" => {
                i += 1;
                opts.long_lines =
                    parse_usize(take_arg(args, i, "--long-lines")?, "long line threshold")?;
            }
            value if value.starts_with("--long-lines=") => {
                opts.long_lines =
                    parse_usize(&value["--long-lines=".len()..], "long line threshold")?;
            }
            "--long-words" => {
                i += 1;
                opts.long_words =
                    parse_usize(take_arg(args, i, "--long-words")?, "long word threshold")?;
            }
            value if value.starts_with("--long-words=") => {
                opts.long_words =
                    parse_usize(&value["--long-words=".len()..], "long word threshold")?;
            }
            "--short-words" => {
                i += 1;
                opts.short_words =
                    parse_usize(take_arg(args, i, "--short-words")?, "short stub threshold")?;
            }
            value if value.starts_with("--short-words=") => {
                opts.short_words =
                    parse_usize(&value["--short-words=".len()..], "short stub threshold")?;
            }
            "--duplicate-threshold" => {
                i += 1;
                opts.duplicate_threshold = parse_f64(
                    take_arg(args, i, "--duplicate-threshold")?,
                    "duplicate threshold",
                )?;
            }
            value if value.starts_with("--duplicate-threshold=") => {
                opts.duplicate_threshold = parse_f64(
                    &value["--duplicate-threshold=".len()..],
                    "duplicate threshold",
                )?;
            }
            other => common.push(other.to_string()),
        }
        i += 1;
    }
    validate_options(&opts, custom_filter)?;
    let parsed = super::parse_common(&common)?;
    if !parsed.rest.is_empty() {
        return Err("suggest does not accept positional arguments".to_string());
    }
    let vault = Vault::load(
        &parsed.root,
        Options {
            recursive: parsed.recursive,
        },
    )?;
    let report = vault.suggest(&opts)?;
    println!("{}", format_suggest_report(&vault, &report));
    Ok(())
}

fn suggest_usage() {
    eprintln!("Usage: awiki suggest [flags]");
    eprintln!("  -root <dir>                 Wiki root, default .");
    eprintln!("  -recursive, -r              Load Markdown recursively");
    eprintln!("  --filter <names>            Comma-separated filters");
    eprintln!("      sampled-diameter,wanted-pressure,long-pages,short-stubs,near-duplicates");
    eprintln!("  --samples <n>               Path samples for sampled-diameter, default 2000");
    eprintln!("  --paths <n>                 Longest sampled paths to print, default 5");
    eprintln!("  -n, --limit <n>             Rows per section, default 10");
    eprintln!("  --long-lines <n>            Long page line threshold, default 120");
    eprintln!("  --long-words <n>            Long page word threshold, default 1200");
    eprintln!("  --short-words <n>           Short stub word threshold, default 40");
    eprintln!("  --duplicate-threshold <x>   Near-duplicate score threshold, default 0.82");
}

fn validate_options(opts: &SuggestOptions, custom_filter: bool) -> Result<(), String> {
    if custom_filter && opts.filters.is_empty() {
        return Err("suggest filter must not be empty".to_string());
    }
    if opts.duplicate_threshold <= 0.0 || opts.duplicate_threshold > 1.0 {
        return Err("duplicate threshold must be in (0, 1]".to_string());
    }
    Ok(())
}

fn parse_suggest_filters(value: &str) -> Result<Vec<SuggestFilter>, String> {
    value
        .split(',')
        .filter(|v| !v.trim().is_empty())
        .map(|v| SuggestFilter::parse(v.trim()))
        .collect()
}

fn take_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(|v| v.as_str())
        .ok_or_else(|| format!("flag needs an argument: {flag}"))
}

fn parse_usize(value: &str, label: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("{label} must not be negative"))
}

fn parse_u64(value: &str, label: &str) -> Result<u64, String> {
    value.parse().map_err(|_| format!("invalid {label}"))
}

fn parse_f64(value: &str, label: &str) -> Result<f64, String> {
    value.parse().map_err(|_| format!("invalid {label}"))
}
