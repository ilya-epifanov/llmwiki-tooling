use crate::config::WikiConfig;
use crate::error::WikiError;
use crate::splice;
use crate::wiki::Wiki;

/// Run `sections rename`: rename a heading across the wiki, including fragment references.
pub fn rename(
    wiki: &mut Wiki,
    old_name: &str,
    new_name: &str,
    dirs: &Option<Vec<String>>,
    write: bool,
) -> Result<usize, WikiError> {
    // Collect all changes first (read phase)
    let mut changes: super::FileEdits = Vec::new();

    for file_path in wiki.all_scannable_files() {
        let rel_path = wiki.rel_path(&file_path);

        // If dirs filter is set, skip files outside those directories
        if let Some(dir_filter) = dirs
            && !WikiConfig::matches_dirs(rel_path, dir_filter)
        {
            continue;
        }

        let source = wiki.source(&file_path)?;
        let mut file_edits = Vec::new();

        // Find heading occurrences to rename
        let headings = wiki.headings(&file_path)?;
        for h in headings {
            if h.text.eq_ignore_ascii_case(old_name) {
                // Replace just the heading text within the heading range.
                // The heading range includes `## ` prefix. Find the text portion.
                let heading_src = &source[h.byte_range.clone()];
                if let Some(text_offset) = heading_src.find(&h.text) {
                    let abs_start = h.byte_range.start + text_offset;
                    let abs_end = abs_start + h.text.len();
                    file_edits.push((abs_start..abs_end, new_name.to_owned()));
                }
            }
        }

        // Find wikilink heading fragment references: [[page#Old Name]] -> [[page#New Name]]
        let wikilinks = wiki.wikilinks(&file_path)?;
        for wl in wikilinks {
            if let Some(crate::page::WikilinkFragment::Heading(ref heading)) = wl.fragment
                && heading.as_str().eq_ignore_ascii_case(old_name)
            {
                let wl_src = &source[wl.byte_range.clone()];
                let new_wl = wl_src.replace(heading.as_str(), new_name);
                if new_wl != wl_src {
                    file_edits.push((wl.byte_range.clone(), new_wl));
                }
            }
        }

        if !file_edits.is_empty() {
            changes.push((file_path, source.to_owned(), file_edits));
        }
    }

    // Apply changes (write phase)
    let mut total_changes = 0;
    for (file_path, source, file_edits) in changes {
        let rel_path = wiki.rel_path(&file_path);

        if write {
            let result = splice::apply(&source, &file_edits);
            wiki.write_file(&file_path, &result)?;
            println!(
                "{}: renamed {} occurrence(s)",
                rel_path.display(),
                file_edits.len()
            );
        } else {
            print!("{}", splice::diff(&source, rel_path, &file_edits));
        }

        total_changes += file_edits.len();
    }

    Ok(total_changes)
}
