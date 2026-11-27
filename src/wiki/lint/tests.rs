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
