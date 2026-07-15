use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use crate::markdown_document::MarkdownLinkDestination;

type Edits = Vec<(Range<usize>, String)>;

pub(crate) fn rebase_relative_links(
    links: &[MarkdownLinkDestination],
    old_path: &Path,
    new_path: &Path,
) -> Edits {
    let old_parent = old_path.parent().unwrap_or_else(|| Path::new(""));
    let new_parent = new_path.parent().unwrap_or_else(|| Path::new(""));
    markdown_link_edits(links, |dest| {
        let (path_part, suffix) = split_suffix(dest)?;
        if !is_relative_md_path(path_part) {
            return None;
        }
        let path_part = decode_url_path(path_part)?;
        let target = normalize_path(old_parent.join(path_part));
        let target = if target == old_path {
            new_path.to_path_buf()
        } else {
            target
        };
        Some(markdown_url(&relative_path(new_parent, &target), suffix))
    })
}

pub(crate) fn retarget_relative_links(
    links: &[MarkdownLinkDestination],
    source_path: &Path,
    old_path: &Path,
    new_path: &Path,
) -> Edits {
    let source_parent = source_path.parent().unwrap_or_else(|| Path::new(""));
    markdown_link_edits(links, |dest| {
        let (path_part, suffix) = split_suffix(dest)?;
        if !is_relative_md_path(path_part) {
            return None;
        }
        let path_part = decode_url_path(path_part)?;
        let target = normalize_path(source_parent.join(path_part));
        (target == old_path).then(|| markdown_url(&relative_path(source_parent, new_path), suffix))
    })
}

fn markdown_link_edits<F>(links: &[MarkdownLinkDestination], mut replacement: F) -> Edits
where
    F: FnMut(&str) -> Option<String>,
{
    let mut edits = Vec::new();
    for link in links {
        let Some(new_dest) = replacement(&link.destination) else {
            continue;
        };
        if new_dest != link.destination {
            edits.push((link.byte_range.clone(), new_dest));
        }
    }
    edits
}

fn split_suffix(dest: &str) -> Option<(&str, &str)> {
    let (path, suffix) = dest.split_once('#').unwrap_or((dest, ""));
    (!path.is_empty()).then_some((path, suffix))
}

pub(crate) fn is_relative_md_path(path: &str) -> bool {
    let Some(decoded) = decode_url_path(path) else {
        return false;
    };
    !decoded.starts_with(['/', '\\'])
        && !decoded
            .split(['/', '\\'])
            .next()
            .is_some_and(|segment| segment.contains(':'))
        && Path::new(&decoded)
            .extension()
            .is_some_and(|ext| ext == "md")
}

fn markdown_url(path: &Path, suffix: &str) -> String {
    let mut out = encode_url_path(&path.to_string_lossy().replace('\\', "/"));
    if !suffix.is_empty() {
        out.push('#');
        out.push_str(suffix);
    }
    out
}

pub(crate) fn encode_url_path(path: &str) -> String {
    path.chars()
        .map(|character| match character {
            ' ' => "%20".to_owned(),
            '%' => "%25".to_owned(),
            '#' => "%23".to_owned(),
            '?' => "%3F".to_owned(),
            '<' => "%3C".to_owned(),
            '>' => "%3E".to_owned(),
            '"' => "%22".to_owned(),
            _ => character.to_string(),
        })
        .collect()
}

pub(crate) fn normalize_path(path: PathBuf) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub(crate) fn decode_url_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let value = u8::from_str_radix(
                std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?,
                16,
            )
            .ok()?;
            decoded.push(value);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

pub(crate) fn relative_path(from_dir: &Path, target: &Path) -> PathBuf {
    let from = normalize_path(from_dir.to_path_buf());
    let target = normalize_path(target.to_path_buf());
    let from_components: Vec<_> = from.components().collect();
    let target_components: Vec<_> = target.components().collect();
    let common = from_components
        .iter()
        .zip(&target_components)
        .take_while(|(a, b)| a == b)
        .count();

    let mut out = PathBuf::new();
    for component in &from_components[common..] {
        if matches!(component, Component::Normal(_)) {
            out.push("..");
        }
    }
    for component in &target_components[common..] {
        if let Component::Normal(part) = component {
            out.push(part);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown_document::MarkdownDocument;
    use crate::splice;

    fn document(source: &str) -> MarkdownDocument {
        MarkdownDocument::new(source.to_owned())
    }

    #[test]
    fn retargets_inline_destination_without_touching_title() {
        let source = "See [topic](topics/old.md \"topics/old.md\").\n";
        let document = document(source);
        let edits = retarget_relative_links(
            document.markdown_links(),
            Path::new("/repo/overview.md"),
            Path::new("/repo/topics/old.md"),
            Path::new("/repo/topics/new.md"),
        );

        assert_eq!(
            splice::apply(source, &edits),
            "See [topic](topics/new.md \"topics/old.md\").\n"
        );
    }

    #[test]
    fn retargets_reference_definition_destination() {
        let source = "See [topic][topic].\n\n[topic]: topics/old.md \"title\"\n";
        let document = document(source);
        let edits = retarget_relative_links(
            document.markdown_links(),
            Path::new("/repo/overview.md"),
            Path::new("/repo/topics/old.md"),
            Path::new("/repo/topics/new.md"),
        );

        assert_eq!(
            splice::apply(source, &edits),
            "See [topic][topic].\n\n[topic]: topics/new.md \"title\"\n"
        );
    }

    #[test]
    fn retargets_multiline_reference_definition_destination() {
        let source = "See [topic].\n\n[topic]:\n  topics/Old%20Name.md \"title\"\n";
        let document = document(source);
        let edits = retarget_relative_links(
            document.markdown_links(),
            Path::new("/repo/overview.md"),
            Path::new("/repo/topics/Old Name.md"),
            Path::new("/repo/topics/New Name.md"),
        );

        assert_eq!(
            splice::apply(source, &edits),
            "See [topic].\n\n[topic]:\n  topics/New%20Name.md \"title\"\n"
        );
    }

    #[test]
    fn retargets_balanced_inline_destination() {
        let source = "See [topic](topics/Old(Name).md).\n";
        let document = document(source);
        let edits = retarget_relative_links(
            document.markdown_links(),
            Path::new("/repo/overview.md"),
            Path::new("/repo/topics/Old(Name).md"),
            Path::new("/repo/topics/New(Name).md"),
        );

        assert_eq!(
            splice::apply(source, &edits),
            "See [topic](topics/New(Name).md).\n"
        );
    }

    #[test]
    fn retargets_definition_after_escaped_label_delimiter() {
        let source = "[foo\\]:bar]: topics/Old%20Name.md\n";
        let document = document(source);
        let edits = retarget_relative_links(
            document.markdown_links(),
            Path::new("/repo/overview.md"),
            Path::new("/repo/topics/Old Name.md"),
            Path::new("/repo/topics/New Name.md"),
        );

        assert_eq!(
            splice::apply(source, &edits),
            "[foo\\]:bar]: topics/New%20Name.md\n"
        );
    }
}
