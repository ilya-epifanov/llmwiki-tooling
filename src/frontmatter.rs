use crate::error::FrontmatterError;

/// Parsed YAML frontmatter from a markdown file. Schema-free.
#[derive(Debug, Clone)]
pub struct Frontmatter {
    /// Raw YAML text between the `---` delimiters (excluding delimiters).
    pub raw_yaml: String,
    /// Byte range of the entire frontmatter block including `---` delimiters.
    pub byte_range: std::ops::Range<usize>,
    /// Parsed YAML value for arbitrary field access.
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

    /// Get a field as a string, if it is one.
    pub fn get_str(&self, field: &str) -> Option<&str> {
        self.get(field).and_then(|v| v.as_str())
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

/// Split frontmatter from markdown source. Returns `(yaml_str, yaml_byte_range)` if present.
fn split_frontmatter(source: &str) -> Option<(&str, std::ops::Range<usize>)> {
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
            source: e,
            context: yaml_str.chars().take(80).collect(),
        })?;
    Ok(Some(Frontmatter {
        raw_yaml: yaml_str.to_owned(),
        byte_range,
        data,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_frontmatter() {
        let source = "---\ntitle: Test Page\ntags: [a, b]\ndate: 2026-01-01\nsources: [raw/papers/test.md]\n---\n\n# Content";
        let fm = parse_frontmatter(source).unwrap().unwrap();
        assert_eq!(fm.get_str("title"), Some("Test Page"));
        assert_eq!(fm.get_str_list("tags"), vec!["a", "b"]);
        assert_eq!(fm.get_str("date"), Some("2026-01-01"));
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
        assert_eq!(fm.get_str("title"), Some("Minimal"));
        assert!(!fm.has_field("tags"));
        assert!(!fm.has_field("date"));
    }

    #[test]
    fn schema_free_arbitrary_fields() {
        let source = "---\ncustom_field: hello\nnested:\n  key: value\n---\n\nContent";
        let fm = parse_frontmatter(source).unwrap().unwrap();
        assert_eq!(fm.get_str("custom_field"), Some("hello"));
        assert!(fm.has_field("nested"));
    }

    #[test]
    fn autolink_field_check() {
        let source = "---\ntitle: Test\nautolink: false\n---\n\nContent";
        let fm = parse_frontmatter(source).unwrap().unwrap();
        assert_eq!(fm.get("autolink"), Some(&serde_yml::Value::Bool(false)));
    }
}
