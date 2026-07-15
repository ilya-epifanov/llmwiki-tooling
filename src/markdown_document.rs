use std::ops::Range;

use once_cell::unsync::OnceCell;

use crate::error::FrontmatterError;
use crate::frontmatter::{self, Frontmatter};
use crate::page::{BlockId, Heading, InternalLinkOccurrence, WikilinkOccurrence};

mod parse;

/// Classification of a byte range within a markdown source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeKind {
    /// Regular text where bare mentions should be detected.
    Prose,
    /// Heading content — bare mentions should not be linked here.
    Heading,
    /// YAML frontmatter block.
    Frontmatter,
    /// Fenced or indented code block.
    CodeBlock,
    /// Inline code span.
    InlineCode,
    /// An existing wikilink `[[...]]`.
    Wikilink,
    /// An embed `![[...]]`.
    Embed,
    /// An autolink or URL.
    Url,
    /// Raw HTML block.
    HtmlBlock,
    /// HTML inline tag.
    HtmlInline,
}

/// A byte range within the source classified by its structural role.
#[derive(Debug, Clone)]
pub struct ClassifiedRange {
    pub kind: RangeKind,
    pub byte_range: Range<usize>,
}

/// A standard Markdown link destination with its source byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownLinkDestination {
    pub destination: String,
    pub byte_range: Range<usize>,
}

/// Markdown source plus cached wiki-relevant parsed structure.
pub struct MarkdownDocument {
    source: String,
    frontmatter: OnceCell<Option<Frontmatter>>,
    headings: OnceCell<Vec<Heading>>,
    wikilinks: OnceCell<Vec<WikilinkOccurrence>>,
    internal_links: OnceCell<Vec<InternalLinkOccurrence>>,
    markdown_links: OnceCell<Vec<MarkdownLinkDestination>>,
    classified_ranges: OnceCell<Vec<ClassifiedRange>>,
    block_ids: OnceCell<Vec<BlockId>>,
}

impl MarkdownDocument {
    pub(crate) fn new(source: String) -> Self {
        Self {
            source,
            frontmatter: OnceCell::new(),
            headings: OnceCell::new(),
            wikilinks: OnceCell::new(),
            internal_links: OnceCell::new(),
            markdown_links: OnceCell::new(),
            classified_ranges: OnceCell::new(),
            block_ids: OnceCell::new(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn frontmatter(&self) -> Result<Option<&Frontmatter>, FrontmatterError> {
        self.frontmatter
            .get_or_try_init(|| frontmatter::parse_frontmatter(&self.source))
            .map(Option::as_ref)
    }

    pub(crate) fn set_frontmatter_field_edit(
        &self,
        field: &str,
        value: &str,
    ) -> Result<(Range<usize>, String), FrontmatterError> {
        frontmatter::set_field_edit(self.frontmatter()?, field, value)
    }

    pub fn headings(&self) -> &[Heading] {
        self.headings
            .get_or_init(|| parse::extract_headings(&self.source))
    }

    pub fn wikilinks(&self) -> &[WikilinkOccurrence] {
        self.wikilinks
            .get_or_init(|| parse::extract_wikilinks(&self.source))
    }

    pub fn internal_links(&self) -> &[InternalLinkOccurrence] {
        self.internal_links
            .get_or_init(|| parse::extract_internal_links(&self.source))
    }

    pub fn markdown_links(&self) -> &[MarkdownLinkDestination] {
        self.markdown_links
            .get_or_init(|| parse::extract_markdown_links(&self.source))
    }

    pub fn classified_ranges(&self) -> &[ClassifiedRange] {
        self.classified_ranges
            .get_or_init(|| parse::classify_ranges(&self.source))
    }

    pub fn block_ids(&self) -> &[BlockId] {
        self.block_ids
            .get_or_init(|| parse::extract_block_ids(&self.source))
    }
}
