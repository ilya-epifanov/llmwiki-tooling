use std::path::{Path, PathBuf};

use crate::edit_plan::{DryRunOutput, EditPlan, EditPlanMode};
use crate::error::WikiError;
use crate::markdown_links;
use crate::page::PageId;
use crate::wiki::Wiki;

pub fn move_page(
    wiki: &mut Wiki,
    page_name: &str,
    dest_dir: &Path,
    write: bool,
) -> Result<(), WikiError> {
    let page_id = PageId::from(page_name);
    let Some((_, rel_path)) = wiki.find(page_name) else {
        return Err(WikiError::PageNotFound(page_id));
    };

    let old_path = wiki.abs_path(rel_path);
    let dest_dir = markdown_links::normalize_path(wiki.abs_path(dest_dir));
    if !dest_dir.starts_with(wiki.root().path()) {
        return Err(WikiError::PathOutsideRoot { path: dest_dir });
    }
    let new_path = dest_dir.join(old_path.file_name().expect("markdown file has filename"));
    if new_path.exists() {
        return Err(WikiError::TargetPathExists { path: new_path });
    }

    let plan = plan_move(wiki, old_path, new_path)?;
    plan.execute(
        wiki,
        EditPlanMode::from_write_flag(
            write,
            DryRunOutput::Summary {
                title: "Planned move (dry-run):",
                moves_heading: "File move",
                edits_heading: "Markdown link updates",
            },
        ),
    )?;
    Ok(())
}

fn plan_move(wiki: &Wiki, old_path: PathBuf, new_path: PathBuf) -> Result<EditPlan, WikiError> {
    let mut plan = EditPlan::new();
    plan.move_file(old_path.clone(), new_path.clone());

    plan.add_scannable_edits(wiki, |file_path, _source| {
        let links = wiki.file(file_path)?.markdown_links();
        Ok(if file_path == old_path.as_path() {
            markdown_links::rebase_relative_links(links, &old_path, &new_path)
        } else {
            markdown_links::retarget_relative_links(links, file_path, &old_path, &new_path)
        })
    })?;
    Ok(plan)
}
