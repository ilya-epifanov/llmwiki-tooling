use crate::error::WikiError;
use crate::link_index::LinkIndex;
use crate::mention::ConceptMatcher;
use crate::page::PageId;
use crate::resolve;
use crate::splice;
use crate::wiki::Wiki;

/// Run `links check`: find bare mentions that should be wikilinks.
pub fn check(wiki: &Wiki) -> Result<usize, WikiError> {
    let matcher = ConceptMatcher::new(wiki.autolink_pages()?);
    let mut total_mentions = 0;

    for file_path in wiki.all_scannable_files() {
        let source = wiki.source(&file_path)?;
        let classified = wiki.classified_ranges(&file_path)?;

        let self_page = PageId::from_path(&file_path).unwrap_or_else(|| PageId::from(""));
        let mentions = matcher.find_bare_mentions(source, classified, &self_page);

        let rel_path = wiki.rel_path(&file_path);

        for m in &mentions {
            let display = wiki.display_name(&m.concept).unwrap_or(m.concept.as_str());
            println!(
                "{}:{}:{}: bare mention \"{}\" (should be [[{}]])",
                rel_path.display(),
                m.line,
                m.col,
                display,
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

    // Collect all changes first (read phase)
    let mut changes: super::FileEdits = Vec::new();

    for file_path in wiki.all_scannable_files() {
        let source = wiki.source(&file_path)?;
        let classified = wiki.classified_ranges(&file_path)?;

        let self_page = PageId::from_path(&file_path).unwrap_or_else(|| PageId::from(""));
        let mentions = matcher.find_bare_mentions(source, classified, &self_page);

        if mentions.is_empty() {
            continue;
        }

        let edits: Vec<_> = mentions
            .iter()
            .map(|m| {
                let display = wiki.display_name(&m.concept).unwrap_or(m.concept.as_str());
                (m.byte_range.clone(), format!("[[{}]]", display))
            })
            .collect();

        changes.push((file_path, source.to_owned(), edits));
        total_fixes += mentions.len();
    }

    // Apply changes (write phase)
    for (file_path, source, edits) in changes {
        let rel_path = wiki.rel_path(&file_path);

        if write {
            let result = splice::apply(&source, &edits);
            wiki.write_file(&file_path, &result)?;
            println!(
                "{}: fixed {} bare mention(s)",
                rel_path.display(),
                edits.len()
            );
        } else {
            print!("{}", splice::diff(&source, rel_path, &edits));
        }
    }

    Ok(total_fixes)
}

/// Run `links broken`: find broken wikilinks.
pub fn broken(wiki: &Wiki) -> Result<usize, WikiError> {
    let mut total_broken = 0;

    for file_path in wiki.all_scannable_files() {
        let broken_links = resolve::find_broken_links(wiki, &file_path)?;
        let source = wiki.source(&file_path)?;

        let rel_path = wiki.rel_path(&file_path);

        for (wl, reason) in &broken_links {
            let (line, col) = splice::offset_to_line_col(source, wl.byte_range.start);
            let ref_text = &source[wl.byte_range.clone()];
            println!(
                "{}:{}:{}: broken link {}: {}",
                rel_path.display(),
                line,
                col,
                ref_text.trim(),
                reason,
            );
        }

        total_broken += broken_links.len();
    }

    Ok(total_broken)
}

/// Run `links orphans`: find pages with no inbound wikilinks.
pub fn orphans(wiki: &Wiki) -> Result<usize, WikiError> {
    let index = LinkIndex::build(wiki)?;
    let orphan_pages = index.orphans(wiki);

    for page_id in &orphan_pages {
        if let Some(entry) = wiki.get(page_id) {
            println!(
                "{}: orphan page (no inbound links)",
                entry.rel_path.display()
            );
        }
    }

    Ok(orphan_pages.len())
}
