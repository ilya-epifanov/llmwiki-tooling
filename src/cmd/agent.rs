use std::collections::HashMap;

use crate::config::IgnoreConfig;
use crate::error::WikiError;
use crate::frontmatter;
use crate::parse;
use crate::walk::{is_markdown_file, wiki_walk_builder};
use crate::wiki::WikiRoot;

use super::{DirStats, detect_mirror_candidates};

/// Output the setup workflow prompt for an LLM agent.
pub fn setup(root: &WikiRoot) -> Result<(), WikiError> {
    let has_config = root.path().join("wiki.toml").is_file();
    let version = env!("CARGO_PKG_VERSION");

    print!(
        r#"## Wiki Tool Setup (v{version})

You are configuring a wiki for use with the `wiki` CLI tool.

### Discover available commands

Run `wiki --help` to see all top-level commands.
Run `wiki <command> --help` for subcommand details (e.g., `wiki links --help`).

"#
    );

    if has_config {
        print!(
            r#"### wiki.toml already exists

A wiki.toml is present at the wiki root. Skip to validation.

1. Read the existing wiki.toml to understand the current configuration.
2. Run `wiki scan` to see the actual wiki structure and check for mismatches.
3. Run `wiki lint` and iterate:
   - Real content problem -> fix the wiki content
   - Config too strict or wrong scope -> adjust wiki.toml
   - Uncertain -> ask the user
4. Run `wiki links check` to verify auto-linking candidates look right.
5. Once everything is clean, proceed to Step 6 (automated linting) and Step 7 (persist).

"#
        );
    } else {
        print!(
            r#"### Step 1: Scan the wiki structure

Run: `wiki scan`

This outputs per-directory statistics: file counts, frontmatter field coverage,
common section headings, and detected mirror candidates.

### Step 2: Learn the config schema

Run: `wiki setup example-config`

This outputs a complete wiki.toml with every option, annotated with comments.
Study it to understand what's available.

### Step 3: Generate and customize wiki.toml

Run: `wiki setup init`

This generates a starting-point wiki.toml. Edit it to customize:
- Set `autolink = false` on directories whose page names are too long or specific
  to be useful auto-link patterns (dates, identifiers, compound slugs)
- Add `[[rules]]` for required sections, required frontmatter, mirror parity
- Add citation patterns if the wiki tracks references to external sources
- Adjust `[checks]` severities if needed

### Step 4: Validate iteratively

Run: `wiki lint`

For each finding:
- Real content problem -> fix the wiki content
- Config too strict or wrong scope -> adjust wiki.toml
- Uncertain -> ask the user

Use `wiki lint --severity error` to focus on blocking issues first.
Use `wiki lint --severity warn` to review advisories separately.

Repeat until `wiki lint` exits clean.

### Step 5: Verify commands

Run and verify output makes sense:
- `wiki links check` — bare mentions should be genuine misses, not false positives
- `wiki links broken` — should be empty if the wiki is healthy
- `wiki refs to <pick a page from the wiki>` — verify the link graph looks right
- Review `wiki scan` output for inconsistent section headings across directories
  and use `wiki sections rename` to standardize them

"#
        );
    }

    print!(
        r#"### Step 6: Set up automated linting

Configure `wiki lint` to run automatically before commits. Options:
- Git pre-commit hook (`.githooks/pre-commit` or `.git/hooks/pre-commit`)
- Agent hook (e.g., Claude Code `pre-commit` hook in `.claude/settings.json`)
- Both, if the wiki is edited by agents and humans

Choose what fits this project's setup.

### Step 7: Update project documentation

Check if project documentation (CLAUDE.md, AGENTS.md, .cursorrules, or equivalent)
already references wiki tooling commands.

If it does:
- Update command references to match the current CLI (`wiki --help`)
- Remove references to commands that no longer exist
- Verify workflow instructions use the correct command names and flags

If it doesn't:
- Add a tooling section documenting the key commands and when to use them
- Integrate commands into existing workflow documentation where relevant
  (e.g., "run `wiki links fix --write` after ingest" in an ingest workflow)

Key commands the documentation should cover:
- `wiki lint` — structural integrity check (before commits)
- `wiki links check` / `wiki links fix --write` — bare mention detection (after page creation)
- `wiki rename <old> <new> --write` — page rename with reference update
- `wiki refs to <page>` — impact analysis before editing
- `wiki sections rename <old> <new> --write` — heading standardization
- `wiki setup prompt` — re-read these instructions
"#
    );

    Ok(())
}

