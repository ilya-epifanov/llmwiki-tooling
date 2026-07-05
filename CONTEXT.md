# Wiki CLI Tool

This context names the domain concepts used by llmwiki-tooling, a CLI for maintaining markdown LLM-wikis with Obsidian-style wikilinks.

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
A markdown file's source plus its parsed wiki-relevant structure: frontmatter, headings, wikilinks, block IDs, and prose/non-prose ranges. It is the document being interpreted, not a command output.
_Avoid_: markdown facts, parse result

**Markdown file set**:
The scannable markdown files under the wiki root after ignore and verbatim rules are applied, each paired with its relative path and Markdown document. It is the shared input for Wiki inventory and link-aware tooling.
_Avoid_: file list, walk result
