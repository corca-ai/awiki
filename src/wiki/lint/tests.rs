use super::super::Options;
use super::*;

fn run(body: &str, policy: &str) -> Diagnostics {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Note.md"), body).unwrap();
    fs::write(dir.path().join("awiki.toml"), policy).unwrap();
    Vault::load(dir.path().to_str().unwrap(), Options { recursive: false })
        .unwrap()
        .configured_lint()
        .unwrap()
        .unwrap()
}

const BASE: &str = "[lint.rules]\norphan = 'off'\nlink-only-line = 'off'\n";

#[test]
fn undefined_explicit_references_are_reported_without_treating_text_as_links() {
    let config = format!("{BASE}broken-reference-link = 'error'");
    let body = "Summary.\n\n[missing][absent] ![image][no-image] [collapsed][] [ordinary]\n\n`[code][skip]`\n\n[defined][later]\n\n[later]: Target.md\n";
    let result = run(body, &config);
    let failures: Vec<_> = result
        .items
        .iter()
        .filter(|item| item.rule == "broken-reference-link")
        .collect();
    assert_eq!(failures.len(), 3, "{:#?}", result.items);
    assert!(failures.iter().all(|item| item.line == 3));
    assert!(run(body, BASE)
        .items
        .iter()
        .all(|item| item.rule != "broken-reference-link"));
}

#[test]
fn local_resources_are_checked_without_changing_the_wiki_graph() {
    let dir = tempfile::tempdir().unwrap();
    let docs = dir.path().join("docs");
    fs::create_dir(&docs).unwrap();
    fs::write(dir.path().join("outside file.json"), "{}").unwrap();
    fs::write(dir.path().join("outside.md"), "Outside.\n").unwrap();
    fs::write(
        docs.join("awiki.toml"),
        format!("{BASE}broken-document-link = 'error'\nbroken-local-link = 'error'\n"),
    )
    .unwrap();
    fs::write(docs.join("Note.md"), "Summary.\n\n[ok](../outside%20file.json?view=1) [outside](../outside.md)\n![lost](missing.png) [lost](missing.md) [[unknown]]\n\n`![ignored](code.png)`\n\n[web](https://invalid.example/x) <person@example.org>\n").unwrap();
    let vault = Vault::load(docs.to_str().unwrap(), Options { recursive: false }).unwrap();
    let result = vault.configured_lint().unwrap().unwrap();
    let failures: Vec<_> = result
        .items
        .iter()
        .map(|item| (item.rule.as_str(), item.line))
        .collect();
    assert_eq!(
        failures,
        vec![
            ("broken-local-link", 4),
            ("broken-local-link", 4),
            ("broken-document-link", 4)
        ]
    );
}

#[test]
fn local_markdown_fragments_follow_headings_and_duplicate_suffixes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("Target.md"),
        "Summary.\n\n# Hello `world`\n\n## Duplicate\n\n## Duplicate\n\n## 한글\n",
    )
    .unwrap();
    fs::write(dir.path().join("Note.md"), "Summary.\n\n# Local\n\n[one](Target.md?view=1#hello-world) [two](Target.md#duplicate-1) [unicode](Target.md#%ED%95%9C%EA%B8%80) [self](#local)\n\n[missing](Target.md#no-such-heading) [missing self](#absent)\n").unwrap();
    fs::write(
        dir.path().join("awiki.toml"),
        format!("{BASE}broken-local-link = 'error'\nbroken-code-reference = 'error'\n"),
    )
    .unwrap();
    let vault = Vault::load(dir.path().to_str().unwrap(), Options { recursive: false }).unwrap();
    let result = vault.configured_lint().unwrap().unwrap();
    let failures: Vec<_> = result
        .items
        .iter()
        .map(|item| (item.rule.as_str(), item.line))
        .collect();
    assert_eq!(
        failures,
        vec![("broken-local-link", 7), ("broken-local-link", 7)]
    );
}