/// Scan wiki structure and output per-directory statistics.
pub fn scan(root: &WikiRoot, ignore: &IgnoreConfig) -> Result<(), WikiError> {
    let wiki_root = root.path();

    // Find all directories containing .md files
    let mut dir_stats: HashMap<String, DirStats> = HashMap::new();

    for entry in wiki_walk_builder(wiki_root, wiki_root, ignore)?.build() {
        let entry = entry.map_err(|e| WikiError::Walk {
            path: wiki_root.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if !is_markdown_file(path) {
            continue;
        }

        let rel_path = path.strip_prefix(wiki_root).unwrap_or(path);
        let dir = rel_path
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or(".")
            .to_owned();

        let stats = dir_stats.entry(dir).or_default();
        stats.file_count += 1;

        let source = std::fs::read_to_string(path).map_err(|e| WikiError::ReadFile {
            path: path.to_path_buf(),
            source: e,
        })?;

        // Frontmatter analysis
        if let Ok(Some(fm)) = frontmatter::parse_frontmatter(&source)
            && let serde_yml::Value::Mapping(map) = fm.data()
        {
            for key in map.keys() {
                *stats.frontmatter_fields.entry(key.to_owned()).or_insert(0) += 1;
            }
        }

        // Section heading analysis (## level)
        let headings = parse::extract_headings(&source);
        for h in &headings {
            if h.level == 2 {
                *stats.section_headings.entry(h.text.clone()).or_insert(0) += 1;
            }
        }
    }

    // Sort directories for consistent output
    let mut dirs: Vec<_> = dir_stats.into_iter().collect();
    dirs.sort_by(|a, b| a.0.cmp(&b.0));

    for (dir, stats) in &dirs {
        let display_dir = if dir.is_empty() { "." } else { dir.as_str() };
        println!(
            "## Directory: {display_dir}/ ({} files)\n",
            stats.file_count
        );

        if !stats.frontmatter_fields.is_empty() {
            println!("Frontmatter fields:");
            let mut fields: Vec<_> = stats.frontmatter_fields.iter().collect();
            fields.sort_by(|a, b| b.1.cmp(a.1));
            for (field, count) in &fields {
                let pct = **count as f64 / stats.file_count as f64 * 100.0;
                println!("  {field:20} {count}/{} ({pct:.0}%)", stats.file_count);
            }
        } else {
            println!("  No frontmatter detected.");
        }

        if !stats.section_headings.is_empty() {
            println!("\nSection headings (## level):");
            let mut headings: Vec<_> = stats.section_headings.iter().collect();
            headings.sort_by(|a, b| b.1.cmp(a.1));
            for (heading, count) in headings.iter().take(10) {
                let pct = **count as f64 / stats.file_count as f64 * 100.0;
                println!(
                    "  \"{heading:18}\" {count}/{} ({pct:.0}%)",
                    stats.file_count
                );
            }
            if headings.len() > 10 {
                println!("  ... and {} more", headings.len() - 10);
            }
        }

        println!();
    }

    // Detect mirror candidates: directories with matching file stems
    let dir_counts: Vec<(String, usize)> = dirs
        .iter()
        .map(|(dir, stats)| (dir.clone(), stats.file_count))
        .collect();
    let mirror_candidates = detect_mirror_candidates(&dir_counts);
    if !mirror_candidates.is_empty() {
        println!("## Mirror candidates\n");
        for (a, b, count) in &mirror_candidates {
            println!("  {a}/ ({count} files) <-> {b}/ ({count} files)");
        }
        println!();
    }

    // Check for index file
    for candidate in &["index.md", "README.md", "_index.md"] {
        let path = wiki_root.join(candidate);
        if path.is_file() {
            let source = std::fs::read_to_string(&path).map_err(|e| WikiError::ReadFile {
                path: path.clone(),
                source: e,
            })?;
            let wikilinks = parse::extract_wikilinks(&source);
            let unique_refs: std::collections::HashSet<&str> =
                wikilinks.iter().map(|wl| wl.page.as_str()).collect();
            println!(
                "## Index: {candidate}\n  References {} unique page names via wikilinks\n",
                unique_refs.len()
            );
            break;
        }
    }

    Ok(())
}

/// Output a complete annotated wiki.toml with all options.
pub fn example_config() {
    let sections = [
        (
            "# wiki.toml — Complete configuration reference\n\
             #\n\
             # Every available option with explanatory comments.\n\
             # In practice, only include settings that differ from defaults.\n",
            build_index_section(),
        ),
        (
            "# Declare which directories contain wiki pages. Each entry is recursive\n\
             # (includes all subdirectories).\n\
             #\n\
             # When multiple entries overlap (parent + child), the most-specific path wins\n\
             # for per-page settings. This is the intended override mechanism:\n\
             #   path = \"wiki\"          (parent, sets defaults for all of wiki/)\n\
             #   path = \"wiki/papers\"   (child, overrides settings for wiki/papers/)\n\
             #\n\
             # If no [[directories]] are declared:\n\
             #   Defaults to \"wiki/\" with autolink = true.\n\
             #\n\
             # If ANY [[directories]] are declared, the default is replaced entirely.\n",
            build_directories_section(),
        ),
        ("", build_ignore_section()),
        ("", build_linking_section()),
        (
            "# Wiki-wide structural checks. These apply to all pages regardless of directory.\n\
             # Values: \"error\" (causes exit code 2), \"warn\" (prints but exits 0), \"off\"\n",
            build_checks_section(),
        ),
        (
            "# Parameterized rules scoped to specific directories. Each rule has a `check`\n\
             # type and a `severity` (\"error\", \"warn\", or \"off\").\n\
             #\n\
             # The `dirs` field uses path-prefix matching:\n\
             #   dirs = [\"wiki\"] matches any page under wiki/ (including subdirectories)\n\
             #   dirs = [\"wiki/concepts\"] matches only pages under wiki/concepts/\n",
            build_rules_section(),
        ),
    ];

    for (comment, toml) in &sections {
        if !comment.is_empty() {
            println!("{comment}");
        }
        print!("{toml}");
    }
}

fn toml_array(items: &[&str]) -> toml::Value {
    toml::Value::Array(
        items
            .iter()
            .map(|s| toml::Value::String(s.to_string()))
            .collect(),
    )
}

fn build_index_section() -> String {
    let mut tbl = toml::Table::new();
    tbl.insert(
        "index".to_owned(),
        toml::Value::String("index.md".to_owned()),
    );

    let mut out = String::new();
    out.push_str("# Index file path, relative to wiki root.\n");
    out.push_str(
        "# Scanned for wikilinks (index-coverage check) but NOT treated as a wiki page.\n",
    );
    out.push_str("# Default: \"index.md\". Set to \"\" to disable index coverage.\n");
    out.push_str(&toml::to_string_pretty(&tbl).unwrap());
    out
}

fn build_directories_section() -> String {
    let dirs = vec![
        (
            "wiki",
            true,
            "# autolink: pages here feed bare-mention auto-linking.\n# When true, filename stems become patterns for `wiki links check`.\n# Default: true\n",
        ),
        (
            "wiki/papers",
            false,
            "# Long, specific names are poor auto-link patterns — disable.\n",
        ),
        ("wiki/topics", false, ""),
    ];

    let mut out = String::new();
    for (path, autolink, comment) in dirs {
        if !comment.is_empty() {
            out.push_str(comment);
        }
        out.push_str("[[directories]]\n");
        out.push_str(&format!("path = \"{path}\"\n"));
        out.push_str(&format!("autolink = {autolink}\n\n"));
    }
    out
}

fn build_ignore_section() -> String {
    let mut out = String::new();
    out.push_str("[ignore]\n");
    out.push_str("# Built-in non-wiki tool directory patterns are enabled by default:\n");
    out.push_str("# .agents/, .claude/, .cursor/, .windsurf/, .pi/, etc.\n");
    out.push_str("# Cargo-style: set default_patterns = false to discard them all.\n");
    out.push_str("# Default: true\n");
    out.push_str("default_patterns = true\n\n");
    out.push_str("# Extra gitignore-style patterns, relative to the wiki root.\n");
    out.push_str("# Additive with defaults; cannot subtract individual default patterns.\n");
    out.push_str("# Default: []\n");
    out.push_str("patterns = [\"generated/\", \"scratch/**/*.md\"]\n\n");
    out
}

fn build_linking_section() -> String {
    let mut out = String::new();
    out.push_str("[linking]\n");

    out.push_str("# Page names to never auto-link, even in autolink=true directories.\n");
    out.push_str("# Default: []\n");
    let exclude = toml::Value::Array(vec![
        toml::Value::String("the".to_owned()),
        toml::Value::String("a".to_owned()),
        toml::Value::String("an".to_owned()),
    ]);
    out.push_str(&format!("exclude = {exclude}\n\n"));

    out.push_str("# Frontmatter field that pages can set to false to opt out of auto-linking.\n");
    out.push_str("# Default: \"autolink\"\n");
    out.push_str("autolink_field = \"autolink\"\n\n");

    out
}

fn build_checks_section() -> String {
    let mut out = String::new();
    out.push_str("[checks]\n");

    out.push_str("# Every [[wikilink]] must resolve to an existing page.\n");
    out.push_str("# Fragment references ([[page#heading]], [[page#^block]]) are also validated.\n");
    out.push_str("# Default: \"error\"\n");
    out.push_str("broken_links = \"error\"\n\n");

    out.push_str(
        "# Every wiki page must have at least one inbound [[wikilink]] from another page.\n",
    );
    out.push_str("# Default: \"error\"\n");
    out.push_str("orphan_pages = \"error\"\n\n");

    out.push_str("# Every wiki page must be referenced via [[wikilink]] in the index file.\n");
    out.push_str("# Only active if `index` is set and the file exists.\n");
    out.push_str("# Default: \"error\"\n");
    out.push_str("index_coverage = \"error\"\n\n");

    out
}

fn build_rules_section() -> String {
    let mut out = String::new();

    // Required sections
    out.push_str("# --- Required sections ---\n");
    out.push_str("# Pages in the specified directories must contain these ## headings.\n\n");

    out.push_str("[[rules]]\ncheck = \"required-sections\"\n");
    out.push_str(&format!("dirs = {}\n", toml_array(&["wiki/concepts"])));
    out.push_str(&format!(
        "sections = {}\n",
        toml_array(&["See also", "Viability check"])
    ));
    out.push_str("severity = \"error\"\n\n");

    out.push_str("[[rules]]\ncheck = \"required-sections\"\n");
    out.push_str(&format!("dirs = {}\n", toml_array(&["wiki/topics"])));
    out.push_str(&format!("sections = {}\n", toml_array(&["See also"])));
    out.push_str("severity = \"warn\"\n\n");

    // Required frontmatter
    out.push_str("# --- Required frontmatter fields ---\n");
    out.push_str(
        "# Pages in the specified directories must have these YAML frontmatter fields.\n\n",
    );

    out.push_str("[[rules]]\ncheck = \"required-frontmatter\"\n");
    out.push_str(&format!(
        "dirs = {}\n",
        toml_array(&["wiki/concepts", "wiki/topics"])
    ));
    out.push_str(&format!(
        "fields = {}\n",
        toml_array(&["title", "tags", "date"])
    ));
    out.push_str("severity = \"error\"\n\n");

    out.push_str("[[rules]]\ncheck = \"required-frontmatter\"\n");
    out.push_str(&format!("dirs = {}\n", toml_array(&["wiki/papers"])));
    out.push_str(&format!(
        "fields = {}\n",
        toml_array(&["title", "tags", "date", "sources"])
    ));
    out.push_str("severity = \"error\"\n\n");

    // Mirror parity
    out.push_str("# --- Mirror parity ---\n");
    out.push_str(
        "# Two directories must contain matching filenames (by stem, ignoring extension).\n",
    );
    out.push_str("# Useful for raw-source / processed-page pairs.\n");
    out.push_str("# Note: `right` does NOT need to be a declared [[directories]] entry.\n\n");

    out.push_str("[[rules]]\ncheck = \"mirror-parity\"\n");
    out.push_str("left = \"wiki/papers\"\nright = \"raw/papers\"\n");
    out.push_str("severity = \"error\"\n\n");

    // Citation patterns
    out.push_str("# --- Citation patterns ---\n");
    out.push_str("# Detect references in prose that should have corresponding wiki pages.\n");
    out.push_str("#\n");
    out.push_str("# Each pattern has a regex with a named capture group `id`.\n");
    out.push_str("# `match_in`: which directory to search for matching pages.\n");
    out.push_str("# `match_mode`:\n");
    out.push_str(
        "#   \"content\"  - search page file contents for the captured ID string (default)\n",
    );
    out.push_str("#   \"filename\" - check if a page with the captured ID as filename exists\n");
    out.push_str("#\n");
    out.push_str("# Use `preset` instead of `pattern` for built-in patterns:\n");
    out.push_str(
        "#   \"bold-method-year\" - matches **MethodName** (Author, YEAR), checks filenames\n\n",
    );

    out.push_str("[[rules]]\ncheck = \"citation-pattern\"\nname = \"arxiv\"\n");
    out.push_str(&format!(
        "dirs = {}\n",
        toml_array(&["wiki/concepts", "wiki/topics"])
    ));
    out.push_str("pattern = 'arxiv\\.org/abs/(?P<id>\\d{4}\\.\\d{4,5})'\n");
    out.push_str("match_in = \"wiki/papers\"\nmatch_mode = \"content\"\nseverity = \"warn\"\n\n");

    out.push_str("# Preset-based: no regex needed, preset bundles pattern + match_mode.\n");
    out.push_str("[[rules]]\ncheck = \"citation-pattern\"\nname = \"bold-method\"\n");
    out.push_str("preset = \"bold-method-year\"\n");
    out.push_str(&format!(
        "dirs = {}\n",
        toml_array(&["wiki/concepts", "wiki/topics"])
    ));
    out.push_str("match_in = \"wiki/papers\"\nseverity = \"warn\"\n\n");

    out.push_str("[[rules]]\ncheck = \"citation-pattern\"\nname = \"doi\"\n");
    out.push_str(&format!("dirs = {}\n", toml_array(&["wiki"])));
    out.push_str("pattern = 'doi\\.org/(?P<id>10\\.\\d{4,}/[^\\s)]+)'\n");
    out.push_str("match_in = \"wiki/papers\"\nmatch_mode = \"content\"\nseverity = \"warn\"\n");

    out
}
