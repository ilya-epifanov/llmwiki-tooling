# Wiki CLI Tool

This context names the domain concepts used by llmwiki-tooling, a CLI for maintaining Markdown LLM-wikis and their internal links.

## Language

**Wiki inventory**:
A summary of the wiki's markdown shape: exact containing directories, file counts, frontmatter field coverage, heading coverage, mirror candidates, and index references. It includes unmanaged markdown and excludes ignored/verbatim markdown; it is data about the wiki structure, not the act of scanning it.
_Avoid_: structure scan, scan result

**Unmanaged page**:
A markdown page outside configured managed directories that is still visible to link-aware tooling. It appears in Wiki inventory unless ignored or verbatim.
_Avoid_: loose note

**Verbatim page**:
A markdown page intentionally invisible to link scans, rewrites, lint checks, and Wiki inventory.
_Avoid_: raw page, skipped page

**Markdown document**:
A Markdown file's source plus its parsed wiki-relevant structure: frontmatter, headings, internal links, block IDs, and prose/non-prose ranges. It is the document being interpreted, not a command output.
_Avoid_: markdown facts, parse result

**Markdown file set**:
The scannable markdown files under the wiki root after ignore and verbatim rules are applied, each paired with its relative path and Markdown document. It is the shared input for Wiki inventory and link-aware tooling.
_Avoid_: file list, walk result

**Internal link**:
A navigational link from one Markdown document to a page, heading, or block in the same wiki. Its target identity is independent of whether its source uses Obsidian or Markdown syntax.
_Avoid_: wikilink when referring to both styles

**Link style**:
The source syntax of an internal link: Obsidian style (`[[Page]]`) or Markdown style (`[Page](path/Page.md)`). It changes representation, not target identity.
_Avoid_: link format, link type

**Page name**:
The repository-wide, case-insensitive name derived from a page's filename stem. Page names and Obsidian aliases share one uniqueness namespace even when the repository emits Markdown-style links.
_Avoid_: path, reference label

**Preferred link style**:
The repository-configured style used when generating internal links and by explicit link formatting. It does not implicitly rewrite existing links during unrelated commands.
_Avoid_: forced format

**Reference-style link**:
A Markdown link whose destination is stored in a link reference definition, normally at the bottom of the document. Generated reference labels use the logical page name plus any heading or block fragment.
_Avoid_: deferred link, footer link
