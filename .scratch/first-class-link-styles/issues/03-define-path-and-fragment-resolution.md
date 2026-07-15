---
title: Define path and fragment resolution across link styles
labels: ["wayfinder:grilling"]
status: closed
assignee: null
parent: ../map.md
blocked_by:
  - 01-define-unified-internal-link-model.md
  - 02-confirm-github-heading-and-reference-semantics.md
---

## Question

How should Markdown paths, Obsidian page names and aliases, heading fragments, block fragments, URL encoding, and ambiguous or unresolved targets map into the unified internal-link identity without changing repository-wide page-name uniqueness?

## Resolution comment

Obsidian targets retain case-insensitive page-name/alias resolution. Markdown targets decode percent escapes and resolve relative to the source document's actual `.md` path. Heading fragments use the fixture-backed GitHub-compatible profile; `#^block-id` remains supported in either syntax. Missing or ambiguous targets stay unresolved and preserve repository-wide page-name/alias uniqueness.
