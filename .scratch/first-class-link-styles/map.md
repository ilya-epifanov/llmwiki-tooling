---
title: First-class Obsidian and Markdown link styles
labels: ["wayfinder:map"]
status: open
---

## Destination

An implementation-ready specification for equal parsing, resolution, linting, graphing, rename, and move support for Obsidian and Markdown internal links, plus explicit repository-configured formatting into either style and optional Reference-style Markdown links.

## Notes

- This map plans the change; implementation is out of scope.
- Work parser-first. Do not specify formatting or command integration against separate syntax-specific models.
- Consult `wayfinder`, `domain-modeling`, `analysis-design-guidelines`, `rust-code-guidelines`, and `ponytail` while resolving tickets. Consult `prompt-writing-guidelines` for agent/setup guidance.
- The local-Markdown tracker uses issue frontmatter for status, assignee, parent, labels, and fallback `blocked_by` relationships. An open issue with no blockers and no assignee is on the frontier.
- Both link styles are always accepted in the same repository. The configured style controls generation and explicit `links format`, not parsing.
- Markdown internal links are path-addressed; Obsidian internal links are page-name/alias-addressed. Page-name and alias uniqueness remains repository-wide and case-insensitive.
- Existing block links remain supported in both styles, but generated agent guidance prefers heading links. Embeds are parsed but never style-converted.
- Unconfigured repositories continue generating Obsidian links for backward compatibility.

## Decisions so far

- [Confirm GitHub heading-anchor and Reference-style semantics](issues/02-confirm-github-heading-and-reference-semantics.md) — Use exact CommonMark reference semantics and a fixture-backed local GitHub-compatible heading profile because GitHub's exact anchor algorithm is unpublished.

## Not yet specified

- Whether parser edge cases expose a need for document-local formatting escape hatches.
- Whether setup can safely infer a preferred style or Reference-style threshold from an existing repository after both styles enter the inventory.

## Out of scope

- Implementing the specification produced by this map.
- Configurable Markdown renderer or heading-anchor profiles beyond the GitHub-compatible baseline.
- Converting Obsidian embeds into Markdown images or another transclusion syntax.
- Formatting external links, images, or unresolved internal links.
- Automatically normalizing touched documents during rename, move, or other unrelated modifying commands.
- Inventing portable HTML anchors for Obsidian block IDs.
