---
title: Decide the library API compatibility boundary
labels: ["wayfinder:grilling"]
status: closed
assignee: null
parent: ../map.md
blocked_by:
  - 01-define-unified-internal-link-model.md
  - 07-define-link-aware-command-integration.md
---

## Question

Which current Rust types and methods are externally observable, and should the unified internal-link model replace, wrap, or remain behind the existing wikilink- and Markdown-destination-specific APIs for the next compatible release?

## Resolution comment

The relevant modules are crate-private, so no external Rust API migration is required. The unified view is additive inside `MarkdownDocument`; existing `wikilinks` and `markdown_links` views remain for syntax-specific rewrite operations.
