---
title: First-class Obsidian and Markdown link styles
labels: ["wayfinder:map"]
status: closed
---

## Destination

Implemented and verified equal parsing, resolution, linting, graphing, rename, and move support for Obsidian and Markdown internal links, plus explicit repository-configured formatting into either style and optional Reference-style Markdown links.

## Notes

- The user expanded this map to carry implementation and logical-slice commits through final review.
- Work parser-first. Do not specify formatting or command integration against separate syntax-specific models.
- Consult `wayfinder`, `domain-modeling`, `analysis-design-guidelines`, `rust-code-guidelines`, and `ponytail` while resolving tickets. Consult `prompt-writing-guidelines` for agent/setup guidance.
- The local-Markdown tracker uses issue frontmatter for status, assignee, parent, labels, and fallback `blocked_by` relationships. An open issue with no blockers and no assignee is on the frontier.
- Both link styles are always accepted in the same repository. The configured style controls generation and explicit `links format`, not parsing.
- Markdown internal links are path-addressed; Obsidian internal links are page-name/alias-addressed. Page-name and alias uniqueness remains repository-wide and case-insensitive.
- Existing block links remain supported in both styles, but generated agent guidance prefers heading links. Embeds are parsed but never style-converted.
- Unconfigured repositories continue generating Obsidian links for backward compatibility.

## Decisions so far

- [Confirm GitHub heading-anchor and Reference-style semantics](issues/02-confirm-github-heading-and-reference-semantics.md) — Use exact CommonMark reference semantics and a fixture-backed local GitHub-compatible heading profile because GitHub's exact anchor algorithm is unpublished.
- [Define the unified internal-link model](issues/01-define-unified-internal-link-model.md) — Parse both styles into one occurrence carrying syntax, target, fragment, display text, edit ranges, reference label, and embed state.
- [Define path and fragment resolution across link styles](issues/03-define-path-and-fragment-resolution.md) — Resolve Markdown links by decoded relative path and GitHub-compatible anchors while retaining name/alias resolution for Obsidian links.
- [Define link-formatting and reference-definition invariants](issues/04-define-formatting-invariants.md) — Format resolvable navigational links idempotently and preserve definitions still used by images or unconverted references.
- [Define the repository link-style configuration contract](issues/05-define-repository-link-style-configuration.md) — Use `link_style` plus an optional positive Markdown-only `reference_style_threshold`, defaulting to Obsidian.
- [Define the links format command contract](issues/06-define-links-format-command.md) — Format every non-verbatim Markdown file through `EditPlan`, dry-run by default, and warn while preserving unresolved links.
- [Define integration across link-aware commands](issues/07-define-link-aware-command-integration.md) — Route graph, lint, refs, inventory, autolinking, and heading rename through the unified semantics while reusing existing rename/move destination edits.
- [Decide the library API compatibility boundary](issues/08-decide-library-api-compatibility.md) — Keep the new model internal and preserve the existing syntax-specific document views used inside the crate.
- [Define repository compatibility and migration behavior](issues/09-define-compatibility-and-migration.md) — Parse mixed styles without churn and require explicit `links format` for repository-wide conversion.
- [Define agent and setup guidance for link styles](issues/10-define-agent-and-setup-guidance.md) — Document both styles, explicit formatting, the threshold, stable page-name uniqueness, and heading-over-block guidance.
- [Sequence implementation and verification](issues/11-sequence-implementation-and-verification.md) — Land parser-first logical slices, close review-discovered parity gaps, and verify the complete implementation with tests, clippy, formatting, release build, and two-axis review.

## Not yet specified

None.

## Out of scope

- Configurable Markdown renderer or heading-anchor profiles beyond the GitHub-compatible baseline.
- Converting Obsidian embeds into Markdown images or another transclusion syntax.
- Formatting external links, images, or unresolved internal links.
- Automatically normalizing touched documents during rename, move, or other unrelated modifying commands.
- Inventing portable HTML anchors for Obsidian block IDs.
- Document-local formatting escape hatches without a demonstrated repository need.
- Automatically inferring preferred style or Reference-style thresholds during setup.
