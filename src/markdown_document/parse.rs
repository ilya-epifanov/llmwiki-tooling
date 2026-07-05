use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

use super::{ClassifiedRange, MarkdownLinkDestination, RangeKind};
use crate::page::{BlockId, Heading, PageId, WikilinkFragment, WikilinkOccurrence};

fn parser_options() -> Options {
    Options::ENABLE_WIKILINKS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_HEADING_ATTRIBUTES
}

/// Classify every emitted byte range in the source by structural role.
///
/// Ranges tagged `Prose` are suitable for bare mention scanning.
/// Non-prose ranges (headings, code, frontmatter, wikilinks, etc.) must be left untouched.
pub(super) fn classify_ranges(source: &str) -> Vec<ClassifiedRange> {
    let parser = Parser::new_ext(source, parser_options());
    let offset_iter = parser.into_offset_iter();

    let mut ranges = Vec::new();
    // Stack tracks the current container context.
    // When inside a heading/code/frontmatter container, text events are non-prose.
    let mut context_stack: Vec<RangeKind> = Vec::new();

    for (event, range) in offset_iter {
        match event {
            Event::Start(Tag::MetadataBlock(_)) => {
                context_stack.push(RangeKind::Frontmatter);
            }
            Event::End(TagEnd::MetadataBlock(_)) => {
                context_stack.pop();
                ranges.push(ClassifiedRange {
                    kind: RangeKind::Frontmatter,
                    byte_range: range,
                });
            }

            Event::Start(Tag::Heading { .. }) => {
                context_stack.push(RangeKind::Heading);
            }
            Event::End(TagEnd::Heading(_)) => {
                context_stack.pop();
            }

            Event::Start(Tag::CodeBlock(_)) => {
                context_stack.push(RangeKind::CodeBlock);
            }
            Event::End(TagEnd::CodeBlock) => {
                context_stack.pop();
            }

            Event::Start(Tag::HtmlBlock) => {
                context_stack.push(RangeKind::HtmlBlock);
            }
            Event::End(TagEnd::HtmlBlock) => {
                context_stack.pop();
            }

            // Wikilinks: Link with WikiLink link type
            Event::Start(Tag::Link {
                link_type: LinkType::WikiLink { .. },
                ..
            }) => {
                ranges.push(ClassifiedRange {
                    kind: RangeKind::Wikilink,
                    byte_range: range,
                });
                context_stack.push(RangeKind::Wikilink);
            }
            Event::End(TagEnd::Link) if context_stack.last() == Some(&RangeKind::Wikilink) => {
                context_stack.pop();
            }

            // Embed wikilinks: Image with WikiLink link type
            Event::Start(Tag::Image {
                link_type: LinkType::WikiLink { .. },
                ..
            }) => {
                ranges.push(ClassifiedRange {
                    kind: RangeKind::Embed,
                    byte_range: range,
                });
                context_stack.push(RangeKind::Embed);
            }
            Event::End(TagEnd::Image) if context_stack.last() == Some(&RangeKind::Embed) => {
                context_stack.pop();
            }

            // Autolinks / email links
            Event::Start(Tag::Link {
                link_type: LinkType::Autolink | LinkType::Email,
                ..
            }) => {
                ranges.push(ClassifiedRange {
                    kind: RangeKind::Url,
                    byte_range: range,
                });
                context_stack.push(RangeKind::Url);
            }
            Event::End(TagEnd::Link) if context_stack.last() == Some(&RangeKind::Url) => {
                context_stack.pop();
            }

            // Inline code
            Event::Code(_) => {
                ranges.push(ClassifiedRange {
                    kind: RangeKind::InlineCode,
                    byte_range: range,
                });
            }

            // Inline HTML
            Event::InlineHtml(_) => {
                ranges.push(ClassifiedRange {
                    kind: RangeKind::HtmlInline,
                    byte_range: range,
                });
            }

            // Text events: classify based on current context.
            // Skip recording text inside wikilinks/embeds/urls — the parent Start
            // event already covers the full range.
            Event::Text(_) => {
                let kind = context_stack.last().copied().unwrap_or(RangeKind::Prose);
                match kind {
                    RangeKind::Wikilink | RangeKind::Embed | RangeKind::Url => {}
                    _ => {
                        ranges.push(ClassifiedRange {
                            kind,
                            byte_range: range,
                        });
                    }
                }
            }

            // All other events (paragraph start/end, list items, emphasis, etc.)
            // don't produce classified ranges themselves — their text children do.
            _ => {}
        }
    }

    ranges.sort_by_key(|r| r.byte_range.start);
    ranges
}

