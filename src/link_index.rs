use std::collections::{HashMap, HashSet};

use crate::error::WikiError;
use crate::page::{PageId, WikilinkOccurrence};
use crate::wiki::Wiki;

/// Directed link graph across all wiki pages.
#[derive(Debug)]
pub struct LinkIndex {
    outbound: HashMap<PageId, Vec<WikilinkOccurrence>>,
    inbound: HashMap<PageId, HashSet<PageId>>,
}

impl LinkIndex {
    /// Build the link index by scanning all wiki files.
    pub fn build(wiki: &Wiki) -> Result<Self, WikiError> {
        let mut outbound: HashMap<PageId, Vec<WikilinkOccurrence>> = HashMap::new();
        let mut inbound: HashMap<PageId, HashSet<PageId>> = HashMap::new();

        for page_id in wiki.pages().keys() {
            inbound.entry(page_id.clone()).or_default();
        }

        for file_path in wiki.all_scannable_files() {
            let source_page = PageId::from_path(&file_path);
            let wikilinks = wiki.wikilinks(&file_path)?;

            if let Some(source_id) = &source_page {
                for wl in wikilinks {
                    inbound
                        .entry(wl.page.clone())
                        .or_default()
                        .insert(source_id.clone());
                }
                outbound.insert(source_id.clone(), wikilinks.to_vec());
            }
        }

        Ok(Self { outbound, inbound })
    }

    pub fn inbound(&self, page: &PageId) -> Vec<&PageId> {
        self.inbound
            .get(page)
            .map(|set| set.iter().collect())
            .unwrap_or_default()
    }

    pub fn outbound(&self, page: &PageId) -> &[WikilinkOccurrence] {
        self.outbound
            .get(page)
            .map(|v| v.as_slice())
            .unwrap_or_default()
    }

    pub fn orphans(&self, wiki: &Wiki) -> Vec<PageId> {
        let mut orphans: Vec<PageId> = wiki
            .pages()
            .keys()
            .filter(|id| {
                self.inbound
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
