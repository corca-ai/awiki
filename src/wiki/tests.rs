use std::fs;

use tempfile::tempdir;

use crate::output::format_lint_report;

use super::{format_suggest_report, Options, SuggestFilter, SuggestOptions, Vault};

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

    let vault = Vault::load(dir.path().to_str().unwrap(), Options { recursive: false }).unwrap();
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
    let output = format_lint_report(&vault, &report);
    assert!(output.contains("// why: a line with only one link"));
    assert!(output.contains("// fix: add a short phrase"));
    assert!(output.contains("// example: - [[Bayes theorem]] ->"));
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
    let vault = Vault::load(dir.path().to_str().unwrap(), Options { recursive: false }).unwrap();
    let pages = vault.all_wanted_pages();
    assert_eq!(pages[0].name, "Wanted A");
    assert_eq!(pages[0].mentions, 3);
    assert_eq!(pages[1].name, "Wanted B");
}

#[test]
fn format_rewrites_markdown_safely() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Alpha.md"),
        "---\naliases: [ \"Beta\" , \"\", Beta, 안녕 ]\ntitle:  Hello world  \ntags: [wiki, \"graph stuff\"]\nother: value\n---\n\n#   Heading   ###\n\n\n* [[ Page | Alias ]]\n+ [ Label ]( ./hello.md )\n\n```\n* [[ No | Change ]]\n```\n",
    )
    .unwrap();

    let vault = Vault::load(dir.path().to_str().unwrap(), Options { recursive: false }).unwrap();
    let report = vault.format().unwrap();
    assert_eq!(report.documents, 1);
    assert_eq!(report.changed, vec!["Alpha"]);
    let formatted = fs::read_to_string(dir.path().join("Alpha.md")).unwrap();
    assert_eq!(
        formatted,
        "---\ntitle: Hello world\naliases:\n  - Beta\n  - 안녕\ntags:\n  - wiki\n  - graph stuff\nother: value\n---\n\n# Heading\n\n- [[Page|Alias]]\n- [Label](./hello.md)\n\n```\n* [[ No | Change ]]\n```\n"
    );

    let vault = Vault::load(dir.path().to_str().unwrap(), Options { recursive: false }).unwrap();
    assert!(vault.format().unwrap().changed.is_empty());
}

#[test]
fn recursive_load_ignores_generated_target_dir() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("target")).unwrap();
    fs::write(dir.path().join("Visible.md"), "Visible note.\n").unwrap();
    fs::write(
        dir.path().join("target").join("Generated.md"),
        "Generated note.\n",
    )
    .unwrap();

    let vault = Vault::load(dir.path().to_str().unwrap(), Options { recursive: true }).unwrap();
    let names: Vec<_> = vault
        .documents
        .iter()
        .map(|doc| doc.name.as_str())
        .collect();
    assert_eq!(names, vec!["Visible"]);
}

#[test]
fn suggest_reports_refactoring_candidates() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Alpha.md"),
        "Alpha connects to [[Beta]] and asks for [[Missing Idea]].\n",
    )
    .unwrap();
    fs::write(dir.path().join("Beta.md"), "Beta connects to [[Gamma]].\n").unwrap();
    fs::write(dir.path().join("Gamma.md"), "Gamma completes the chain.\n").unwrap();
    fs::write(
        dir.path().join("Long.md"),
        "One.\nTwo.\nThree.\nFour.\nFive.\n",
    )
    .unwrap();
    fs::write(dir.path().join("Stub.md"), "Tiny note.\n").unwrap();
    let duplicate = "This page repeats a focused explanation about graph gardening, wiki structure, duplicated knowledge, and refactoring candidates so that the duplicate detector has enough shared text to compare reliably.\n";
    fs::write(dir.path().join("Duplicate A.md"), duplicate).unwrap();
    fs::write(dir.path().join("Duplicate B.md"), duplicate).unwrap();

    let vault = Vault::load(dir.path().to_str().unwrap(), Options { recursive: false }).unwrap();
    let report = vault
        .suggest(&SuggestOptions {
            filters: SuggestFilter::ALL.to_vec(),
            samples: 20,
            paths: 2,
            limit: 5,
            seed: 1,
            long_lines: 4,
            long_words: 80,
            short_words: 3,
            duplicate_threshold: 0.8,
        })
        .unwrap();

    assert!(!report.sampled_diameter.unwrap().paths.is_empty());
    assert_eq!(report.wanted_pressure.unwrap()[0].name, "Missing Idea");
    assert!(report
        .long_pages
        .unwrap()
        .pages
        .iter()
        .any(|hit| vault.documents[hit.doc].name == "Long"));
    assert!(report
        .short_stubs
        .unwrap()
        .pages
        .iter()
        .any(|hit| vault.documents[hit.doc].name == "Stub"));
    assert!(report.near_duplicates.unwrap().pairs.iter().any(|pair| {
        let a = &vault.documents[pair.a].name;
        let b = &vault.documents[pair.b].name;
        (a == "Duplicate A" && b == "Duplicate B") || (a == "Duplicate B" && b == "Duplicate A")
    }));

    let output = format_suggest_report(&vault, &vault.suggest(&SuggestOptions::default()).unwrap());
    assert!(output.contains("// sampled_diameter"));
    assert!(output.contains("// wanted_pressure"));
    assert!(output.contains("// near_duplicates"));
    assert!(output.contains("// why: these sampled paths are long"));
    assert!(output.contains("// fix: create the page, correct misspelled links"));
    assert!(output.contains("// example: split \"Machine learning\""));
}
