use crate::config::WikiConfig;
use crate::edit_plan::{DryRunOutput, EditPlan, EditPlanMode};
use crate::error::WikiError;
use crate::wiki::Wiki;

/// Run `sections rename`: rename a heading across the wiki, including fragment references.
pub fn rename(
    wiki: &mut Wiki,
    old_name: &str,
    new_name: &str,
    dirs: &Option<Vec<String>>,
    write: bool,
) -> Result<usize, WikiError> {
    let mut plan = EditPlan::new();

    plan.add_scannable_edits(wiki, |file_path, source| {
        let rel_path = wiki.rel_path(file_path);

        // If dirs filter is set, skip files outside those directories
        if let Some(dir_filter) = dirs
            && !WikiConfig::matches_dirs(rel_path, dir_filter)
        {
            return Ok(Vec::new());
        }

        let mut file_edits = Vec::new();

        let document = wiki.file(file_path)?;

        // Find heading occurrences to rename
        for h in document.headings() {
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
        for wl in document.wikilinks() {
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

        Ok(file_edits)
    })?;

    let total_changes = plan.edit_count();
    plan.execute(
        wiki,
        EditPlanMode::from_write_flag(write, DryRunOutput::Diff),
    )?;

    Ok(total_changes)
}
