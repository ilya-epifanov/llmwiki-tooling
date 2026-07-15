---
title: Confirm GitHub heading-anchor and Reference-style semantics
labels: ["wayfinder:research"]
status: closed
assignee: research-github-semantics
parent: ../map.md
blocked_by: []
---

## Question

What exact primary-source rules govern GitHub-compatible heading fragments and CommonMark Reference-style labels and definitions, including punctuation, Unicode, duplicate headings, escaping, normalization, and fragment URL encoding?

## Resolution comment

[Research findings](../research/github-heading-and-reference-semantics.md): follow exact CommonMark rules for Reference-style parsing, matching, and definitions. Define and fixture-test a local GitHub-compatible heading profile because GitHub documents only basic behavior and keeps its exact anchor post-processing unpublished.
