---
title: Define the links format command contract
labels: ["wayfinder:grilling"]
status: closed
assignee: null
parent: ../map.md
blocked_by:
  - 04-define-formatting-invariants.md
  - 05-define-repository-link-style-configuration.md
---

## Question

What should `links format` show in dry-run and write modes across managed and unmanaged non-verbatim Markdown, and how should it report unresolved links, unconverted embeds, block-link portability, and invalid or missing configuration without adding a CLI style override?

## Resolution comment

`links format` uses the repository configuration and existing `EditPlan`, shows diffs by default, and writes only with `--write`. It scans managed and unmanaged non-verbatim Markdown, leaves embeds and images untouched, preserves unresolved links with per-file warnings, and has no CLI style override.
