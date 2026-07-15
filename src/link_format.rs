use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};

use crate::error::WikiError;
use crate::markdown_document::MarkdownReferenceDefinition;
use crate::markdown_links;
use crate::mention::BareMention;
use crate::page::{InternalLinkOccurrence, LinkFragment, LinkStyle, PageId};
use crate::splice;
use crate::wiki::Wiki;

pub(crate) struct FormatOutcome {
    pub edits: Vec<(Range<usize>, String)>,
    pub skipped: usize,
}

#[derive(Clone)]
struct ResolvedLink {
    occurrence: InternalLinkOccurrence,
    page: PageId,
    target_path: PathBuf,
    display_name: String,
    fragment: Option<LinkFragment>,
}

struct GeneratedDefinition {
    key: (PageId, Option<LinkFragment>),
    label: String,
    destination: String,
}

pub(crate) fn format_document(wiki: &Wiki, file_path: &Path) -> Result<FormatOutcome, WikiError> {
    let document = wiki.file(file_path)?;
    let source = document.source();
    let mut skipped = 0;
    let mut resolved = Vec::new();

    for link in document.internal_links().iter().filter(|link| !link.embed) {
        let Some((page, target_path)) = wiki.resolve_internal_link(file_path, link) else {
            skipped += 1;
            continue;
        };
        let Some(fragment) = canonical_fragment(wiki, target_path, link)? else {
            skipped += 1;
            continue;
        };
        resolved.push(ResolvedLink {
            occurrence: link.clone(),
            page: page.clone(),
            target_path: target_path.clone(),
            display_name: wiki.display_name(page).unwrap_or(page.as_str()).to_owned(),
            fragment,
        });
    }

    let target_counts = resolved.iter().fold(HashMap::new(), |mut counts, link| {
        *counts.entry(link.page.clone()).or_insert(0) += 1;
        counts
    });
    let reference_uses = reference_use_labels(source);
    let converted_reference_labels = resolved
        .iter()
        .filter_map(|link| link.occurrence.reference_label.clone())
        .collect::<Vec<_>>();
    let removable_definitions = document
        .reference_definitions()
        .iter()
        .filter(|definition| {
            let total = reference_uses
                .iter()
                .filter(|label| reference_labels_match(label, &definition.label))
                .count();
            total > 0
                && total
                    == converted_reference_labels
                        .iter()
                        .filter(|label| reference_labels_match(label, &definition.label))
                        .count()
        })
        .collect::<Vec<_>>();
    let mut reserved_labels = document
        .reference_definitions()
        .iter()
        .filter(|definition| {
            !removable_definitions
                .iter()
                .any(|removed| removed.byte_range == definition.byte_range)
        })
        .map(|definition| definition.label.clone())
        .collect::<Vec<_>>();

    let mut definitions = Vec::new();
    let mut edits = Vec::new();
    for link in &resolved {
        let replacement = match wiki.config().linking.link_style {
            LinkStyle::Obsidian => obsidian_link(link),
            LinkStyle::Markdown => {
                let threshold = wiki.config().linking.reference_style_threshold;
                if threshold.is_some_and(|threshold| target_counts[&link.page] >= threshold.get()) {
                    reference_link(
                        file_path,
                        wiki,
                        link,
                        &mut definitions,
                        &mut reserved_labels,
                    )
                } else {
                    inline_markdown_link(file_path, wiki, link)
                }
            }
        };
        if source[link.occurrence.byte_range.clone()] != replacement {
            edits.push((link.occurrence.byte_range.clone(), replacement));
        }
    }

    for definition in removable_definitions {
        edits.push((definition_line_range(source, definition), String::new()));
    }

    append_definitions(
        source,
        &mut edits,
        definitions
            .iter()
            .map(|definition| (definition.label.as_str(), definition.destination.as_str())),
    );

    if splice::apply(source, &edits) == source {
        edits.clear();
    }
    Ok(FormatOutcome { edits, skipped })
}

