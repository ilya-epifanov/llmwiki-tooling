use std::path::Path;

use crate::edit_plan::{DryRunOutput, EditPlan, EditPlanMode};
use crate::error::WikiError;
use crate::link_format;
use crate::link_index::LinkIndex;
use crate::mention::{BareMention, ConceptMatcher};
use crate::page::PageId;
use crate::splice;
use crate::wiki::Wiki;

fn bare_mentions_for_file(
    wiki: &Wiki,
    matcher: &ConceptMatcher,
    file_path: &Path,
) -> Result<Vec<BareMention>, WikiError> {
    let document = wiki.file(file_path)?;
    let self_page = PageId::from_path(file_path).unwrap_or_else(|| PageId::from(""));
    Ok(matcher.find_bare_mentions(document.source(), document.classified_ranges(), &self_page))
}

/// Run `links check`: find bare mentions that should be internal links.
pub fn check(wiki: &Wiki) -> Result<usize, WikiError> {
    let matcher = ConceptMatcher::new(wiki.autolink_pages()?);
    let mut total_mentions = 0;

    for file_path in wiki.scannable_files() {
        let mentions = bare_mentions_for_file(wiki, &matcher, file_path)?;
        let rel_path = wiki.rel_path(file_path);

        for m in &mentions {
            let display = wiki.display_name(&m.concept).unwrap_or(m.concept.as_str());
            println!(
                "{}:{}:{}: bare mention \"{}\" (should be linked)",
                rel_path.display(),
                m.line,
                m.col,
                display,
            );
        }

        total_mentions += mentions.len();
    }

    Ok(total_mentions)
}

/// Run `links fix`: auto-link bare mentions.
pub fn fix(wiki: &mut Wiki, write: bool) -> Result<usize, WikiError> {
    let matcher = ConceptMatcher::new(wiki.autolink_pages()?);
    let mut total_fixes = 0;

    let mut plan = EditPlan::new();
    plan.add_scannable_edits(wiki, |file_path, _source| {
        let mentions = bare_mentions_for_file(wiki, &matcher, file_path)?;
        total_fixes += mentions.len();
        link_format::format_mentions(wiki, file_path, &mentions)
    })?;

    plan.execute(
        wiki,
        EditPlanMode::from_write_flag(write, DryRunOutput::Diff),
    )?;

    Ok(total_fixes)
}

/// Run `links format`: convert resolvable internal links to the configured style.
pub fn format(wiki: &mut Wiki, write: bool) -> Result<usize, WikiError> {
    let mut total_edits = 0;
    let mut plan = EditPlan::new();
    plan.add_scannable_edits(wiki, |file_path, _source| {
        let outcome = link_format::format_document(wiki, file_path)?;
        if outcome.skipped > 0 {
            eprintln!(
                "warn: left {} unresolved link(s) unchanged in {}",
                outcome.skipped,
                wiki.rel_path(file_path).display()
            );
        }
        total_edits += outcome.edits.len();
        Ok(outcome.edits)
    })?;
    plan.execute(
        wiki,
        EditPlanMode::from_write_flag(write, DryRunOutput::Diff),
    )?;
    Ok(total_edits)
}

/// Run `links broken`: find broken internal links.
pub fn broken(wiki: &Wiki) -> Result<usize, WikiError> {
    let index = LinkIndex::build(wiki)?;

    for broken in index.broken_links() {
        let source = wiki.file(&broken.source_path)?.source();
        let rel_path = wiki.rel_path(&broken.source_path);
        let (line, col) = splice::offset_to_line_col(source, broken.link.byte_range.start);
        let ref_text = &source[broken.link.byte_range.clone()];
        println!(
            "{}:{}:{}: broken link {}: {}",
            rel_path.display(),
            line,
            col,
            ref_text.trim(),
            broken.reason,
        );
    }

    Ok(index.broken_links().len())
}

/// Run `links orphans`: find pages with no inbound internal links.
pub fn orphans(wiki: &Wiki) -> Result<usize, WikiError> {
    let index = LinkIndex::build(wiki)?;
    let orphan_pages = index.orphans(wiki);

    for page_id in &orphan_pages {
        if let Some(rel_path) = wiki.get(page_id) {
            println!("{}: orphan page (no inbound links)", rel_path.display());
        }
    }

    Ok(orphan_pages.len())
}
