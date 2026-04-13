use std::collections::HashSet;
use std::ops::Range;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};

use crate::page::PageId;
use crate::parse::{ClassifiedRange, RangeKind};
use crate::splice;

/// A bare concept mention found in prose that should be a wikilink.
#[derive(Debug)]
pub struct BareMention {
    pub concept: PageId,
    pub byte_range: Range<usize>,
    pub line: usize,
    pub col: usize,
}

/// Efficient multi-pattern matcher for auto-linkable page names.
pub struct ConceptMatcher {
    automaton: AhoCorasick,
    concepts: Vec<PageId>,
}

impl ConceptMatcher {
    pub fn new(pages: &HashSet<PageId>) -> Self {
        let concept_list: Vec<PageId> = pages.iter().cloned().collect();
        let patterns: Vec<&str> = concept_list.iter().map(|c| c.as_str()).collect();
        // Always case-insensitive since PageIds are normalized to lowercase
        let automaton = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .ascii_case_insensitive(true)
            .build(&patterns)
            .expect("concept patterns are valid");
        Self {
            automaton,
            concepts: concept_list,
        }
    }

    /// Find all bare mentions in a page's prose ranges.
    pub fn find_bare_mentions(
        &self,
        source: &str,
        classified_ranges: &[ClassifiedRange],
        self_page: &PageId,
    ) -> Vec<BareMention> {
        let line_offsets = splice::compute_line_offsets(source);
        let mut mentions = Vec::new();

        for cr in classified_ranges {
            if cr.kind != RangeKind::Prose {
                continue;
            }

            let slice = &source[cr.byte_range.clone()];

            for mat in self.automaton.find_iter(slice) {
                let concept = &self.concepts[mat.pattern().as_usize()];

                if concept == self_page {
                    continue;
                }

                let abs_start = cr.byte_range.start + mat.start();
                let abs_end = cr.byte_range.start + mat.end();

                // Word boundary checks use byte indexing on ASCII-only characters.
                // Safe because aho-corasick returns byte-aligned positions and we
                // only inspect ASCII punctuation/alphanumeric at those boundaries.
                if abs_start > 0 {
                    let prev = source.as_bytes()[abs_start - 1];
                    if prev.is_ascii_alphanumeric() || prev == b'_' {
                        continue;
                    }
                    if prev == b'-' && abs_start >= 2 {
                        let before_dash = source.as_bytes()[abs_start - 2];
                        if before_dash.is_ascii_alphanumeric() {
                            continue;
                        }
                    }
                }

                if abs_end < source.len() {
                    let next = source.as_bytes()[abs_end];
                    if next.is_ascii_alphanumeric() || next == b'_' {
                        continue;
                    }
                    if next == b'-' && abs_end + 1 < source.len() {
                        let after_dash = source.as_bytes()[abs_end + 1];
                        if after_dash.is_ascii_alphanumeric() {
                            continue;
                        }
                    }
                }

                let line_0 = splice::offset_to_line(&line_offsets, abs_start);
                let col = abs_start - line_offsets[line_0];
                mentions.push(BareMention {
                    concept: concept.clone(),
                    byte_range: abs_start..abs_end,
                    line: line_0 + 1,
                    col: col + 1,
                });
            }
        }

        mentions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_matcher(names: &[&str]) -> ConceptMatcher {
        let concepts: HashSet<PageId> = names.iter().map(|&n| PageId::from(n)).collect();
        ConceptMatcher::new(&concepts)
    }

    fn prose_range(start: usize, end: usize) -> ClassifiedRange {
        ClassifiedRange {
            kind: RangeKind::Prose,
            byte_range: start..end,
        }
    }

    #[test]
    fn finds_bare_mention() {
        let source = "Use GRPO for training.";
        let matcher = make_matcher(&["GRPO"]);
        let ranges = vec![prose_range(0, source.len())];
        let mentions = matcher.find_bare_mentions(source, &ranges, &PageId::from("other"));
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].concept.as_str(), "grpo");
    }

    #[test]
    fn skips_self_page() {
        let source = "GRPO is great.";
        let matcher = make_matcher(&["GRPO"]);
        let ranges = vec![prose_range(0, source.len())];
        let mentions = matcher.find_bare_mentions(source, &ranges, &PageId::from("GRPO"));
        assert!(mentions.is_empty());
    }

    #[test]
    fn skips_compound_terms_suffix() {
        let source = "GRPO-based approach";
        let matcher = make_matcher(&["GRPO"]);
        let ranges = vec![prose_range(0, source.len())];
        let mentions = matcher.find_bare_mentions(source, &ranges, &PageId::from("other"));
        assert!(mentions.is_empty());
    }

    #[test]
    fn skips_compound_terms_prefix() {
        let source = "SA-SFT and Mix-CPT are methods";
        let matcher = make_matcher(&["SFT", "CPT"]);
        let ranges = vec![prose_range(0, source.len())];
        let mentions = matcher.find_bare_mentions(source, &ranges, &PageId::from("other"));
        assert!(mentions.is_empty());
    }

    #[test]
    fn skips_word_boundary_violations() {
        let source = "xGRPO and GRPOx";
        let matcher = make_matcher(&["GRPO"]);
        let ranges = vec![prose_range(0, source.len())];
        let mentions = matcher.find_bare_mentions(source, &ranges, &PageId::from("other"));
        assert!(mentions.is_empty());
    }

    #[test]
    fn finds_multiple_concepts() {
        let source = "DPO and GRPO are methods.";
        let matcher = make_matcher(&["DPO", "GRPO"]);
        let ranges = vec![prose_range(0, source.len())];
        let mentions = matcher.find_bare_mentions(source, &ranges, &PageId::from("other"));
        assert_eq!(mentions.len(), 2);
    }

    #[test]
    fn skips_non_prose_ranges() {
        let source = "GRPO in heading";
        let ranges = vec![ClassifiedRange {
            kind: RangeKind::Heading,
            byte_range: 0..source.len(),
        }];
        let matcher = make_matcher(&["GRPO"]);
        let mentions = matcher.find_bare_mentions(source, &ranges, &PageId::from("other"));
        assert!(mentions.is_empty());
    }

    #[test]
    fn reports_correct_line_col() {
        let source = "line one\nGRPO here";
        let matcher = make_matcher(&["GRPO"]);
        let ranges = vec![prose_range(9, source.len())];
        let mentions = matcher.find_bare_mentions(source, &ranges, &PageId::from("other"));
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].line, 2);
        assert_eq!(mentions[0].col, 1);
    }

    #[test]
    fn allows_concept_followed_by_punctuation() {
        let source = "Use GRPO, DPO.";
        let matcher = make_matcher(&["GRPO", "DPO"]);
        let ranges = vec![prose_range(0, source.len())];
        let mentions = matcher.find_bare_mentions(source, &ranges, &PageId::from("other"));
        assert_eq!(mentions.len(), 2);
    }

    #[test]
    fn case_insensitive_matching() {
        let source = "Use grpo for training.";
        let concepts: HashSet<PageId> = ["GRPO"].iter().map(|&n| PageId::from(n)).collect();
        let matcher = ConceptMatcher::new(&concepts);
        let ranges = vec![prose_range(0, source.len())];
        let mentions = matcher.find_bare_mentions(source, &ranges, &PageId::from("other"));
        assert_eq!(mentions.len(), 1);
    }
}
