use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_awiki"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn text(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Target.md"), "A target page.\n").unwrap();
    fs::write(dir.path().join("awiki.toml"), "[lint.rules]\norphan='off'\nisland='off'\nlink-only-line='off'\nbroken-document-link='error'\n").unwrap();
    dir
}

#[test]
fn reference_links_share_graph_and_rename_semantics() {
    let dir = fixture();
    fs::write(
        dir.path().join("Source.md"),
        "See [target][page] and [page][].\n\n[page]: Target.md#Heading \"Title\"\n",
    )
    .unwrap();
    let graph = run(dir.path(), &["graph", "-json"]);
    assert!(
        text(&graph).contains("\"source\":\"Source\",\"target\":\"Target\""),
        "{}",
        text(&graph)
    );
    let renamed = run(dir.path(), &["rename", "Target", "New target"]);
    assert!(
        renamed.status.success(),
        "{}",
        String::from_utf8_lossy(&renamed.stderr)
    );
    let body = fs::read_to_string(dir.path().join("Source.md")).unwrap();
    assert!(body.contains("<New target.md#Heading> \"Title\""), "{body}");
    assert!(run(dir.path(), &["lint"]).status.success());
}

#[test]
fn comments_and_code_examples_are_not_links_or_rewrite_targets() {
    let dir = fixture();
    let body = "Summary.\n\n<!-- [ghost](Missing.md) [[Missing]] -->\n\n    [ghost](Missing.md)\n\n`[[Missing]]`\n\nSee [real](Target.md).\n";
    fs::write(dir.path().join("Source.md"), body).unwrap();
    let out = run(dir.path(), &["lint"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(run(dir.path(), &["rename", "Target", "Renamed"])
        .status
        .success());
    let body = fs::read_to_string(dir.path().join("Source.md")).unwrap();
    assert!(body.contains("<!-- [ghost](Missing.md) [[Missing]] -->"));
    assert!(body.contains("    [ghost](Missing.md)"));
}

#[test]
fn json_diagnostics_have_stable_fields_numeric_positions_and_exit_codes() {
    let dir = fixture();
    fs::write(
        dir.path().join("Source.md"),
        "Summary.\nSee [[Missing2]].\n\n\n\n\n\n\n\nSee [[Missing10]].\n",
    )
    .unwrap();
    let out = run(dir.path(), &["lint", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["errors"], 2);
    assert_eq!(value["diagnostics"][0]["line"], 2);
    assert_eq!(value["diagnostics"][1]["line"], 10);
    assert_eq!(value["diagnostics"][0]["column"], 5);
    assert_eq!(value["diagnostics"][0]["rule"], "broken-document-link");
    assert_eq!(value["diagnostics"][0]["severity"], "error");
    let config = fs::read_to_string(dir.path().join("awiki.toml"))
        .unwrap()
        .replace(
            "broken-document-link='error'",
            "broken-document-link='warn'",
        );
    fs::write(dir.path().join("awiki.toml"), config).unwrap();
    let out = run(dir.path(), &["lint", "-json"]);
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["errors"], 0);
    assert_eq!(value["warnings"], 2);
}

#[test]
fn invalid_config_uses_stderr_and_legacy_json_uses_default_policy() {
    let dir = fixture();
    fs::write(dir.path().join("awiki.toml"), "[lint.rules]\nunknown='off'").unwrap();
    let out = run(dir.path(), &["lint", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown lint rule"));
    fs::remove_file(dir.path().join("awiki.toml")).unwrap();
    let out = run(dir.path(), &["lint", "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["diagnostics"][0]["rule"], "orphan");
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn dry_runs_leave_names_contents_and_directories_unchanged() {
    let dir = fixture();
    let before = "Summary with [[Target]].  \n\n* Item\n";
    fs::write(dir.path().join("한글 문서.md"), before).unwrap();
    let out = run(dir.path(), &["format", "--dry-run"]);
    assert!(out.status.success());
    assert!(text(&out).contains("dry_run"));
    assert_eq!(
        fs::read_to_string(dir.path().join("한글 문서.md")).unwrap(),
        before
    );
    let out = run(
        dir.path(),
        &[
            "rename",
            "-recursive",
            "--dry-run",
            "Target",
            "new directory/New target",
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.path().join("Target.md").exists());
    assert!(!dir.path().join("new directory").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("한글 문서.md")).unwrap(),
        before
    );
}

#[test]
fn format_is_idempotent_and_preserves_link_graph_code_comments_and_images() {
    let dir = fixture();
    let body = "Summary.  \n\n* See [ rich `label` ]( Target.md ) and [[ Target | label ]].\n\n<!-- [[ Target | comment ]] -->\n\n    * [[ Target | example ]]\n\n```md\n+ [ code ]( Target.md )  \n```\n\n![ image ]( Target.md )\n";
    fs::write(dir.path().join("문서 A.md"), body).unwrap();
    let before: serde_json::Value =
        serde_json::from_slice(&run(dir.path(), &["graph", "-json"]).stdout).unwrap();
    assert!(run(dir.path(), &["format"]).status.success());
    let once = fs::read_to_string(dir.path().join("문서 A.md")).unwrap();
    assert!(once.contains("<!-- [[ Target | comment ]] -->"));
    assert!(once.contains("    * [[ Target | example ]]"));
    assert!(once.contains("+ [ code ]( Target.md )  "));
    assert!(once.contains("![ image ]( Target.md )"));
    let twice = run(dir.path(), &["format"]);
    assert!(twice.status.success());
    assert!(text(&twice).contains("changed=0"));
    assert_eq!(
        fs::read_to_string(dir.path().join("문서 A.md")).unwrap(),
        once
    );
    let after: serde_json::Value =
        serde_json::from_slice(&run(dir.path(), &["graph", "-json"]).stdout).unwrap();
    assert_eq!(before, after);
}

#[test]
fn moving_a_document_preserves_inbound_and_outbound_relative_edges() {
    let dir = fixture();
    fs::create_dir(dir.path().join("a")).unwrap();
    fs::write(
        dir.path().join("a/Original.md"),
        "See [target](../Target.md) and [[../Target]].\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("Source.md"),
        "See [original](a/Original.md).\n",
    )
    .unwrap();
    let out = run(
        dir.path(),
        &["rename", "-recursive", "a/Original", "b/deep/New name"],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body = fs::read_to_string(dir.path().join("b/deep/New name.md")).unwrap();
    assert!(body.contains("../../Target.md"), "{body}");
    let graph: serde_json::Value =
        serde_json::from_slice(&run(dir.path(), &["graph", "-recursive", "-json"]).stdout).unwrap();
    assert_eq!(graph["edges"].as_array().unwrap().len(), 2);
    assert!(graph["edges"]
        .as_array()
        .unwrap()
        .iter()
        .all(|edge| edge["missing"] == false));
}

#[test]
fn a_reference_shared_with_an_image_keeps_the_image_target() {
    let dir = fixture();
    fs::write(
        dir.path().join("Source.md"),
        "See [target][shared].\n\n![image][shared]\n\n[shared]: Target.md \"Title\"\n",
    )
    .unwrap();
    assert!(run(dir.path(), &["rename", "Target", "New name"])
        .status
        .success());
    let body = fs::read_to_string(dir.path().join("Source.md")).unwrap();
    assert!(body.contains("[shared]: Target.md \"Title\""), "{body}");
    assert!(body.contains("[target](<New name.md> \"Title\")"), "{body}");
    assert!(run(dir.path(), &["lint"]).status.success());
}

#[test]
fn prepare_failure_does_not_partially_format_other_documents() {
    let dir = fixture();
    let path = dir.path().join("Z.md");
    fs::write(dir.path().join("A.md"), "Summary.  \n").unwrap();
    fs::write(&path, "Readonly.  \n").unwrap();
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let out = run(dir.path(), &["format"]);
    fs::set_permissions(&path, original).unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(dir.path().join("A.md")).unwrap(),
        "Summary.  \n"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "Readonly.  \n");
}

#[test]
fn rich_links_frontmatter_and_escaped_wikilinks_keep_the_same_targets() {
    let dir = fixture();
    fs::write(dir.path().join("Source.md"), "---\nrelated: [[Target]]\n---\nSummary with [nested [label]](Target.md).\n\n| Column |\n| --- |\n| [[Target\\|Label]] |\n\n\\[[Missing]]\n").unwrap();
    assert!(run(dir.path(), &["lint"]).status.success());
    assert!(run(dir.path(), &["rename", "Target", "New target"])
        .status
        .success());
    assert!(run(dir.path(), &["lint"]).status.success());
    let body = fs::read_to_string(dir.path().join("Source.md")).unwrap();
    assert!(body.contains("related: [[New target]]"), "{body}");
    assert!(body.contains("[[New target\\|Label]]"), "{body}");
    assert!(body.contains("[nested [label]](<New target.md>)"), "{body}");
    assert!(body.contains("\\[[Missing]]"));
}

#[test]
fn code_reference_cache_is_fresh_for_each_command_and_ignores_frontmatter() {
    let dir = fixture();
    fs::write(
        dir.path().join("model.ts"),
        "export class Foo { method() {} }",
    )
    .unwrap();
    fs::write(dir.path().join("Source.md"), "---\nexample: '[unused](missing.ts#Ghost)'\n---\nSummary with [`Foo.method()`](model.ts#Foo.method).\n").unwrap();
    fs::write(
        dir.path().join("awiki.toml"),
        "[lint.rules]\norphan='off'\nisland='off'\nbroken-code-reference='error'\n",
    )
    .unwrap();
    let out = run(dir.path(), &["lint"]);
    assert!(out.status.success(), "{}", text(&out));
    fs::write(dir.path().join("model.ts"), "export class Foo {}").unwrap();
    let out = run(dir.path(), &["lint"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out).contains("declaration not found: Foo.method"));
}

#[test]
fn moving_a_document_keeps_relative_code_references_valid() {
    let dir = fixture();
    fs::create_dir(dir.path().join("a")).unwrap();
    fs::write(dir.path().join("model.ts"), "export class Foo {}").unwrap();
    fs::write(
        dir.path().join("a/Note.md"),
        "Summary with [Foo](../model.ts#Foo).\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("awiki.toml"),
        "[lint.rules]\norphan='off'\nbroken-code-reference='error'\n",
    )
    .unwrap();
    assert!(run(
        dir.path(),
        &["rename", "-recursive", "a/Note", "b/deep/Note"]
    )
    .status
    .success());
    let out = run(dir.path(), &["lint", "-recursive"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(fs::read_to_string(dir.path().join("b/deep/Note.md"))
        .unwrap()
        .contains("../../model.ts#Foo"));
}