/// Extract all wikilink occurrences from the source.
pub(super) fn extract_wikilinks(source: &str) -> Vec<WikilinkOccurrence> {
    let parser = Parser::new_ext(source, parser_options());
    let offset_iter = parser.into_offset_iter();
    let mut wikilinks = Vec::new();

    for (event, range) in offset_iter {
        let dest_url = match &event {
            Event::Start(Tag::Link {
                link_type: LinkType::WikiLink { .. },
                dest_url,
                ..
            })
            | Event::Start(Tag::Image {
                link_type: LinkType::WikiLink { .. },
                dest_url,
                ..
            }) => dest_url.as_ref(),
            _ => continue,
        };

        let (page_str, fragment) = match dest_url.split_once('#') {
            Some((page, frag)) => {
                let fragment = if let Some(block) = frag.strip_prefix('^') {
                    WikilinkFragment::Block(BlockId::from(block))
                } else {
                    WikilinkFragment::Heading(frag.to_owned())
                };
                (page, Some(fragment))
            }
            None => (dest_url, None),
        };

        wikilinks.push(WikilinkOccurrence {
            page: PageId::from(page_str),
            fragment,
            byte_range: range,
        });
    }

    wikilinks
}

/// Extract standard Markdown link destinations from the source.
pub(super) fn extract_markdown_links(source: &str) -> Vec<MarkdownLinkDestination> {
    let mut links = Vec::new();

    for (event, range) in Parser::new_ext(source, parser_options()).into_offset_iter() {
        let dest_url = match event {
            Event::Start(Tag::Link {
                link_type: LinkType::Inline,
                dest_url,
                ..
            })
            | Event::Start(Tag::Image {
                link_type: LinkType::Inline,
                dest_url,
                ..
            }) => dest_url,
            _ => continue,
        };
        if let Some(dest_range) = inline_destination_range(&source[range.clone()], &dest_url) {
            links.push(MarkdownLinkDestination {
                destination: dest_url.to_string(),
                byte_range: range.start + dest_range.start..range.start + dest_range.end,
            });
        }
    }

    links.extend(markdown_reference_definitions(source));
    links
}

fn inline_destination_range(markup: &str, dest_url: &str) -> Option<Range<usize>> {
    let mut start = markup.find("](")? + 2;
    while markup
        .as_bytes()
        .get(start)
        .is_some_and(u8::is_ascii_whitespace)
    {
        start += 1;
    }
    if markup.as_bytes().get(start) == Some(&b'<') {
        let inner_start = start + 1;
        let inner_end = markup[inner_start..].find('>')? + inner_start;
        return (&markup[inner_start..inner_end] == dest_url).then_some(inner_start..inner_end);
    }
    let end = markup[start..]
        .find(|c: char| c == ')' || c.is_whitespace())
        .map(|offset| start + offset)?;
    (&markup[start..end] == dest_url).then_some(start..end)
}

fn markdown_reference_definitions(source: &str) -> Vec<MarkdownLinkDestination> {
    let mut links = Vec::new();
    let mut line_start = 0;
    for line in source.split_inclusive('\n') {
        if let Some((byte_range, destination)) = reference_definition_dest(line, line_start) {
            links.push(MarkdownLinkDestination {
                destination: destination.to_owned(),
                byte_range,
            });
        }
        line_start += line.len();
    }
    links
}

fn reference_definition_dest(line: &str, line_start: usize) -> Option<(Range<usize>, &str)> {
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let label_end = rest.find("]:")?;
    if !rest.starts_with('[') {
        return None;
    }
    let mut dest_start = indent + label_end + 2;
    while line
        .as_bytes()
        .get(dest_start)
        .is_some_and(u8::is_ascii_whitespace)
    {
        dest_start += 1;
    }
    if line.as_bytes().get(dest_start) == Some(&b'<') {
        let inner_start = dest_start + 1;
        let inner_end = line[inner_start..].find('>')? + inner_start;
        let dest = &line[inner_start..inner_end];
        return Some((line_start + inner_start..line_start + inner_end, dest));
    }
    let dest_end = line[dest_start..]
        .find(char::is_whitespace)
        .map(|offset| dest_start + offset)
        .unwrap_or(line.len());
    let dest = &line[dest_start..dest_end];
    (!dest.is_empty()).then_some((line_start + dest_start..line_start + dest_end, dest))
}

/// Extract all headings from the source.
pub(super) fn extract_headings(source: &str) -> Vec<Heading> {
    let parser = Parser::new_ext(source, parser_options());
    let offset_iter = parser.into_offset_iter();
    let mut headings = Vec::new();
    let mut in_heading: Option<(u8, Range<usize>)> = None;
    let mut heading_text = String::new();

    for (event, range) in offset_iter {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = Some((level as u8, range));
                heading_text.clear();
            }
            Event::Text(text) if in_heading.is_some() => {
                heading_text.push_str(&text);
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, start_range)) = in_heading.take() {
                    headings.push(Heading {
                        level,
                        text: std::mem::take(&mut heading_text),
                        byte_range: start_range.start..range.end,
                    });
                }
            }
            _ => {}
        }
    }

    headings
}

