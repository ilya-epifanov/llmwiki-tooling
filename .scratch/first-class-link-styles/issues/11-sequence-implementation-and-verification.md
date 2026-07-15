---
title: Sequence implementation and verification
labels: ["wayfinder:grilling"]
status: closed
assignee: null
parent: ../map.md
blocked_by:
  - 06-define-links-format-command.md
  - 09-define-compatibility-and-migration.md
  - 10-define-agent-and-setup-guidance.md
---

## Question

What regression-first vertical slices and checks implement the settled specification parser-first, then integrate existing commands, configuration, explicit formatting, and agent guidance without a flag day or unrelated refactor?

## Resolution comment

Implementation landed as parser/graph support (`3110604`), configured formatting (`4605f46`), user and agent documentation (`96a71d9`, `d250ce0`), unified-semantics corrections (`cdbc6d0`), and CommonMark edge-case corrections (`42cd799`). Regression coverage now exercises mixed styles, inline and Reference-style links, URL encoding, fragments, rename/move, dry-run formatting, idempotence, and lossless duplicate-heading behavior. Final verification passed 68 unit tests, 19 CLI tests, doc tests, formatting, clippy with warnings denied, a release build, and both standards/spec review axes; the final spec review reported no unresolved findings.
