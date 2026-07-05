use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::error::WikiError;
use crate::splice;
use crate::wiki::Wiki;

pub(crate) type Edits = Vec<(Range<usize>, String)>;

pub(crate) struct EditPlan {
    moves: Vec<(PathBuf, PathBuf)>,
    file_edits: Vec<PlannedFileEdit>,
}

pub(crate) enum EditPlanMode<'a> {
    Apply,
    DryRun(DryRunOutput<'a>),
}

impl<'a> EditPlanMode<'a> {
    pub(crate) fn from_write_flag(write: bool, dry_run: DryRunOutput<'a>) -> Self {
        if write {
            Self::Apply
        } else {
            Self::DryRun(dry_run)
        }
    }
}

pub(crate) enum DryRunOutput<'a> {
    Diff,
    Summary {
        title: &'a str,
        moves_heading: &'a str,
        edits_heading: &'a str,
    },
}

struct PlannedFileEdit {
    path: PathBuf,
    source: String,
    edits: Edits,
}

impl EditPlan {
    pub(crate) fn new() -> Self {
        Self {
            moves: Vec::new(),
            file_edits: Vec::new(),
        }
    }

    pub(crate) fn move_file(&mut self, old: PathBuf, new: PathBuf) {
        self.moves.push((old, new));
        self.moves.sort();
        self.moves.dedup();
    }

    pub(crate) fn add_edits(
        &mut self,
        path: PathBuf,
        source: &str,
        edits: Edits,
    ) -> Result<(), WikiError> {
        if edits.is_empty() {
            return Ok(());
        }
        validate_edits(&path, source, &edits)?;

        if let Some(existing) = self.file_edits.iter_mut().find(|file| file.path == path) {
            existing.edits.extend(edits);
            validate_edits(&existing.path, &existing.source, &existing.edits)?;
        } else {
            self.file_edits.push(PlannedFileEdit {
                path,
                source: source.to_owned(),
                edits,
            });
            self.file_edits.sort_by(|a, b| a.path.cmp(&b.path));
        }
        Ok(())
    }

    pub(crate) fn add_scannable_edits<F>(
        &mut self,
        wiki: &Wiki,
        mut edits_for_file: F,
    ) -> Result<(), WikiError>
    where
        F: FnMut(&Path, &str) -> Result<Edits, WikiError>,
    {
        for file_path in wiki.scannable_files() {
            let source = wiki.file(file_path)?.source();
            self.add_edits(
                file_path.clone(),
                source,
                edits_for_file(file_path, source)?,
            )?;
        }
        Ok(())
    }

    fn move_count(&self) -> usize {
        self.moves.len()
    }

    fn edited_file_count(&self) -> usize {
        self.file_edits.len()
    }

    pub(crate) fn edit_count(&self) -> usize {
        self.file_edits.iter().map(|file| file.edits.len()).sum()
    }

    fn path_after_moves<'a>(&'a self, path: &'a Path) -> &'a Path {
        path_after_moves(&self.moves, path)
    }

    fn print_diffs(&self, wiki: &Wiki) {
        for file in &self.file_edits {
            print!(
                "{}",
                splice::diff(
                    &file.source,
                    wiki.rel_path(self.path_after_moves(&file.path)),
                    &file.edits,
                )
            );
        }
    }

    fn print_dry_run(&self, wiki: &Wiki, output: DryRunOutput<'_>) {
        match output {
            DryRunOutput::Diff => self.print_diffs(wiki),
            DryRunOutput::Summary {
                title,
                moves_heading,
                edits_heading,
            } => {
                println!("{title}\n");

                if !self.moves.is_empty() {
                    println!("{moves_heading}:");
                    for (old, new) in &self.moves {
                        println!(
                            "  {} -> {}",
                            wiki.rel_path(old).display(),
                            wiki.rel_path(new).display()
                        );
                    }
                    println!();
                }

                if !self.file_edits.is_empty() {
                    println!("{edits_heading}:");
                    self.print_diffs(wiki);
                }

                println!(
                    "\n{} file(s) to move, {} file(s) to update. Use --write to apply.",
                    self.move_count(),
                    self.edited_file_count(),
                );
            }
        }
    }

    pub(crate) fn execute(self, wiki: &mut Wiki, mode: EditPlanMode<'_>) -> Result<(), WikiError> {
        match mode {
            EditPlanMode::Apply => self.apply(wiki),
            EditPlanMode::DryRun(output) => {
                self.print_dry_run(wiki, output);
                Ok(())
            }
        }
    }

    fn apply(self, wiki: &mut Wiki) -> Result<(), WikiError> {
        let Self { moves, file_edits } = self;

        for (old, new) in &moves {
            if let Some(parent) = new.parent() {
                std::fs::create_dir_all(parent).map_err(|e| WikiError::WriteFile {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            wiki.rename_file(old, new)?;
            println!(
                "moved {} -> {}",
                wiki.rel_path(old).display(),
                wiki.rel_path(new).display()
            );
        }

        for file in file_edits {
            let actual_path = path_after_moves(&moves, &file.path).to_path_buf();
            wiki.write_file(&actual_path, &splice::apply(&file.source, &file.edits))?;
            println!("updated {}", wiki.rel_path(&actual_path).display());
        }
        Ok(())
    }
}

fn path_after_moves<'a>(moves: &'a [(PathBuf, PathBuf)], path: &'a Path) -> &'a Path {
    moves
        .iter()
        .find_map(|(old, new)| (path == old).then_some(new.as_path()))
        .unwrap_or(path)
}

fn validate_edits(
    path: &Path,
    source: &str,
    edits: &[(Range<usize>, String)],
) -> Result<(), WikiError> {
    let mut ranges: Vec<_> = edits.iter().map(|(range, _)| range.clone()).collect();
    ranges.sort_by_key(|range| range.start);

    for range in &ranges {
        if range.start > range.end
            || range.end > source.len()
            || !source.is_char_boundary(range.start)
            || !source.is_char_boundary(range.end)
        {
            return Err(WikiError::InvalidEditRange {
                path: path.to_path_buf(),
                range: range.clone(),
                source_len: source.len(),
            });
        }
    }

    for pair in ranges.windows(2) {
        if pair[0].end > pair[1].start {
            return Err(WikiError::OverlappingEdits {
                path: path.to_path_buf(),
                first: pair[0].clone(),
                second: pair[1].clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_overlapping_edits() {
        let mut plan = EditPlan::new();
        let err = plan
            .add_edits(
                PathBuf::from("page.md"),
                "abcdef",
                vec![(1..4, "x".to_owned()), (3..5, "y".to_owned())],
            )
            .unwrap_err();

        assert!(matches!(err, WikiError::OverlappingEdits { .. }));
    }

    #[test]
    fn rejects_out_of_bounds_edits() {
        let mut plan = EditPlan::new();
        let err = plan
            .add_edits(
                PathBuf::from("page.md"),
                "abc",
                vec![(1..4, "x".to_owned())],
            )
            .unwrap_err();

        assert!(matches!(err, WikiError::InvalidEditRange { .. }));
    }
}
