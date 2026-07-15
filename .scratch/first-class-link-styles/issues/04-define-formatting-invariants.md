---
title: Define link-formatting and reference-definition invariants
labels: ["wayfinder:grilling"]
status: closed
assignee: null
parent: ../map.md
blocked_by:
  - 01-define-unified-internal-link-model.md
  - 03-define-path-and-fragment-resolution.md
---

## Question

What lossless, idempotent rules convert resolvable navigational links between styles, choose shortcut versus full Reference-style syntax, apply the per-source-document target threshold, place or reuse definitions, resolve normalized-label collisions, and remove definitions that become unused while preserving unrelated Markdown?

## Resolution comment

Formatting canonicalizes only resolved navigational links, counts occurrences per target page, emits shortcut references only when display text equals the logical label, and otherwise emits full references. Definitions use page name plus fragment, CommonMark collision matching, and an EOF section after a blank line. A definition is removed only when every use is converted; image and unconverted uses preserve it. A second run produces no edits.