pub(crate) fn format_mentions(
    wiki: &Wiki,
    file_path: &Path,
    mentions: &[BareMention],
) -> Result<Vec<(Range<usize>, String)>, WikiError> {
    let source = wiki.file(file_path)?.source();
    let mut target_counts = HashMap::new();
    for link in wiki.file(file_path)?.internal_links() {
        if let Some((page, _)) = wiki.resolve_internal_link(file_path, link) {
            *target_counts.entry(page.clone()).or_insert(0) += 1;
        }
    }
    for mention in mentions {
        *target_counts.entry(mention.concept.clone()).or_insert(0) += 1;
    }

    let mut reserved_labels = wiki
        .file(file_path)?
        .reference_definitions()
        .iter()
        .map(|definition| definition.label.clone())
        .collect::<Vec<_>>();
    let mut definitions: Vec<(PageId, String, String)> = Vec::new();
    let mut edits = Vec::new();
    for mention in mentions {
        let display = wiki
            .display_name(&mention.concept)
            .unwrap_or(mention.concept.as_str());
        let replacement = match wiki.config().linking.link_style {
            LinkStyle::Obsidian => format!("[[{display}]]"),
            LinkStyle::Markdown => {
                let target_path = wiki
                    .get(&mention.concept)
                    .expect("autolink candidate remains a managed page");
                let destination = markdown_page_destination(file_path, wiki, target_path);
                if wiki
                    .config()
                    .linking
                    .reference_style_threshold
                    .is_some_and(|threshold| target_counts[&mention.concept] >= threshold.get())
                {
                    let label = definitions
                        .iter()
                        .find(|(page, _, _)| page == &mention.concept)
                        .map(|(_, label, _)| label.clone())
                        .unwrap_or_else(|| {
                            let label =
                                unique_reference_label(display.to_owned(), &reserved_labels);
                            reserved_labels.push(label.clone());
                            definitions.push((mention.concept.clone(), label.clone(), destination));
                            label
                        });
                    let escaped = escape_reference_label(&label);
                    if display == label {
                        format!("[{escaped}]")
                    } else {
                        format!("[{display}][{escaped}]")
                    }
                } else {
                    format!("[{display}]({destination})")
                }
            }
        };
        edits.push((mention.byte_range.clone(), replacement));
    }
    append_definitions(
        source,
        &mut edits,
        definitions
            .iter()
            .map(|(_, label, destination)| (label.as_str(), destination.as_str())),
    );
    Ok(edits)
}

fn append_definitions<'a>(
    source: &str,
    edits: &mut Vec<(Range<usize>, String)>,
    definitions: impl Iterator<Item = (&'a str, &'a str)>,
) {
    let footer = definitions
        .map(|(label, destination)| format!("[{}]: {destination}\n", escape_reference_label(label)))
        .collect::<String>();
    if footer.is_empty() {
        return;
    }
    let without_definitions = splice::apply(source, edits);
    let separator = if without_definitions.ends_with("\n\n") {
        ""
    } else if without_definitions.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    edits.push((source.len()..source.len(), format!("{separator}{footer}")));
}

fn canonical_fragment(
    wiki: &Wiki,
    target_path: &Path,
    link: &InternalLinkOccurrence,
) -> Result<Option<Option<LinkFragment>>, WikiError> {
    let Some(fragment) = &link.fragment else {
        return Ok(Some(None));
    };
    let document = wiki.file(&wiki.abs_path(target_path))?;
    match fragment {
        LinkFragment::Heading(fragment) => {
            let heading = document.resolve_heading(fragment, link.style);
            if wiki.config().linking.link_style == LinkStyle::Obsidian
                && link.style == LinkStyle::Markdown
                && heading.is_some_and(|resolved| {
                    document
                        .headings()
                        .iter()
                        .find(|candidate| candidate.text.eq_ignore_ascii_case(&resolved.text))
                        .is_some_and(|first| first.byte_range != resolved.byte_range)
                })
            {
                return Ok(None);
            }
            Ok(heading.map(|heading| Some(LinkFragment::Heading(heading.text.clone()))))
        }
        LinkFragment::Block(block) => Ok(document
            .block_ids()
            .iter()
            .any(|candidate| candidate == block)
            .then(|| Some(LinkFragment::Block(block.clone())))),
    }
}

fn obsidian_link(link: &ResolvedLink) -> String {
    let target = logical_label(&link.display_name, link.fragment.as_ref());
    if link.occurrence.display_text == link.display_name && link.fragment.is_none() {
        format!("[[{target}]]")
    } else {
        format!("[[{target}|{}]]", link.occurrence.display_text)
    }
}

fn inline_markdown_link(file_path: &Path, wiki: &Wiki, link: &ResolvedLink) -> String {
    format!(
        "[{}]({})",
        link.occurrence.display_text,
        markdown_destination(file_path, wiki, link)
    )
}

