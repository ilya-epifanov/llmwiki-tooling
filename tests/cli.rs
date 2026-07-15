use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_llmwiki-tool"))
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn write_repo_wide_fixture(root: &Path) {
    std::fs::create_dir(root.join("topics")).unwrap();
    std::fs::create_dir(root.join("resources")).unwrap();
    std::fs::create_dir(root.join("drafts")).unwrap();
    std::fs::write(
        root.join("wiki.toml"),
        r#"index = "index.md"
verbatim = ["drafts/"]

[[directories]]
path = "topics"

[[directories]]
path = "resources"
"#,
    )
    .unwrap();
    std::fs::write(root.join("index.md"), "# Index\n").unwrap();
    std::fs::write(
        root.join("topics/Example Concept.md"),
        "# Example Concept\n",
    )
    .unwrap();
    std::fs::write(
        root.join("resources/Example Resource.md"),
        "# Example Resource\n",
    )
    .unwrap();
    std::fs::write(
        root.join("overview.md"),
        "Example Resource references [[Example Concept]] and [[Missing Page]].\n",
    )
    .unwrap();
    std::fs::write(
        root.join("drafts/ignored.md"),
        "Example Resource [[Missing Page]]\n",
    )
    .unwrap();
}

#[test]
fn repo_wide_commands_scan_loose_notes_and_skip_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    write_repo_wide_fixture(dir.path());

    let refs = run(dir.path(), &["refs", "to", "Example Concept"]);
    assert!(refs.status.success(), "{}", text(&refs.stderr));
    let stdout = text(&refs.stdout);
    assert!(
        stdout.contains("overview.md -> Example Concept"),
        "{stdout}"
    );
    assert!(!stdout.contains("drafts/ignored.md"), "{stdout}");

    let broken = run(dir.path(), &["links", "broken"]);
    assert!(!broken.status.success());
    let stdout = text(&broken.stdout);
    assert!(stdout.contains("overview.md"), "{stdout}");
    assert!(!stdout.contains("drafts/ignored.md"), "{stdout}");

    let check = run(dir.path(), &["links", "check"]);
    assert!(check.status.success(), "{}", text(&check.stderr));
    let stdout = text(&check.stdout);
    assert!(
        stdout.contains("overview.md:1:1: bare mention \"Example Resource\""),
        "{stdout}"
    );
    assert!(!stdout.contains("drafts/ignored.md"), "{stdout}");

    let rename = run(
        dir.path(),
        &["rename", "Example Concept", "Renamed Concept"],
    );
    assert!(rename.status.success(), "{}", text(&rename.stderr));
    let stdout = text(&rename.stdout);
    assert!(stdout.contains("--- overview.md"), "{stdout}");
    assert!(stdout.contains("[[Renamed Concept]]"), "{stdout}");
    assert!(!stdout.contains("drafts/ignored.md"), "{stdout}");
}

#[test]
fn rename_collapses_alias_when_alias_matches_new_name() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("topics")).unwrap();
    std::fs::write(
        dir.path().join("wiki.toml"),
        r#"index = "index.md"

[[directories]]
path = "topics"
"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("topics/old-topic.md"), "# Old Topic\n").unwrap();
    std::fs::write(
        dir.path().join("index.md"),
        "See [[old-topic|New Topic]].\n",
    )
    .unwrap();

    let output = run(dir.path(), &["rename", "old-topic", "New Topic"]);
    assert!(output.status.success(), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert!(stdout.contains("+See [[New Topic]]."), "{stdout}");
    assert!(!stdout.contains("[[New Topic|New Topic]]"), "{stdout}");
}

#[test]
fn rename_updates_relative_markdown_links() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("topics")).unwrap();
    std::fs::write(
        dir.path().join("wiki.toml"),
        r#"index = ""

[[directories]]
path = "topics"
"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("topics/old-topic.md"), "# Topic\n").unwrap();
    std::fs::write(
        dir.path().join("overview.md"),
        "See [topic](topics/old-topic.md).\n",
    )
    .unwrap();

    let output = run(dir.path(), &["rename", "old-topic", "new-topic", "--write"]);
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(!dir.path().join("topics/old-topic.md").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("overview.md")).unwrap(),
        "See [topic](topics/new-topic.md).\n"
    );
}

#[test]
fn loose_pages_and_obsidian_aliases_are_link_targets() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("topics")).unwrap();
    std::fs::write(
        dir.path().join("wiki.toml"),
        r#"index = "index.md"

[[directories]]
path = "topics"
"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("index.md"), "[[Short Name]]\n").unwrap();
    std::fs::write(
        dir.path().join("topics/Canonical Page.md"),
        "---\naliases: [Short Name]\n---\n\n# Canonical Page\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("loose-note.md"), "# Loose\n").unwrap();
    std::fs::write(
        dir.path().join("overview.md"),
        "See [[Short Name]] and [[loose-note]].\n",
    )
    .unwrap();

    let broken = run(dir.path(), &["links", "broken"]);
    assert!(broken.status.success(), "{}", text(&broken.stdout));

    let refs = run(dir.path(), &["refs", "to", "Canonical Page"]);
    assert!(refs.status.success(), "{}", text(&refs.stderr));
    assert!(text(&refs.stdout).contains("overview.md -> Canonical Page"));

    let refs = run(dir.path(), &["refs", "to", "Short Name"]);
    assert!(refs.status.success(), "{}", text(&refs.stderr));
    assert!(text(&refs.stdout).contains("overview.md -> Short Name"));

    let orphans = run(dir.path(), &["links", "orphans"]);
    assert!(orphans.status.success(), "{}", text(&orphans.stderr));
    assert!(!text(&orphans.stdout).contains("Canonical Page.md"));
}

#[test]
fn markdown_links_are_first_class_graph_edges() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("topics")).unwrap();
    std::fs::write(
        dir.path().join("wiki.toml"),
        "index = \"\"\n\n[[directories]]\npath = \"topics\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("topics/Target Page.md"),
        "# Target Page\n\n## Key Findings\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("overview.md"),
        "See [Target Page](topics/Target%20Page.md#key-findings) and [reference][Target Page].\n\n[Target Page]: topics/Target%20Page.md\n",
    )
    .unwrap();

    let broken = run(dir.path(), &["links", "broken"]);
    assert!(broken.status.success(), "{}", text(&broken.stdout));

    let refs = run(dir.path(), &["refs", "to", "Target Page"]);
    assert!(refs.status.success(), "{}", text(&refs.stderr));
    assert!(text(&refs.stdout).contains("overview.md -> Target Page"));

    let orphans = run(dir.path(), &["links", "orphans"]);
    assert!(!text(&orphans.stdout).contains("Target Page.md"));

    let check = run(dir.path(), &["links", "check"]);
    assert!(check.status.success(), "{}", text(&check.stderr));
    assert!(text(&check.stdout).is_empty(), "{}", text(&check.stdout));
}

