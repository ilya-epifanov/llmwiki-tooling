use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::error::WikiError;
use crate::page::{InternalLinkOccurrence, LinkFragment, PageId, WikilinkOccurrence};
use crate::wiki::Wiki;

/// Why a wikilink could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokenReason {
    Page,
    Heading { heading: String },
    Block { block_id: String },
}

impl std::fmt::Display for BrokenReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Page => write!(f, "page not found"),
            Self::Heading { heading } => write!(f, "heading not found: '{heading}'"),
            Self::Block { block_id } => write!(f, "block not found: '^{block_id}'"),
        }
    }
}

/// A wikilink that does not resolve against the wiki.
#[derive(Debug, Clone)]
pub struct BrokenLink {
    pub source_path: PathBuf,
    pub link: InternalLinkOccurrence,
    pub reason: BrokenReason,
}

/// Directed link graph across all wiki pages.
#[derive(Debug)]
pub struct LinkIndex {
    outbound: HashMap<PageId, Vec<WikilinkOccurrence>>,
    inbound_paths: HashMap<PageId, BTreeSet<PathBuf>>,
    broken_links: Vec<BrokenLink>,
}

impl LinkIndex {
    /// Build the link index by scanning all wiki files.
    pub fn build(wiki: &Wiki) -> Result<Self, WikiError> {
        let mut outbound: HashMap<PageId, Vec<WikilinkOccurrence>> = HashMap::new();
        let mut inbound_paths: HashMap<PageId, BTreeSet<PathBuf>> = HashMap::new();
        let mut broken_links = Vec::new();

        for page_id in wiki.pages().keys() {
            inbound_paths.entry(page_id.clone()).or_default();
        }

        for file_path in wiki.scannable_files() {
            let Some(source_id) = PageId::from_path(file_path) else {
                continue;
            };
            let source_path = wiki.rel_path(file_path).to_path_buf();
            let outbound_links = outbound.entry(source_id).or_default();
            let document = wiki.file(file_path)?;

            for link in document.internal_links() {
                let Some((target, target_path)) = wiki.resolve_internal_link(file_path, link)
                else {
                    broken_links.push(BrokenLink {
                        source_path: file_path.clone(),
                        link: link.clone(),
                        reason: BrokenReason::Page,
                    });
                    continue;
                };
                let target = target.clone();
                inbound_paths
                    .entry(target.clone())
                    .or_default()
                    .insert(source_path.clone());
                outbound_links.push(WikilinkOccurrence {
                    page: target,
                    fragment: link.fragment.clone(),
                    byte_range: link.byte_range.clone(),
                });

                if let Some(reason) = broken_reason(wiki, target_path, link)? {
                    broken_links.push(BrokenLink {
                        source_path: file_path.clone(),
                        link: link.clone(),
                        reason,
                    });
                }
            }
        }

        Ok(Self {
            outbound,
            inbound_paths,
            broken_links,
        })
    }

    pub fn outbound(&self, page: &PageId) -> &[WikilinkOccurrence] {
        self.outbound
            .get(page)
            .map(|v| v.as_slice())
            .unwrap_or_default()
    }

    pub fn inbound_paths(&self, page: &PageId) -> Vec<&Path> {
        self.inbound_paths
            .get(page)
            .map(|set| set.iter().map(PathBuf::as_path).collect())
            .unwrap_or_default()
    }

    pub fn broken_links(&self) -> &[BrokenLink] {
        &self.broken_links
    }

    pub fn orphans(&self, wiki: &Wiki) -> Vec<PageId> {
        let mut orphans: Vec<PageId> = wiki
            .pages()
            .keys()
            .filter(|id| {
                self.inbound_paths
                    .get(*id)
                    .map(|set| set.is_empty())
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        orphans.sort();
        orphans
    }

    pub fn all_edges(&self) -> Vec<(&PageId, &PageId)> {
        let mut edges = Vec::new();
        for (source, wikilinks) in &self.outbound {
            for wl in wikilinks {
                edges.push((source, &wl.page));
            }
        }
        edges.sort();
        edges
    }
}

fn broken_reason(
    wiki: &Wiki,
    target_path: &Path,
    link: &InternalLinkOccurrence,
) -> Result<Option<BrokenReason>, WikiError> {
    let Some(fragment) = &link.fragment else {
        return Ok(None);
    };
    let target_path = wiki.abs_path(target_path);

    match fragment {
        LinkFragment::Heading(heading) => Ok((wiki
            .file(&target_path)?
            .resolve_heading(heading, link.style)
            .is_none())
        .then(|| BrokenReason::Heading {
            heading: heading.clone(),
        })),
        LinkFragment::Block(block_id) => Ok((!wiki
            .file(&target_path)?
            .block_ids()
            .iter()
            .any(|b| b.as_str() == block_id.as_str()))
        .then(|| BrokenReason::Block {
            block_id: block_id.as_str().to_owned(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WikiConfig;
    use crate::page::Heading;
    use crate::wiki::WikiRoot;

    #[test]
    fn records_broken_heading_fragments() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("wiki")).unwrap();
        std::fs::write(dir.path().join("index.md"), "[[Target#Missing]]\n").unwrap();
        std::fs::write(
            dir.path().join("wiki/Target.md"),
            "# Target\n\n## Present\n",
        )
        .unwrap();

        let root = WikiRoot::from_path(dir.path().to_path_buf()).unwrap();
        let config = WikiConfig::auto_detect(root.path());
        let wiki = Wiki::build(root, config).unwrap();
        let index = LinkIndex::build(&wiki).unwrap();

        assert_eq!(index.broken_links().len(), 1);
        assert_eq!(
            index.broken_links()[0].reason,
            BrokenReason::Heading {
                heading: "Missing".to_owned()
            }
        );
    }

    #[test]
    fn github_compatible_anchors_preserve_unicode_and_disambiguate_duplicates() {
        let headings = [
            "This'll be a Helpful Section About the Greek Letter Θ!",
            "Repeated",
            "Repeated",
            "Repeated-1",
            "Repeated",
        ]
        .into_iter()
        .map(|text| Heading {
            level: 2,
            text: text.to_owned(),
            byte_range: 0..0,
        })
        .collect::<Vec<_>>();

        assert_eq!(
            crate::page::github_heading_anchors(&headings).collect::<Vec<_>>(),
            [
                "thisll-be-a-helpful-section-about-the-greek-letter-Θ",
                "repeated",
                "repeated-1",
                "repeated-1-1",
                "repeated-2",
            ]
        );
    }
}
