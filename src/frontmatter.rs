use std::ops::Range;

use crate::error::FrontmatterError;

/// Parsed YAML frontmatter from a markdown file. Schema-free.
#[derive(Debug, Clone)]
pub struct Frontmatter {
    byte_range: Range<usize>,
    data: serde_yml::Value,
}

impl Frontmatter {
    /// Get a frontmatter field by name.
    pub fn get(&self, field: &str) -> Option<&serde_yml::Value> {
        self.data.get(field)
    }

    /// Check if a field exists in the frontmatter.
    pub fn has_field(&self, field: &str) -> bool {
        self.get(field).is_some()
    }

    /// Get a field as a string list (handles both YAML sequences and single strings).
    pub fn get_str_list(&self, field: &str) -> Vec<&str> {
        match self.get(field) {
            Some(serde_yml::Value::Sequence(seq)) => {
                seq.iter().filter_map(|v| v.as_str()).collect()
            }
            Some(serde_yml::Value::String(s)) => vec![s.as_str()],
            _ => Vec::new(),
        }
    }

    /// Get the full parsed data.
    pub fn data(&self) -> &serde_yml::Value {
        &self.data
    }
}

/// Return the edit that sets a frontmatter field, creating the block if needed.
pub(crate) fn set_field_edit(
    frontmatter: Option<&Frontmatter>,
    field: &str,
    value: &str,
) -> Result<(Range<usize>, String), FrontmatterError> {
    let parsed_value =
        serde_yml::from_str(value).unwrap_or(serde_yml::Value::String(value.to_owned()));
    let (range, yaml_value) = match frontmatter {
        Some(fm) => {
            let mut yaml_value = fm.data.clone();
            let serde_yml::Value::Mapping(ref mut map) = yaml_value else {
                return Err(FrontmatterError::NotMapping);
            };
            map.insert(field.to_owned(), parsed_value);
            (fm.byte_range.clone(), yaml_value)
        }
        None => {
            let mut map = serde_yml::Mapping::new();
            map.insert(field.to_owned(), parsed_value);
            (0..0, serde_yml::Value::Mapping(map))
        }
    };

    let mut new_yaml = serde_yml::to_string(&yaml_value).map_err(|e| FrontmatterError::Yaml {
        source: Box::new(e),
        context: field.to_owned(),
    })?;
    if !new_yaml.ends_with('\n') {
        new_yaml.push('\n');
    }
    Ok((
        range,
        match frontmatter {
            Some(_) => format!("---\n{new_yaml}---\n"),
            None => format!("---\n{new_yaml}---\n\n"),
        },
    ))
}

/// Split frontmatter from markdown source. Returns `(yaml_str, yaml_byte_range)` if present.
fn split_frontmatter(source: &str) -> Option<(&str, Range<usize>)> {
    let trimmed = source.strip_prefix("---")?;
    if !trimmed.starts_with('\n') && !trimmed.starts_with("\r\n") {
        return None;
    }
    let after_opener = source.len() - trimmed.len();
    let closing = trimmed.find("\n---")?;
    let yaml_start = after_opener;
    let yaml_end = yaml_start + closing;
    let block_end_offset = closing + "\n---".len();
    let rest = &trimmed[block_end_offset..];
    let block_end = yaml_start
        + block_end_offset
        + if rest.starts_with('\n') {
            1
        } else if rest.starts_with("\r\n") {
            2
        } else {
            0
        };
    Some((&source[yaml_start..yaml_end], 0..block_end))
}

/// Parse frontmatter from a markdown source string.
pub fn parse_frontmatter(source: &str) -> Result<Option<Frontmatter>, FrontmatterError> {
    let Some((yaml_str, byte_range)) = split_frontmatter(source) else {
        return Ok(None);
    };
    let data: serde_yml::Value =
        serde_yml::from_str(yaml_str).map_err(|e| FrontmatterError::Yaml {
            source: Box::new(e),
            context: yaml_str.chars().take(80).collect(),
        })?;
    Ok(Some(Frontmatter { byte_range, data }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::splice;

    #[test]
    fn parses_standard_frontmatter() {
        let source = "---\ntitle: Test Page\ntags: [a, b]\ndate: 2026-01-01\nsources: [raw/papers/test.md]\n---\n\n# Content";
        let fm = parse_frontmatter(source).unwrap().unwrap();
        assert_eq!(fm.get("title").and_then(|v| v.as_str()), Some("Test Page"));
        assert_eq!(fm.get_str_list("tags"), vec!["a", "b"]);
        assert_eq!(fm.get("date").and_then(|v| v.as_str()), Some("2026-01-01"));
        assert_eq!(fm.get_str_list("sources"), vec!["raw/papers/test.md"]);
        assert_eq!(fm.byte_range.start, 0);
        assert!(source[fm.byte_range].ends_with('\n'));
    }

    #[test]
    fn returns_none_without_frontmatter() {
        let source = "# Just a heading\n\nSome content.";
        assert!(parse_frontmatter(source).unwrap().is_none());
    }

    #[test]
    fn handles_empty_optional_fields() {
        let source = "---\ntitle: Minimal\n---\n\nContent";
        let fm = parse_frontmatter(source).unwrap().unwrap();
        assert_eq!(fm.get("title").and_then(|v| v.as_str()), Some("Minimal"));
        assert!(!fm.has_field("tags"));
        assert!(!fm.has_field("date"));
    }

    #[test]
    fn schema_free_arbitrary_fields() {
        let source = "---\ncustom_field: hello\nnested:\n  key: value\n---\n\nContent";
        let fm = parse_frontmatter(source).unwrap().unwrap();
        assert_eq!(
            fm.get("custom_field").and_then(|v| v.as_str()),
            Some("hello")
        );
        assert!(fm.has_field("nested"));
    }

    #[test]
    fn autolink_field_check() {
        let source = "---\ntitle: Test\nautolink: false\n---\n\nContent";
        let fm = parse_frontmatter(source).unwrap().unwrap();
        assert_eq!(fm.get("autolink"), Some(&serde_yml::Value::Bool(false)));
    }

    #[test]
    fn set_field_edit_updates_mapping_frontmatter() {
        let source = "---\ntitle: Test\n---\n\nContent";
        let fm = parse_frontmatter(source).unwrap().unwrap();
        let edit = set_field_edit(Some(&fm), "published", "true").unwrap();

        assert_eq!(
            splice::apply(source, &[edit]),
            "---\ntitle: Test\npublished: true\n---\n\nContent"
        );
    }

    #[test]
    fn set_field_edit_creates_frontmatter() {
        let source = "# Content\n";
        let edit = set_field_edit(None, "owner", "alice").unwrap();

        assert_eq!(
            splice::apply(source, &[edit]),
            "---\nowner: alice\n---\n\n# Content\n"
        );
    }

    #[test]
    fn set_field_edit_rejects_non_mapping_frontmatter() {
        let source = "---\n- item\n---\n\nContent";
        let fm = parse_frontmatter(source).unwrap().unwrap();

        assert!(matches!(
            set_field_edit(Some(&fm), "owner", "alice"),
            Err(FrontmatterError::NotMapping)
        ));
    }
}
