use std::collections::HashSet;

use crate::config::IgnoreConfig;
use crate::error::WikiError;
use crate::inventory::{DirectoryInventory, WikiInventory};
use crate::wiki::WikiRoot;

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

    let inventory = WikiInventory::build(root.path(), &IgnoreConfig::default())?;
    let content_dirs = content_dirs(root, &inventory);
    let content_dir_names: HashSet<&str> =
        content_dirs.iter().map(|dir| dir.path.as_str()).collect();

    let mut lines = Vec::new();

    // Only emit index if it's not the default "index.md".
    if let Some(index) = inventory.index()
        && index.path != "index.md"
    {
        lines.push(format!("index = \"{}\"", index.path));
        lines.push(String::new());
    }

    let mut rules_lines = Vec::new();
    for dir in &content_dirs {
        add_required_frontmatter_rule(&mut rules_lines, dir);
        add_required_sections_rule(&mut rules_lines, dir);
    }

    for candidate in inventory.mirror_candidates() {
        let left_is_content = content_dir_names.contains(candidate.left.as_str());
        let right_is_content = content_dir_names.contains(candidate.right.as_str());
        if left_is_content == right_is_content && left_is_content {
            continue;
        }
        let (left, right) = if left_is_content {
            (candidate.left.as_str(), candidate.right.as_str())
        } else {
            (candidate.right.as_str(), candidate.left.as_str())
        };
        rules_lines.push("[[rules]]".to_owned());
        rules_lines.push("check = \"mirror-parity\"".to_owned());
        rules_lines.push(format!("left = \"{left}\""));
        rules_lines.push(format!("right = \"{right}\""));
        rules_lines.push("severity = \"error\"".to_owned());
        rules_lines.push(String::new());
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

fn content_dirs<'a>(root: &WikiRoot, inventory: &'a WikiInventory) -> Vec<&'a DirectoryInventory> {
    let has_wiki_dir = root.path().join("wiki").is_dir();
    inventory
        .directories()
        .iter()
        .filter(|dir| {
            if has_wiki_dir {
                dir.path == "wiki" || dir.path.starts_with("wiki/")
            } else {
                true
            }
        })
        .collect()
}

fn add_required_frontmatter_rule(lines: &mut Vec<String>, dir: &DirectoryInventory) {
    let required: Vec<_> = dir
        .frontmatter_fields
        .iter()
        .filter(|(_, count)| **count == dir.file_count)
        .map(|(name, _)| name.as_str())
        .collect();
    if required.is_empty() {
        return;
    }

    lines.push("[[rules]]".to_owned());
    lines.push("check = \"required-frontmatter\"".to_owned());
    lines.push(format!("dirs = [\"{}\"]", dir.path));
    lines.push(format!("fields = [{}]", quoted_list(&required)));
    lines.push("severity = \"error\"".to_owned());
    lines.push(String::new());
}

fn add_required_sections_rule(lines: &mut Vec<String>, dir: &DirectoryInventory) {
    let required: Vec<_> = dir
        .section_headings
        .iter()
        .filter(|(_, count)| **count == dir.file_count)
        .map(|(name, _)| name.as_str())
        .collect();
    if required.is_empty() {
        return;
    }

    lines.push("[[rules]]".to_owned());
    lines.push("check = \"required-sections\"".to_owned());
    lines.push(format!("dirs = [\"{}\"]", dir.path));
    lines.push(format!("sections = [{}]", quoted_list(&required)));
    lines.push("severity = \"error\"".to_owned());
    lines.push(String::new());
}

fn quoted_list(items: &[&str]) -> String {
    items
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(", ")
}
