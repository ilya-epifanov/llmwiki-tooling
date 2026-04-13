use std::ops::Range;
use std::path::PathBuf;

use crate::error::{RenameError, WikiError};
use crate::page::PageId;
use crate::splice;
use crate::wiki::Wiki;

type Edits = Vec<(Range<usize>, String)>;

struct RenameOp {
    moves: Vec<(PathBuf, PathBuf)>,
    /// (path, original source, edits) — source captured during planning to avoid re-reading.
    edits: Vec<(PathBuf, String, Edits)>,
}

/// Run `rename <old> <new>`: rename a page with full reference update.
pub fn rename(
    wiki: &mut Wiki,
    old_name: &str,
    new_name: &str,
    write: bool,
) -> Result<(), RenameError> {
    let old_id = PageId::from(old_name);
    let new_id = PageId::from(new_name);

    if !wiki.contains(&old_id) {
        return Err(RenameError::SourceNotFound(old_id));
    }
    if wiki.contains(&new_id) {
        let entry = wiki.get(&new_id).unwrap();
        return Err(RenameError::TargetExists {
            path: wiki.root().path().join(&entry.rel_path),
        });
    }

    let op = plan_rename(wiki, old_name, new_name)?;

    if write {
        execute_rename(wiki, &op)?;
    } else {
        display_rename(wiki, &op)?;
    }

    Ok(())
}

fn case_insensitive_replace(haystack: &str, needle: &str, replacement: &str) -> String {
    let lower_haystack = haystack.to_lowercase();
    let lower_needle = needle.to_lowercase();
    if let Some(pos) = lower_haystack.find(&lower_needle) {
        let mut result = String::with_capacity(haystack.len() - needle.len() + replacement.len());
        result.push_str(&haystack[..pos]);
        result.push_str(replacement);
        result.push_str(&haystack[pos + needle.len()..]);
        result
    } else {
        haystack.to_owned()
    }
}

fn plan_rename(wiki: &Wiki, old_name: &str, new_name: &str) -> Result<RenameOp, WikiError> {
    let mut moves = Vec::new();
    let config = wiki.config();
    let root = wiki.root();

    // Search all configured directories
    for dir_config in &config.directories {
        let dir = root.path().join(&dir_config.path);
        moves.extend(find_files_to_rename(&dir, old_name, new_name));
    }

    // Search mirror-parity right paths
    for (_, right) in config.mirror_paths() {
        let dir = root.path().join(right);
        moves.extend(find_files_to_rename(&dir, old_name, new_name));
    }

    // Find all wikilink references to update
    let mut edits = Vec::new();
    let old_id = PageId::from(old_name);

    // Get the actual display name (preserves original file casing) for replacement
    let old_display = wiki.display_name(&old_id).unwrap_or(old_name);

    for file_path in wiki.all_scannable_files() {
        let source = wiki.source(&file_path)?;

        let mut file_edits = Vec::new();

        // Update wikilinks
        let wikilinks = wiki.wikilinks(&file_path)?;
        for wl in wikilinks {
            if wl.page != old_id {
                continue;
            }
            let old_text = &source[wl.byte_range.clone()];
            // Use case-insensitive replacement to handle varying casing in wikilinks
            let new_text = case_insensitive_replace(old_text, old_display, new_name);
            if new_text != old_text {
                file_edits.push((wl.byte_range.clone(), new_text));
            }
        }

        // Update mirror path references in file content
        for (_, right) in config.mirror_paths() {
            let old_ref = format!("{right}/{old_name}.md");
            let new_ref = format!("{right}/{new_name}.md");
            if let Some(pos) = source.find(&old_ref) {
                file_edits.push((pos..pos + old_ref.len(), new_ref));
            }
        }

        if !file_edits.is_empty() {
            edits.push((file_path, source.to_owned(), file_edits));
        }
    }

    Ok(RenameOp { moves, edits })
}

fn find_files_to_rename(
    dir: &std::path::Path,
    old_name: &str,
    new_name: &str,
) -> Vec<(PathBuf, PathBuf)> {
    let mut moves = Vec::new();
    if !dir.is_dir() {
        return moves;
    }
    let target_filename = format!("{old_name}.md");
    for entry in ignore::WalkBuilder::new(dir)
        .hidden(false)
        .build()
        .flatten()
    {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .is_some_and(|n| n == target_filename.as_str())
        {
            let new_path = path.with_file_name(format!("{new_name}.md"));
            moves.push((path.to_path_buf(), new_path));
        }
    }
    moves
}

fn display_rename(wiki: &Wiki, op: &RenameOp) -> Result<(), WikiError> {
    println!("Planned rename (dry-run):\n");

    if !op.moves.is_empty() {
        println!("File moves:");
        for (old, new) in &op.moves {
            println!(
                "  {} -> {}",
                wiki.rel_path(old).display(),
                wiki.rel_path(new).display()
            );
        }
        println!();
    }

    if !op.edits.is_empty() {
        println!("Reference updates:");
        for (path, source, edits) in &op.edits {
            print!("{}", splice::diff(source, wiki.rel_path(path), edits));
        }
    }

    println!(
        "\n{} file(s) to move, {} file(s) to update. Use --write to apply.",
        op.moves.len(),
        op.edits.len(),
    );

    Ok(())
}

fn execute_rename(wiki: &mut Wiki, op: &RenameOp) -> Result<(), WikiError> {
    for (old, new) in &op.moves {
        wiki.rename_file(old, new)?;
        println!(
            "moved {} -> {}",
            wiki.rel_path(old).display(),
            wiki.rel_path(new).display()
        );
    }

    for (path, source, edits) in &op.edits {
        let actual_path = if path.is_file() {
            path.clone()
        } else {
            let mut found = path.clone();
            for (old, new) in &op.moves {
                if path == old {
                    found = new.clone();
                    break;
                }
            }
            found
        };

        let result = splice::apply(source, edits);
        wiki.write_file(&actual_path, &result)?;
        println!("updated {}", wiki.rel_path(&actual_path).display());
    }

    Ok(())
}
