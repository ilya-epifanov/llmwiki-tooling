---
title: Define the repository link-style configuration contract
labels: ["wayfinder:grilling"]
status: closed
assignee: null
parent: ../map.md
blocked_by:
  - 01-define-unified-internal-link-model.md
---

## Question

What minimal `wiki.toml` fields and validation rules express an Obsidian or Markdown preferred link style plus an optional positive Reference-style threshold, while preserving current Obsidian generation when configuration is absent?

## Resolution comment

`[linking].link_style` accepts `obsidian` or `markdown` and defaults to `obsidian`. Optional `reference_style_threshold` is a positive integer accepted only with Markdown style; omission keeps Markdown links inline.
