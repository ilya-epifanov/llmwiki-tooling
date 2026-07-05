use std::fmt;
use std::ops::Range;
use std::path::Path;

/// Filename stem identifying a wiki page, normalized to lowercase for O(1) lookups.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageId(String);

impl PageId {
    pub fn from_path(path: &Path) -> Option<Self> {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| Self(s.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for PageId {
    fn from(s: &str) -> Self {
        Self(s.to_ascii_lowercase())
    }
}

/// A block identifier without the `^` prefix, e.g. `"method-comparison"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockId(String);

impl BlockId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BlockId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// Fragment part of a wikilink after `#`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WikilinkFragment {
    /// `#heading-text`
    Heading(String),
    /// `#^block-id`
    Block(BlockId),
}

/// A parsed wikilink occurrence with its source location.
#[derive(Debug, Clone)]
pub struct WikilinkOccurrence {
    pub page: PageId,
    pub fragment: Option<WikilinkFragment>,
    pub byte_range: Range<usize>,
}

/// A parsed heading from a markdown file.
#[derive(Debug, Clone)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub byte_range: Range<usize>,
}