/// Extract block IDs (lines like `^block-id`) from the source.
pub(super) fn extract_block_ids(source: &str) -> Vec<BlockId> {
    // Block IDs appear as `^identifier` at the end of a line, typically after content
    // or on their own line. We scan the raw source since pulldown-cmark treats these
    // as regular text.
    let mut block_ids = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(id) = trimmed.strip_prefix('^')
            && !id.is_empty()
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            block_ids.push(BlockId::from(id));
        }
        // Also check for block IDs at end of a line: "content ^block-id"
        if let Some(pos) = trimmed.rfind(" ^") {
            let candidate = &trimmed[pos + 2..];
            if !candidate.is_empty()
                && candidate
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                block_ids.push(BlockId::from(candidate));
            }
        }
    }
    block_ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_frontmatter_as_non_prose() {
        let source = "---\ntitle: Test\ntags: [a]\n---\n\nSome prose here.";
        let ranges = classify_ranges(source);
        let fm_ranges: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == RangeKind::Frontmatter)
            .collect();
        assert!(!fm_ranges.is_empty(), "should have frontmatter ranges");

        let prose_ranges: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == RangeKind::Prose)
            .collect();
        assert!(!prose_ranges.is_empty(), "should have prose ranges");
        // Prose should contain "Some prose here."
        for pr in &prose_ranges {
            let text = &source[pr.byte_range.clone()];
            if text.contains("Some prose") {
                return;
            }
        }
        panic!("prose range should contain 'Some prose here.'");
    }

    #[test]
    fn classifies_wikilinks() {
        let source = "Text with [[GRPO]] and more.";
        let ranges = classify_ranges(source);
        let wl_ranges: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == RangeKind::Wikilink)
            .collect();
        assert_eq!(wl_ranges.len(), 1);
    }

    #[test]
    fn classifies_headings_as_non_prose() {
        let source = "# My Heading\n\nParagraph text.";
        let ranges = classify_ranges(source);
        let heading_ranges: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == RangeKind::Heading)
            .collect();
        assert!(!heading_ranges.is_empty());
        for hr in &heading_ranges {
            let text = &source[hr.byte_range.clone()];
            assert!(
                text.contains("My Heading"),
                "heading range should contain heading text, got: {text:?}"
            );
        }
    }

    #[test]
    fn classifies_code_blocks() {
        let source = "Text before\n\n```rust\nlet x = 1;\n```\n\nText after";
        let ranges = classify_ranges(source);
        let code_ranges: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == RangeKind::CodeBlock)
            .collect();
        assert!(!code_ranges.is_empty());
    }

    #[test]
    fn classifies_inline_code() {
        let source = "Use `GRPO` here.";
        let ranges = classify_ranges(source);
        let code_ranges: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == RangeKind::InlineCode)
            .collect();
        assert_eq!(code_ranges.len(), 1);
        assert_eq!(&source[code_ranges[0].byte_range.clone()], "`GRPO`");
    }

    #[test]
    fn extracts_wikilinks_with_fragments() {
        let source = "See [[post-training#^method-comparison]] for details.";
        let wikilinks = extract_wikilinks(source);
        assert_eq!(wikilinks.len(), 1);
        assert_eq!(wikilinks[0].page.as_str(), "post-training");
        assert_eq!(
            wikilinks[0].fragment,
            Some(WikilinkFragment::Block(BlockId::from("method-comparison")))
        );
    }

    #[test]
    fn extracts_embed_wikilinks() {
        let source = "![[post-training#^method-comparison]]";
        let wikilinks = extract_wikilinks(source);
        assert_eq!(wikilinks.len(), 1);
        assert_eq!(wikilinks[0].page.as_str(), "post-training");
    }

    #[test]
    fn extracts_heading_fragment() {
        let source = "See [[page#Some Heading]] for details.";
        let wikilinks = extract_wikilinks(source);
        assert_eq!(wikilinks.len(), 1);
        assert_eq!(
            wikilinks[0].fragment,
            Some(WikilinkFragment::Heading("Some Heading".to_owned()))
        );
    }

    #[test]
    fn extracts_headings() {
        let source = "# Title\n\nParagraph\n\n## Section One\n\nMore text\n\n### Sub Section";
        let headings = extract_headings(source);
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "Title");
        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[1].text, "Section One");
        assert_eq!(headings[2].level, 3);
        assert_eq!(headings[2].text, "Sub Section");
    }

    #[test]
    fn extracts_block_ids() {
        let source = "Some content\n\n^method-comparison\n\nMore content ^inline-block";
        let block_ids = extract_block_ids(source);
        assert_eq!(block_ids.len(), 2);
        assert_eq!(block_ids[0].as_str(), "method-comparison");
        assert_eq!(block_ids[1].as_str(), "inline-block");
    }
}