fn reference_link(
    file_path: &Path,
    wiki: &Wiki,
    link: &ResolvedLink,
    definitions: &mut Vec<GeneratedDefinition>,
    reserved_labels: &mut Vec<String>,
) -> String {
    let key = (link.page.clone(), link.fragment.clone());
    let label = definitions
        .iter()
        .find(|definition| definition.key == key)
        .map(|definition| definition.label.clone())
        .unwrap_or_else(|| {
            let label = unique_reference_label(
                logical_label(&link.display_name, link.fragment.as_ref()),
                reserved_labels,
            );
            reserved_labels.push(label.clone());
            definitions.push(GeneratedDefinition {
                key,
                label: label.clone(),
                destination: markdown_destination(file_path, wiki, link),
            });
            label
        });
    let label = escape_reference_label(&label);
    if link.occurrence.display_text == label {
        format!("[{label}]")
    } else {
        format!("[{}][{label}]", link.occurrence.display_text)
    }
}

fn markdown_destination(file_path: &Path, wiki: &Wiki, link: &ResolvedLink) -> String {
    let mut destination = markdown_page_destination(file_path, wiki, &link.target_path);
    if let Some(fragment) = &link.fragment {
        destination.push('#');
        match fragment {
            LinkFragment::Heading(heading) => {
                let document = wiki
                    .file(&wiki.abs_path(&link.target_path))
                    .expect("resolved target remains readable");
                let anchor = document
                    .resolve_heading(heading, LinkStyle::Obsidian)
                    .and_then(|heading| document.markdown_anchor(heading))
                    .unwrap_or_else(|| crate::page::github_heading_anchor(heading));
                destination.push_str(&anchor);
            }
            LinkFragment::Block(block) => {
                destination.push('^');
                destination.push_str(block.as_str());
            }
        }
    }
    destination
}

fn markdown_page_destination(file_path: &Path, wiki: &Wiki, target_path: &Path) -> String {
    let source_parent = wiki
        .rel_path(file_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let path = markdown_links::relative_path(source_parent, target_path)
        .to_string_lossy()
        .replace('\\', "/");
    markdown_links::encode_url_path(&path)
}

fn logical_label(display_name: &str, fragment: Option<&LinkFragment>) -> String {
    let mut label = display_name.to_owned();
    if let Some(fragment) = fragment {
        label.push('#');
        match fragment {
            LinkFragment::Heading(heading) => label.push_str(heading),
            LinkFragment::Block(block) => {
                label.push('^');
                label.push_str(block.as_str());
            }
        }
    }
    label
}

fn unique_reference_label(base: String, reserved: &[String]) -> String {
    if !reserved
        .iter()
        .any(|label| reference_labels_match(label, &base))
    {
        return base;
    }
    for suffix in 2.. {
        let candidate = format!("{base}-{suffix}");
        if !reserved
            .iter()
            .any(|label| reference_labels_match(label, &candidate))
        {
            return candidate;
        }
    }
    unreachable!()
}

fn reference_labels_match(left: &str, right: &str) -> bool {
    // ponytail: pairwise parser lookups are enough for document-sized label sets;
    // retain normalized keys if repositories reach thousands of definitions.
    let source = format!("[{}]: /target\n", escape_reference_label(left));
    Parser::new(&source)
        .reference_definitions()
        .get(right)
        .is_some()
}

fn escape_reference_label(label: &str) -> String {
    label
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn reference_use_labels(source: &str) -> Vec<String> {
    Parser::new_ext(source, Options::ENABLE_WIKILINKS)
        .filter_map(|event| match event {
            Event::Start(Tag::Link { link_type, id, .. })
            | Event::Start(Tag::Image { link_type, id, .. })
                if matches!(
                    link_type,
                    LinkType::Reference | LinkType::Collapsed | LinkType::Shortcut
                ) =>
            {
                Some(id.to_string())
            }
            _ => None,
        })
        .collect()
}

fn definition_line_range(source: &str, definition: &MarkdownReferenceDefinition) -> Range<usize> {
    let mut end = definition.byte_range.end;
    if source[end..].starts_with("\r\n") {
        end += 2;
    } else if source[end..].starts_with('\n') {
        end += 1;
    }
    definition.byte_range.start..end
}