#[test]
fn local_link_checks_do_not_weaken_graph_or_duplicate_source_errors() {
    let body = "---\nsummary: '[front](missing-front.md)'\n---\n\nSummary.\n\n[missing](missing.md) [source](missing.ts#Thing)\n";
    let severity = format!("{BASE}broken-document-link = 'error'\nbroken-local-link = 'warn'\n");
    let result = run(body, &severity);
    assert!(result
        .items
        .iter()
        .any(|item| item.rule == "broken-document-link" && item.line == 2));
    assert!(result
        .items
        .iter()
        .any(|item| item.rule == "broken-document-link"
            && item.line == 7
            && item.severity == Severity::Error));
    let both = format!("{BASE}broken-code-reference = 'error'\nbroken-local-link = 'error'\n");
    let result = run("Summary.\n\n[source](missing.ts#Thing)\n", &both);
    assert_eq!(
        result
            .items
            .iter()
            .map(|item| item.rule.as_str())
            .collect::<Vec<_>>(),
        vec!["broken-code-reference"]
    );
    let mixed = format!("{BASE}broken-code-reference = 'warn'\nbroken-local-link = 'error'\n");
    let result = run("Summary.\n\n[source](missing.ts#Thing)\n", &mixed);
    assert!(result.has_errors(), "{:?}", result.items);
    let mixed_html = format!("{BASE}broken-code-reference = 'error'\nbroken-local-link = 'warn'\n");
    let result = run("Summary.\n\n[page](missing.html#part)\n", &mixed_html);
    assert!(result.has_errors(), "{:?}", result.items);
}

#[test]
fn local_checks_allow_source_fragments_at_independent_severities() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("sample.ts"), "export function run() {}\n").unwrap();
    fs::write(
        dir.path().join("Note.md"),
        "Summary.\n\n[valid](sample.ts#run) [missing](sample.ts#absent)\n",
    )
    .unwrap();
    for (severity, expected_warnings) in [("off", 0), ("warn", 1)] {
        fs::write(
            dir.path().join("awiki.toml"),
            format!("{BASE}broken-code-reference = '{severity}'\nbroken-local-link = 'error'\n"),
        )
        .unwrap();
        let result = Vault::load(dir.path().to_str().unwrap(), Options { recursive: false })
            .unwrap()
            .configured_lint()
            .unwrap()
            .unwrap();
        assert!(!result.has_errors(), "{:?}", result.items);
        assert_eq!(result.items.len(), expected_warnings);
    }
}

#[test]
fn paragraph_is_a_block_not_a_line() {
    let config = format!("{BASE}first-block-paragraph = 'error'");
    for body in [
        "Summary.\nContinued.\n",
        "---\ntitle: Note\n---\n<!-- hidden -->\n\nSummary.",
    ] {
        assert!(!run(body, &config).has_errors(), "{body}");
    }
    for body in [
        "",
        "# Heading\n",
        "Heading\n===\n",
        "- Item\n",
        "> Quote\n",
        "```ts\nx\n```\n",
        "<div>HTML</div>\n",
    ] {
        assert!(run(body, &config).has_errors(), "{body}");
    }
}

#[test]
fn severity_overrides_and_exclusions() {
    assert!(!run(
        "# Heading",
        &format!("{BASE}first-block-paragraph = 'warn'")
    )
    .has_errors());
    let config = format!("{BASE}first-block-paragraph = 'error'\n[[lint.overrides]]\nfiles = ['*.md']\n[lint.overrides.rules]\nfirst-block-paragraph = 'off'");
    assert!(!run("# Heading", &config).has_errors());
    assert!(run("# Heading", "[lint]\nexclude = ['Note.md']")
        .items
        .is_empty());
    let config = format!("{BASE}broken-document-link = 'error'");
    let result = run("Summary.\n\nSee [[Absent.v1]].\n", &config);
    assert!(result.has_errors());
    assert!(result.items[0].to_string().starts_with("Note.md:3:"));
    assert!(!run("Summary with [code](file.ts#Foo).", &config).has_errors());
}

#[test]
fn symbols_are_declarations_with_exact_scopes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Foo.ts");
    fs::write(&path, "// missing\nexport function blah() { const local = 1; }\nexport class Foo { blah() {} value: string = ''; }\ninterface Bar { method(): void; }\nconst arrow = () => 1;").unwrap();
    for name in [
        "blah",
        "Foo",
        "Foo.blah",
        "Foo.value",
        "Bar.method",
        "arrow",
    ] {
        assert!(check_symbol(&path, name).is_ok(), "{name}");
    }
    for name in ["missing", "local", "Foo.missing", "foo", "blah()"] {
        assert!(check_symbol(&path, name).is_err(), "{name}");
    }
    fs::write(&path, "class {").unwrap();
    assert!(check_symbol(&path, "Foo").unwrap_err().contains("syntax"));
}

