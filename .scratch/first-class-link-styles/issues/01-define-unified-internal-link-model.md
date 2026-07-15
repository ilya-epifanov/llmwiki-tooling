---
title: Define the unified internal-link model
labels: ["wayfinder:grilling"]
status: closed
assignee: null
parent: ../map.md
blocked_by: []
---

## Question

What single parsed representation should preserve the source range, visible text, syntax style, target page, optional heading or block fragment, reference label, and embed distinction needed by every link-aware command, and exactly which Obsidian and CommonMark inline/full/collapsed/shortcut forms must produce it?

## Resolution comment

`InternalLinkOccurrence` records style, unresolved page name or path, fragment, raw display text, occurrence and destination ranges, optional reference label, and embed state. `MarkdownDocument::internal_links` parses Obsidian links plus CommonMark inline, full, collapsed, and shortcut links while retaining existing syntax-specific views.