#[test]
fn unmanaged_broken_links_warn_without_failing_lint() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("topics")).unwrap();
    std::fs::write(
        dir.path().join("wiki.toml"),
        r#"index = ""

[checks]
orphan_pages = "off"
index_coverage = "off"

[[directories]]
path = "topics"
"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("topics/Managed.md"), "# Managed\n").unwrap();
    std::fs::write(dir.path().join("loose.md"), "See [[Missing Page]].\n").unwrap();

    let output = run(dir.path(), &["lint"]);
    assert!(output.status.success(), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("warn[broken-link]"), "{stderr}");
    assert!(stderr.contains("loose.md"), "{stderr}");
    assert!(stderr.contains("1 warning(s)"), "{stderr}");
}

#[test]
fn conditional_rules_apply_by_frontmatter_predicate() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("items")).unwrap();
    std::fs::write(
        dir.path().join("wiki.toml"),
        r#"index = ""

[checks]
broken_links = "off"
orphan_pages = "off"
index_coverage = "off"

[[directories]]
path = "items"

[[rules]]
check = "required-frontmatter"
when = "type == concept"
fields = ["owner"]
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("items/Concept.md"),
        "---\ntype: concept\n---\n\n# Concept\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("items/Resource.md"),
        "---\ntype: resource\n---\n\n# Resource\n",
    )
    .unwrap();

    let output = run(dir.path(), &["lint"]);
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("items/Concept.md is missing 'owner'"),
        "{stderr}"
    );
    assert!(!stderr.contains("items/Resource.md"), "{stderr}");
}

#[test]
fn move_rebases_relative_markdown_links() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("topics")).unwrap();
    std::fs::create_dir(dir.path().join("references")).unwrap();
    std::fs::write(
        dir.path().join("wiki.toml"),
        r#"index = ""

[[directories]]
path = "topics"
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("topics/Topic.md"),
        "See [source](../references/source.md).\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("references/source.md"), "# Source\n").unwrap();
    std::fs::write(
        dir.path().join("overview.md"),
        "See [topic](topics/Topic.md) and [again][topic].\n\n[topic]: topics/Topic.md\n",
    )
    .unwrap();

    let dry_run = run(dir.path(), &["move", "Topic", "archive/topics"]);
    assert!(dry_run.status.success(), "{}", text(&dry_run.stderr));
    let stdout = text(&dry_run.stdout);
    assert!(
        stdout.contains("topics/Topic.md -> archive/topics/Topic.md"),
        "{stdout}"
    );
    assert!(
        stdout.contains("+See [topic](archive/topics/Topic.md) and [again][topic]."),
        "{stdout}"
    );
    assert!(
        stdout.contains("+[topic]: archive/topics/Topic.md"),
        "{stdout}"
    );
    assert!(
        stdout.contains("+See [source](../../references/source.md)."),
        "{stdout}"
    );

    let output = run(dir.path(), &["move", "Topic", "archive/topics", "--write"]);
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(!dir.path().join("topics/Topic.md").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("archive/topics/Topic.md")).unwrap(),
        "See [source](../../references/source.md).\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("overview.md")).unwrap(),
        "See [topic](archive/topics/Topic.md) and [again][topic].\n\n[topic]: archive/topics/Topic.md\n"
    );
}

#[test]
fn frontmatter_set_creates_block_and_missing_get_fails() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("wiki.toml"), "index = \"\"\n").unwrap();
    std::fs::write(dir.path().join("note.md"), "# Note\n").unwrap();

    let no_frontmatter = run(dir.path(), &["frontmatter", "get", "note.md", "owner"]);
    assert!(!no_frontmatter.status.success());
    assert!(text(&no_frontmatter.stderr).contains("no frontmatter"));

    let set = run(
        dir.path(),
        &["frontmatter", "set", "note.md", "owner", "alice"],
    );
    assert!(set.status.success(), "{}", text(&set.stderr));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("note.md")).unwrap(),
        "---\nowner: alice\n---\n\n# Note\n"
    );

    let get = run(dir.path(), &["frontmatter", "get", "note.md", "owner"]);
    assert!(get.status.success(), "{}", text(&get.stderr));
    assert_eq!(text(&get.stdout).trim(), "\"alice\"");

    let missing = run(dir.path(), &["frontmatter", "get", "note.md", "missing"]);
    assert!(!missing.status.success());
    assert!(text(&missing.stderr).contains("field 'missing' not found"));
}

#[test]
fn version_flag_prints_package_version() {
    let output = Command::new(bin()).arg("--version").output().unwrap();
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains(env!("CARGO_PKG_VERSION")));
}
