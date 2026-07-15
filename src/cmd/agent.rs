use crate::config::IgnoreConfig;
use crate::error::WikiError;
use crate::inventory::WikiInventory;
use crate::wiki::WikiRoot;

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
- Set `[linking].link_style` to `obsidian` or `markdown`
- For Markdown links, set `reference_style_threshold` only when repeated targets
  should use Reference-style links
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
- `wiki links format` — review the configured style conversion before using `--write`
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
- `wiki links format --write` — explicit conversion to the configured link style
- `wiki rename <old> <new> --write` — page rename with reference update
- `wiki move <page> <dir> --write` — page relocation with relative markdown link updates
- `wiki refs to <page>` — impact analysis before editing
- `wiki sections rename <old> <new> --write` — heading standardization
- `wiki setup prompt` — re-read these instructions

When authoring links, prefer heading links. Use block links only when a heading
cannot identify the intended content.
"#
    );

    Ok(())
}

/// Scan wiki structure and output per-directory statistics.
pub fn scan(root: &WikiRoot, ignore: &IgnoreConfig) -> Result<(), WikiError> {
    let inventory = WikiInventory::build(root.path(), ignore)?;

    for dir in inventory.directories() {
        println!("## Directory: {}/ ({} files)\n", dir.path, dir.file_count);

        if !dir.frontmatter_fields.is_empty() {
            println!("Frontmatter fields:");
            let mut fields: Vec<_> = dir.frontmatter_fields.iter().collect();
            fields.sort_by(|a, b| b.1.cmp(a.1));
            for (field, count) in &fields {
                let pct = **count as f64 / dir.file_count as f64 * 100.0;
                println!("  {field:20} {count}/{} ({pct:.0}%)", dir.file_count);
            }
        } else {
            println!("  No frontmatter detected.");
        }

        if !dir.section_headings.is_empty() {
            println!("\nSection headings (## level):");
            let mut headings: Vec<_> = dir.section_headings.iter().collect();
            headings.sort_by(|a, b| b.1.cmp(a.1));
            for (heading, count) in headings.iter().take(10) {
                let pct = **count as f64 / dir.file_count as f64 * 100.0;
                println!("  \"{heading:18}\" {count}/{} ({pct:.0}%)", dir.file_count);
            }
            if headings.len() > 10 {
                println!("  ... and {} more", headings.len() - 10);
            }
        }

        println!();
    }

    if !inventory.mirror_candidates().is_empty() {
        println!("## Mirror candidates\n");
        for candidate in inventory.mirror_candidates() {
            println!(
                "  {}/ ({} files) <-> {}/ ({} files)",
                candidate.left, candidate.file_count, candidate.right, candidate.file_count
            );
        }
        println!();
    }

    if let Some(index) = inventory.index() {
        println!(
            "## Index: {}\n  References {} unique internal-link targets\n",
            index.path, index.unique_refs
        );
    }

    Ok(())
}

/// Output a complete annotated wiki.toml with all options.
pub fn example_config() {
    print!("{}", crate::config::EXAMPLE_CONFIG);
}
