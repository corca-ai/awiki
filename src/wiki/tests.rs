use std::fs;

use tempfile::tempdir;

use super::{Options, Vault};

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
