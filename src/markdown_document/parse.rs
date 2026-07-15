use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

use super::{ClassifiedRange, MarkdownLinkDestination, MarkdownReferenceDefinition, RangeKind};
use crate::markdown_links;
use crate::page::{
    BlockId, Heading, InternalLinkOccurrence, InternalLinkTarget, LinkFragment, LinkStyle, PageId,
    WikilinkOccurrence,
};

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
            Event::Start(Tag::Link { .. }) | Event::Start(Tag::Image { .. }) => {
                ranges.push(ClassifiedRange {
                    kind: RangeKind::Url,
                    byte_range: range,
                });
                context_stack.push(RangeKind::Url);
            }
            Event::End(TagEnd::Link) if context_stack.last() == Some(&RangeKind::Url) => {
                context_stack.pop();
            }
            Event::End(TagEnd::Image) if context_stack.last() == Some(&RangeKind::Url) => {
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
    extract_internal_links(source)
        .into_iter()
        .filter_map(|link| match link.target {
            InternalLinkTarget::PageName(page) => Some(WikilinkOccurrence {
                page,
                fragment: link.fragment,
                byte_range: link.byte_range,
            }),
            InternalLinkTarget::Path(_) => None,
        })
        .collect()
}

/// Extract Obsidian and Markdown internal links into one syntax-neutral view.
pub(super) fn extract_internal_links(source: &str) -> Vec<InternalLinkOccurrence> {
    Parser::new_ext(source, parser_options())
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                id,
                ..
            }) => internal_link(source, range, link_type, &dest_url, &id, false),
            Event::Start(Tag::Image {
                link_type: LinkType::WikiLink { .. },
                dest_url,
                id,
                ..
            }) => internal_link(
                source,
                range,
                LinkType::WikiLink {
                    has_pothole: !id.is_empty(),
                },
                &dest_url,
                &id,
                true,
            ),
            _ => None,
        })
        .collect()
}

fn internal_link(
    source: &str,
    range: Range<usize>,
    link_type: LinkType,
    destination: &str,
    reference_id: &str,
    embed: bool,
) -> Option<InternalLinkOccurrence> {
    let markup = &source[range.clone()];
    let (style, target, fragment) = match link_type {
        LinkType::WikiLink { .. } => {
            let (page, fragment) = split_fragment(destination);
            (
                LinkStyle::Obsidian,
                InternalLinkTarget::PageName(PageId::from(page)),
                fragment,
            )
        }
        LinkType::Inline | LinkType::Reference | LinkType::Collapsed | LinkType::Shortcut => {
            let (path, fragment) = split_fragment(destination);
            if !path.is_empty() && !markdown_links::is_relative_md_path(path) {
                return None;
            }
            (
                LinkStyle::Markdown,
                InternalLinkTarget::Path(path.to_owned()),
                fragment,
            )
        }
        _ => return None,
    };

    let destination_range = (style == LinkStyle::Markdown && link_type == LinkType::Inline)
        .then(|| inline_destination_range(markup, destination))
        .flatten()
        .map(|dest| range.start + dest.start..range.start + dest.end);
    Some(InternalLinkOccurrence {
        style,
        target,
        fragment,
        display_text: link_text(markup, style),
        byte_range: range,
        destination_range,
        reference_label: matches!(
            link_type,
            LinkType::Reference | LinkType::Collapsed | LinkType::Shortcut
        )
        .then(|| reference_id.to_owned()),
        embed,
    })
}

fn split_fragment(destination: &str) -> (&str, Option<LinkFragment>) {
    let (page, fragment) =
        destination
            .split_once('#')
            .map_or((destination, None), |(page, fragment)| {
                let fragment = fragment
                    .strip_prefix('^')
                    .map(|block| LinkFragment::Block(BlockId::from(block)))
                    .unwrap_or_else(|| LinkFragment::Heading(fragment.to_owned()));
                (page, Some(fragment))
            });
    (page, fragment)
}

fn link_text(markup: &str, style: LinkStyle) -> String {
    if style == LinkStyle::Obsidian {
        let inner = markup
            .trim_start_matches('!')
            .strip_prefix("[[")
            .and_then(|text| text.strip_suffix("]]"))
            .unwrap_or(markup);
        return inner
            .split_once('|')
            .map_or_else(
                || inner.split('#').next().unwrap_or(inner),
                |(_, alias)| alias,
            )
            .to_owned();
    }

    let Some(start) = markup.find('[') else {
        return markup.to_owned();
    };
    let mut escaped = false;
    let mut depth = 0;
    for (offset, ch) in markup[start + 1..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '[' => depth += 1,
            ']' if depth == 0 => return markup[start + 1..start + 1 + offset].to_owned(),
            ']' => depth -= 1,
            _ => {}
        }
    }
    markup.to_owned()
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

    let parser = Parser::new_ext(source, parser_options());
    links.extend(
        parser
            .reference_definitions()
            .iter()
            .filter_map(|(_, definition)| {
                reference_definition_destination_range(source, definition.span.clone()).map(
                    |byte_range| MarkdownLinkDestination {
                        destination: definition.dest.to_string(),
                        byte_range,
                    },
                )
            }),
    );
    links
}

