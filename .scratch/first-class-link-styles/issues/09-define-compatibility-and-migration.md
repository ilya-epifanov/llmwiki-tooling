---
title: Define repository compatibility and migration behavior
labels: ["wayfinder:grilling"]
status: closed
assignee: null
parent: ../map.md
blocked_by:
  - 04-define-formatting-invariants.md
  - 05-define-repository-link-style-configuration.md
  - 07-define-link-aware-command-integration.md
  - 08-decide-library-api-compatibility.md
---

## Question

What compatibility guarantees, diagnostics, and opt-in migration path let existing Obsidian repositories upgrade without churn while mixed-style and Markdown-preferred repositories gain first-class checking and explicit formatting?

## Resolution comment

Both styles are always parsed, including mixed documents. Missing configuration retains Obsidian generation. Rename, move, and other unrelated commands preserve existing syntax; repository-wide conversion happens only through explicit `links format`, which is dry-run by default.
