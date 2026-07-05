use std::path::PathBuf;

use crate::edit_plan::{DryRunOutput, EditPlan, EditPlanMode};
use crate::error::{RenameError, WikiError};
use crate::inventory::MarkdownFileSet;
use crate::markdown_links;
use crate::page::PageId;
use crate::wiki::Wiki;

/// Run `rename <old> <new>`: rename a page with full reference update.
pub fn rename(
    wiki: &mut Wiki,
    old_name: &str,
    new_name: &str,
    write: bool,
) -> Result<(), RenameError> {
    let requested_id = PageId::from(old_name);
    let Some((old_id, old_rel_path)) = wiki.find(old_name) else {
        return Err(RenameError::SourceNotFound(requested_id));
    };
    let old_id = old_id.clone();
    let old_stem = old_rel_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(old_name)
        .to_owned();
    let new_path = wiki
        .abs_path(old_rel_path)
        .with_file_name(format!("{new_name}.md"));
    if wiki.find(new_name).is_some() || new_path.exists() {
        let path = wiki
            .find(new_name)
            .map(|(_, rel_path)| wiki.abs_path(rel_path))
            .unwrap_or(new_path);
        return Err(RenameError::TargetExists { path });
    }

    let plan = plan_rename(wiki, &old_id, &old_stem, new_name)?;

    plan.execute(
        wiki,
        EditPlanMode::from_write_flag(
            write,
            DryRunOutput::Summary {
                title: "Planned rename (dry-run):",
                moves_heading: "File moves",
                edits_heading: "Reference updates",
            },
        ),
    )?;

    Ok(())
}

fn rename_wikilink_target(wikilink: &str, new_name: &str) -> String {
    let Some(open) = wikilink.find("[[") else {
        return wikilink.to_owned();
    };
    let Some(close) = wikilink.rfind("]]") else {
        return wikilink.to_owned();
    };

    let target_start = open + 2;
    let inner = &wikilink[target_start..close];
    let target_len = inner.find(['#', '|']).unwrap_or(inner.len());
    let mut renamed = String::with_capacity(wikilink.len() + new_name.len());
    renamed.push_str(&wikilink[..target_start]);
    renamed.push_str(new_name);
    renamed.push_str(&inner[target_len..]);
    renamed.push_str(&wikilink[close..]);
    collapse_redundant_alias(&renamed)
}

fn collapse_redundant_alias(wikilink: &str) -> String {
    let Some(open) = wikilink.find("[[") else {
        return wikilink.to_owned();
    };
    let Some(close) = wikilink.rfind("]]") else {
        return wikilink.to_owned();
    };
    let inner = &wikilink[open + 2..close];
    let Some((target, alias)) = inner.split_once('|') else {
        return wikilink.to_owned();
    };
    if target != alias {
        return wikilink.to_owned();
    }
    format!(
        "{}[[{}]]{}",
        &wikilink[..open],
        target,
        &wikilink[close + 2..]
    )
}

fn plan_rename(
    wiki: &Wiki,
    old_id: &PageId,
    old_stem: &str,
    new_name: &str,
) -> Result<EditPlan, WikiError> {
    let mut plan = EditPlan::new();
    let config = wiki.config();
    let root = wiki.root();
    let mirror_paths = config.mirror_paths();

    let renamed_file = wiki.find(old_stem).map(|(_, rel_path)| {
        let old_path = root.path().join(rel_path);
        let new_path = old_path.with_file_name(format!("{new_name}.md"));
        (old_path, new_path)
    });
    if let Some((old_path, new_path)) = &renamed_file {
        plan.move_file(old_path.clone(), new_path.clone());
    }

    for (_, right) in &mirror_paths {
        let dir = root.path().join(right);
        for (old_path, new_path) in
            find_files_to_rename(root.path(), &dir, &config.ignore, old_stem, new_name)?
        {
            plan.move_file(old_path, new_path);
        }
    }

    plan.add_scannable_edits(wiki, |file_path, source| {
        let document = wiki.file(file_path)?;
        let mut file_edits = Vec::new();

        for wl in document.wikilinks() {
            if wiki.canonical_id(&wl.page) != Some(old_id) {
                continue;
            }
            let old_text = &source[wl.byte_range.clone()];
            let new_text = rename_wikilink_target(old_text, new_name);
            if new_text != old_text {
                file_edits.push((wl.byte_range.clone(), new_text));
            }
        }

        for (_, right) in &mirror_paths {
            let old_ref = format!("{right}/{old_stem}.md");
            let new_ref = format!("{right}/{new_name}.md");
            if let Some(pos) = source.find(&old_ref) {
                file_edits.push((pos..pos + old_ref.len(), new_ref));
            }
        }

        if let Some((old_path, new_path)) = &renamed_file {
            file_edits.extend(markdown_links::retarget_relative_links(
                document.markdown_links(),
                file_path,
                old_path,
                new_path,
            ));
        }

        Ok(file_edits)
    })?;

    Ok(plan)
}

fn find_files_to_rename(
    wiki_root: &std::path::Path,
    dir: &std::path::Path,
    ignore: &crate::config::IgnoreConfig,
    old_name: &str,
    new_name: &str,
) -> Result<Vec<(PathBuf, PathBuf)>, WikiError> {
    let mut moves = Vec::new();
    if !dir.is_dir() {
        return Ok(moves);
    }
    let target_filename = format!("{old_name}.md");
    for file in MarkdownFileSet::build_under(wiki_root, dir, ignore)?.files() {
        if file
            .path
            .file_name()
            .is_some_and(|name| name == target_filename.as_str())
        {
            moves.push((
                file.path.clone(),
                file.path.with_file_name(format!("{new_name}.md")),
            ));
        }
    }
    Ok(moves)
}