pub(super) fn extract_reference_definitions(source: &str) -> Vec<MarkdownReferenceDefinition> {
    let parser = Parser::new_ext(source, parser_options());
    let mut definitions = parser
        .reference_definitions()
        .iter()
        .map(|(label, definition)| MarkdownReferenceDefinition {
            label: label.to_owned(),
            destination: definition.dest.to_string(),
            byte_range: definition.span.clone(),
        })
        .collect::<Vec<_>>();
    definitions.sort_by_key(|definition| definition.byte_range.start);
    definitions
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
        let inner_end = find_unescaped(&markup[inner_start..], '>')? + inner_start;
        return (&markup[inner_start..inner_end] == dest_url).then_some(inner_start..inner_end);
    }
    let mut escaped = false;
    let mut parentheses = 0;
    let mut end = start;
    for (offset, character) in markup[start..].char_indices() {
        if escaped {
            escaped = false;
            end = start + offset + character.len_utf8();
            continue;
        }
        match character {
            '\\' => escaped = true,
            '(' => parentheses += 1,
            ')' if parentheses == 0 => break,
            ')' => parentheses -= 1,
            character if character.is_whitespace() && parentheses == 0 => break,
            _ => {}
        }
        end = start + offset + character.len_utf8();
    }
    (end > start).then_some(start..end)
}

fn reference_definition_destination_range(
    source: &str,
    span: Range<usize>,
) -> Option<Range<usize>> {
    let definition = &source[span.clone()];
    let label_start = definition.find('[')? + 1;
    let mut escaped = false;
    let label_end = definition[label_start..]
        .char_indices()
        .find_map(|(offset, character)| {
            if escaped {
                escaped = false;
                return None;
            }
            if character == '\\' {
                escaped = true;
                return None;
            }
            (character == ']' && definition.as_bytes().get(label_start + offset + 1) == Some(&b':'))
                .then_some(label_start + offset)
        })?;
    let mut start = label_end + 2;
    while definition
        .as_bytes()
        .get(start)
        .is_some_and(u8::is_ascii_whitespace)
    {
        start += 1;
    }
    if definition.as_bytes().get(start) == Some(&b'<') {
        let inner_start = start + 1;
        let inner_end = find_unescaped(&definition[inner_start..], '>')? + inner_start;
        return Some(span.start + inner_start..span.start + inner_end);
    }

    let mut escaped = false;
    let mut parentheses = 0;
    let mut end = start;
    for (offset, character) in definition[start..].char_indices() {
        if escaped {
            escaped = false;
            end = start + offset + character.len_utf8();
            continue;
        }
        match character {
            '\\' => escaped = true,
            '(' => parentheses += 1,
            ')' if parentheses > 0 => parentheses -= 1,
            character if character.is_whitespace() && parentheses == 0 => break,
            _ => {}
        }
        end = start + offset + character.len_utf8();
    }
    (end > start).then_some(span.start + start..span.start + end)
}

fn find_unescaped(source: &str, needle: char) -> Option<usize> {
    let mut escaped = false;
    source.char_indices().find_map(|(offset, character)| {
        if escaped {
            escaped = false;
            return None;
        }
        if character == '\\' {
            escaped = true;
            return None;
        }
        (character == needle).then_some(offset)
    })
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
            Event::Text(text) | Event::Code(text) if in_heading.is_some() => {
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
            Some(LinkFragment::Block(BlockId::from("method-comparison")))
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
            Some(LinkFragment::Heading("Some Heading".to_owned()))
        );
    }

    #[test]
    fn extracts_both_internal_link_styles() {
        let source = "See [[Page#Heading|wiki]], [inline](topics/Page.md#heading), [full][Page#Heading], and [Page].\n\n[Page#Heading]: topics/Page.md#heading\n[Page]: topics/Page.md\n";
        let links = extract_internal_links(source);

        assert_eq!(links.len(), 4);
        assert_eq!(links[0].style, LinkStyle::Obsidian);
        assert_eq!(
            links[0].target,
            InternalLinkTarget::PageName(PageId::from("Page"))
        );
        assert_eq!(
            links[0].fragment,
            Some(LinkFragment::Heading("Heading".to_owned()))
        );
        assert_eq!(links[0].display_text, "wiki");
        assert_eq!(links[1].style, LinkStyle::Markdown);
        assert_eq!(
            links[1].target,
            InternalLinkTarget::Path("topics/Page.md".to_owned())
        );
        assert_eq!(
            links[1].fragment,
            Some(LinkFragment::Heading("heading".to_owned()))
        );
        assert_eq!(links[1].display_text, "inline");
        assert_eq!(links[2].reference_label.as_deref(), Some("Page#Heading"));
        assert_eq!(links[3].reference_label.as_deref(), Some("Page"));
    }

    #[test]
    fn excludes_external_markdown_documents_from_internal_links() {
        let source = "See [remote](https://example.com/readme.md), [file](file:readme.md), [ftp](ftp:readme.md), [encoded](topics/readme%2Emd), and [local](topics/readme.md).";
        let links = extract_internal_links(source);

        assert_eq!(links.len(), 2);
        assert_eq!(
            links[0].target,
            InternalLinkTarget::Path("topics/readme%2Emd".to_owned())
        );
        assert_eq!(
            links[1].target,
            InternalLinkTarget::Path("topics/readme.md".to_owned())
        );
    }

    #[test]
    fn extracts_headings() {
        let source = "# Title\n\nParagraph\n\n## Section One\n\nMore text\n\n### Use `inline code`";
        let headings = extract_headings(source);
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "Title");
        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[1].text, "Section One");
        assert_eq!(headings[2].level, 3);
        assert_eq!(headings[2].text, "Use inline code");
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
