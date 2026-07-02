use std::collections::HashMap;
use std::path::Path;

use crate::config::IgnoreConfig;
use crate::error::WikiError;
use crate::frontmatter;
use crate::parse;
use crate::walk::{is_markdown_file, wiki_walk_builder};
use crate::wiki::WikiRoot;

use super::{DirStats, share_name_component};

/// Generate a minimal wiki.toml from detected wiki structure.
pub fn init(root: &WikiRoot, force: bool, show: bool) -> Result<(), WikiError> {
    let config_path = root.path().join("wiki.toml");

    if !show && config_path.is_file() && !force {
        eprintln!(
            "wiki.toml already exists at {}. Use --force to overwrite.",
            config_path.display()
        );
        return Ok(());
    }

    let mut lines = Vec::new();

    // Only emit index if it's not the default "index.md"
    let detected_index = ["index.md", "README.md"]
        .iter()
        .find(|c| root.path().join(c).is_file())
        .copied();
    if let Some(idx) = detected_index
        && idx != "index.md"
    {
        lines.push(format!("index = \"{idx}\""));
        lines.push(String::new());
    }

    // Detect directories — only emit if structure differs from auto-detection default
    let wiki_dir = root.path().join("wiki");
    let content_dirs: Vec<String> = if wiki_dir.is_dir() {
        let mut dirs = vec!["wiki".to_owned()];
        for subdir in list_subdirs(&wiki_dir) {
            dirs.push(format!("wiki/{subdir}"));
        }
        dirs
    } else {
        vec![".".to_owned()]
    };

    // Auto-detection default is "wiki" if wiki/ exists, "." otherwise.
    // Only emit [[directories]] entries that change a setting (e.g. autolink = false).
    // For init, we don't know which dirs should be autolink=false, so omit all.
    // The agent will add overrides after reviewing `wiki scan` output.

    // Scan directories for rules to generate
    let mut rules_lines = Vec::new();

    for dir in &content_dirs {
        let abs_dir = root.path().join(dir);
        if !abs_dir.is_dir() {
            continue;
        }
        let stats = scan_dir(&abs_dir)?;
        if stats.file_count == 0 {
            continue;
        }

        // Required frontmatter: fields present in 100% of files
        let mut required_fm: Vec<&str> = stats
            .frontmatter_fields
            .iter()
            .filter(|(_, count)| **count == stats.file_count)
            .map(|(name, _)| name.as_str())
            .collect();
        required_fm.sort();

        if !required_fm.is_empty() {
            let fields_str = required_fm
                .iter()
                .map(|f| format!("\"{f}\""))
                .collect::<Vec<_>>()
                .join(", ");
            rules_lines.push("[[rules]]".to_owned());
            rules_lines.push("check = \"required-frontmatter\"".to_owned());
            rules_lines.push(format!("dirs = [\"{dir}\"]"));
            rules_lines.push(format!("fields = [{fields_str}]"));
            rules_lines.push("severity = \"error\"".to_owned());
            rules_lines.push(String::new());
        }

        // Required sections: ## headings present in 100% of files
        let mut required_sections: Vec<&str> = stats
            .section_headings
            .iter()
            .filter(|(_, count)| **count == stats.file_count)
            .map(|(name, _)| name.as_str())
            .collect();
        required_sections.sort();

        if !required_sections.is_empty() {
            let sections_str = required_sections
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(", ");
            rules_lines.push("[[rules]]".to_owned());
            rules_lines.push("check = \"required-sections\"".to_owned());
            rules_lines.push(format!("dirs = [\"{dir}\"]"));
            rules_lines.push(format!("sections = [{sections_str}]"));
            rules_lines.push("severity = \"error\"".to_owned());
            rules_lines.push(String::new());
        }
    }

    // Mirror parity: find directory pairs with matching file counts and shared name component
    let dir_counts = scan_all_dir_counts(root)?;
    for i in 0..dir_counts.len() {
        for j in (i + 1)..dir_counts.len() {
            let (dir_a, count_a) = &dir_counts[i];
            let (dir_b, count_b) = &dir_counts[j];
            if count_a == count_b
                && *count_a > 0
                && share_name_component(dir_a, dir_b)
                // Only suggest mirrors between content and non-content dirs
                && (content_dirs.contains(dir_a) != content_dirs.contains(dir_b)
                    || !content_dirs.contains(dir_a))
            {
                let (left, right) = if content_dirs.contains(dir_a) {
                    (dir_a.as_str(), dir_b.as_str())
                } else {
                    (dir_b.as_str(), dir_a.as_str())
                };
                rules_lines.push("[[rules]]".to_owned());
                rules_lines.push("check = \"mirror-parity\"".to_owned());
                rules_lines.push(format!("left = \"{left}\""));
                rules_lines.push(format!("right = \"{right}\""));
                rules_lines.push("severity = \"error\"".to_owned());
                rules_lines.push(String::new());
            }
        }
    }

    lines.extend(rules_lines);

    let content = lines.join("\n") + "\n";

    if show {
        print!("{content}");
    } else {
        std::fs::write(&config_path, &content).map_err(|e| WikiError::WriteFile {
            path: config_path.clone(),
            source: e,
        })?;
        println!("created {}", config_path.display());
    }

    Ok(())
}

fn scan_dir(dir: &Path) -> Result<DirStats, WikiError> {
    let mut stats = DirStats::default();

    let entries = std::fs::read_dir(dir).map_err(|e| WikiError::ReadFile {
        path: dir.to_path_buf(),
        source: e,
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_markdown_file(&path) {
            continue;
        }
        stats.file_count += 1;

        let source = std::fs::read_to_string(&path).map_err(|e| WikiError::ReadFile {
            path: path.clone(),
            source: e,
        })?;

        if let Ok(Some(fm)) = frontmatter::parse_frontmatter(&source)
            && let serde_yml::Value::Mapping(map) = fm.data()
        {
            for key in map.keys() {
                *stats.frontmatter_fields.entry(key.to_owned()).or_insert(0) += 1;
            }
        }

        for h in parse::extract_headings(&source) {
            if h.level == 2 {
                *stats.section_headings.entry(h.text).or_insert(0) += 1;
            }
        }
    }

    Ok(stats)
}

fn list_subdirs(dir: &Path) -> Vec<String> {
    let mut subdirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && !name.starts_with('.')
            {
                subdirs.push(name.to_owned());
            }
        }
    }
    subdirs.sort();
    subdirs
}

/// Scan all directories under root for file counts (including non-wiki dirs like raw/).
fn scan_all_dir_counts(root: &WikiRoot) -> Result<Vec<(String, usize)>, WikiError> {
    let mut counts: HashMap<String, usize> = HashMap::new();

    let ignore = IgnoreConfig::default();
    for entry in wiki_walk_builder(root.path(), root.path(), &ignore)?.build() {
        let entry = entry.map_err(|e| WikiError::Walk {
            path: root.path().to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if is_markdown_file(path) {
            let rel = path.strip_prefix(root.path()).unwrap_or(path);
            if let Some(dir) = rel.parent().and_then(|p| p.to_str())
                && !dir.is_empty()
            {
                *counts.entry(dir.to_owned()).or_insert(0) += 1;
            }
        }
    }

    let mut result: Vec<_> = counts.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}