#[test]
fn source_references_resolve_outside_wiki_and_ignore_examples() {
    let dir = tempfile::tempdir().unwrap();
    let docs = dir.path().join("docs");
    fs::create_dir(&docs).unwrap();
    fs::write(dir.path().join("Foo.ts"), "export class Foo { blah() {} }").unwrap();
    fs::write(
        docs.join("awiki.toml"),
        format!("{BASE}broken-code-reference = 'error'"),
    )
    .unwrap();
    let note = docs.join("Note.md");
    fs::write(
        &note,
        "Summary.\n\n[`Foo.blah()`](../Foo.ts#Foo.blah)\n\n```md\n[x](../Foo.ts#missing)\n```\n",
    )
    .unwrap();
    let load = || Vault::load(docs.to_str().unwrap(), Options { recursive: false }).unwrap();
    assert!(!load().configured_lint().unwrap().unwrap().has_errors());
    assert!(load().all_wanted_pages().is_empty());
    fs::write(&note, "Summary.\n\n[x](../Foo.ts#missing)").unwrap();
    let result = load().configured_lint().unwrap().unwrap();
    assert!(result.has_errors());
    assert!(result.items[0].to_string().starts_with("Note.md:3:"));
}

#[test]
fn invalid_policy_fails_instead_of_silently_ignoring_typos() {
    for source in [
        "[lint.rules]\ntypo = 'off'",
        "[lint.rules]\norphan = 'warning'",
        "[lint]\nexclude = ['[']",
        "[lint]\nunknown = true",
    ] {
        let config = toml::from_str::<Config>(source);
        assert!(config.is_err() || config.unwrap().lint.validate().is_err());
    }
}

#[test]
fn tsx_and_source_failures_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("View.tsx");
    fs::write(&path, "export function View() { return <div>Hello</div>; }").unwrap();
    assert!(check_symbol(&path, "View").is_ok());
    let unsupported = dir.path().join("View.py");
    fs::write(&unsupported, "class View: pass").unwrap();
    assert!(check_symbol(&unsupported, "View")
        .unwrap_err()
        .contains("unsupported"));
    fs::remove_file(&path).unwrap();
    assert!(check_symbol(&path, "View")
        .unwrap_err()
        .contains("cannot read"));
}

#[test]
fn absent_config_keeps_legacy_policy() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Note.md"), "# Heading\n\nSee [[Absent]].").unwrap();
    let vault = Vault::load(dir.path().to_str().unwrap(), Options { recursive: false }).unwrap();
    assert!(vault.configured_lint().unwrap().is_none());
    assert!(vault.lint().has_issues());
}

#[test]
fn rust_references_follow_direct_declarations_and_impl_members() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wiki.rs");
    fs::write(
        &path,
        r#"
        // ghost
        pub struct Vault<T> { pub name: T }
        impl<T> Vault<T> {
            pub const LIMIT: usize = 1;
            pub fn load() { fn local() {} }
        }
        trait Read { type Item; fn read(&self); }
        impl<T> Read for Vault<T> { type Item = T; fn read(&self) {} }
        enum State { Ready, Failed { reason: String } }
        type Alias = State;
        const LIMIT: usize = 2;
        mod inner { pub fn helper() {} }
        mod elsewhere;
        pub use elsewhere::Imported;
        macro_rules! make { () => { fn generated() {} }; }
        fn run() { fn nested() {} }
    "#,
    )
    .unwrap();
    for name in [
        "Vault",
        "Vault::name",
        "Vault::load",
        "Vault::LIMIT",
        "Vault::read",
        "Read::Item",
        "Read::read",
        "State::Ready",
        "State::Failed::reason",
        "Alias",
        "LIMIT",
        "inner::helper",
        "elsewhere",
        "make",
        "run",
    ] {
        assert!(check_symbol(&path, name).is_ok(), "{name}");
    }
    for name in [
        "ghost",
        "local",
        "nested",
        "Vault::local",
        "Vault.load",
        "Imported",
        "generated",
        "helper",
        "elsewhere::Imported",
    ] {
        assert!(check_symbol(&path, name).is_err(), "{name}");
    }
}

#[test]
fn repository_docs_use_rust_references_without_entering_wiki_graph() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("model.rs"),
        "pub struct Vault; impl Vault { pub fn load() {} }",
    )
    .unwrap();
    fs::write(
        dir.path().join("Note.md"),
        "This wiki is loaded by [`Vault::load()`](model.rs#Vault::load).\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("awiki.toml"),
        format!("{BASE}broken-code-reference = 'error'"),
    )
    .unwrap();
    let load = || Vault::load(dir.path().to_str().unwrap(), Options { recursive: false }).unwrap();
    assert!(!load().configured_lint().unwrap().unwrap().has_errors());
    assert!(load().all_wanted_pages().is_empty());
    fs::write(dir.path().join("model.rs"), "pub struct Vault;").unwrap();
    assert!(load().configured_lint().unwrap().unwrap().has_errors());
}

fn check_symbol(path: &std::path::Path, symbol: &str) -> Result<(), String> {
    SourceCache::default().check(path, symbol)
}
