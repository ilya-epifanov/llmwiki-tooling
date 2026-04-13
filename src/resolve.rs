use crate::error::WikiError;
use crate::page::{WikilinkFragment, WikilinkOccurrence};
use crate::wiki::Wiki;

/// Why a wikilink could not be resolved.
#[derive(Debug)]
pub enum BrokenReason {
    PageNotFound,
    HeadingNotFound { heading: String },
    BlockNotFound { block_id: String },
}

impl std::fmt::Display for BrokenReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PageNotFound => write!(f, "page not found"),
            Self::HeadingNotFound { heading } => write!(f, "heading not found: '{heading}'"),
            Self::BlockNotFound { block_id } => write!(f, "block not found: '^{block_id}'"),
        }
    }
}

/// Resolve a wikilink occurrence against the wiki.
pub fn resolve_wikilink(wikilink: &WikilinkOccurrence, wiki: &Wiki) -> Result<(), ResolveError> {
    let (_, entry) = wiki
        .find(wikilink.page.as_str())
        .ok_or(ResolveError::Broken(BrokenReason::PageNotFound))?;

    if let Some(fragment) = &wikilink.fragment {
        let target_path = wiki.root().path().join(&entry.rel_path);

        match fragment {
            WikilinkFragment::Heading(heading) => {
                let headings = wiki.headings(&target_path).map_err(ResolveError::Wiki)?;
                let found = headings
                    .iter()
                    .any(|h| h.text.eq_ignore_ascii_case(heading));
                if !found {
                    return Err(ResolveError::Broken(BrokenReason::HeadingNotFound {
                        heading: heading.clone(),
                    }));
                }
            }
            WikilinkFragment::Block(block_id) => {
                let block_ids = wiki.block_ids(&target_path).map_err(ResolveError::Wiki)?;
                let found = block_ids.iter().any(|b| b.as_str() == block_id.as_str());
                if !found {
                    return Err(ResolveError::Broken(BrokenReason::BlockNotFound {
                        block_id: block_id.as_str().to_owned(),
                    }));
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("{0}")]
    Broken(BrokenReason),
    #[error(transparent)]
    Wiki(WikiError),
}

/// Check all wikilinks in a file and return the broken ones.
pub fn find_broken_links(
    wiki: &Wiki,
    file_path: &std::path::Path,
) -> Result<Vec<(WikilinkOccurrence, BrokenReason)>, WikiError> {
    let wikilinks = wiki.wikilinks(file_path)?;
    let mut broken = Vec::new();

    for wl in wikilinks {
        if wl.page.as_str().is_empty() {
            continue;
        }
        match resolve_wikilink(wl, wiki) {
            Ok(()) => {}
            Err(ResolveError::Broken(reason)) => broken.push((wl.clone(), reason)),
            Err(ResolveError::Wiki(e)) => return Err(e),
        }
    }

    Ok(broken)
}
