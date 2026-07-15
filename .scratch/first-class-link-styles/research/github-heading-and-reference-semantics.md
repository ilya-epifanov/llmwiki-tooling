# GitHub heading anchors and Reference-style link semantics

Research date: 2026-07-15. Sources are the current CommonMark specification,
official GitHub documentation, and GitHub-owned source.

## Resolution

CommonMark Reference-style links are precisely specified and can be parsed as
first-class links. `Page#Heading` and `Page#^block-id` are valid reference
labels without escaping.

GitHub-compatible heading anchors are different: GitHub documents useful basic
rules, but neither GFM nor public GitHub source specifies the exact algorithm.
The project should define and fixture-test a small local compatibility profile
instead of promising exact parity for undocumented Unicode and collision edge
cases.

## GitHub heading anchors

GitHub documents these basic behaviors:

- lower-case letters;
- replace spaces with `-` and remove other whitespace and punctuation;
- trim leading and trailing whitespace;
- remove markup while retaining its contents; and
- when an anchor duplicates an earlier anchor in the document, append `-N` in
  document order. The documented example starts with `-1`.

The official example turns
`This'll be a _Helpful_ Section About the Greek Letter Θ!` into
`thisll-be-a-helpful-section-about-the-greek-letter-Θ`, and assigns the second
identical heading `this-heading-is-not-unique-in-the-file-1`.
[GitHub's rules and examples](https://github.com/github/docs/blob/e55cfd220e42eb2bd306a9c7b494384c0c3f2dc9/content/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax.md#L131-L171)

These are explicitly described as "basic rules," not a normative algorithm.
The example retains uppercase `Θ` despite the general lower-casing statement.
The documentation does not settle Unicode normalization or casing, the exact
punctuation classification, repeated-space behavior, or collisions such as
`X`, `X`, `X-1`, `X`.
[GitHub's Unicode and duplicate example](https://github.com/github/docs/blob/e55cfd220e42eb2bd306a9c7b494384c0c3f2dc9/content/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax.md#L141-L167)

Heading IDs are not part of GFM syntax. The GFM specification parses heading
contents as inline content and renders plain `<hN>` elements, while explicitly
noting that GitHub performs additional post-processing after GFM-to-HTML
conversion.
[GFM introduction](https://github.github.com/gfm/#what-is-github-flavored-markdown)
[GFM ATX headings](https://github.github.com/gfm/#atx-headings)

GitHub's public rendering library confirms that named anchors are added by
later GitHub.com filters and that the library covers only the initial markup
conversion. The production anchor implementation is not published there.
[github/markup rendering pipeline](https://github.com/github/markup/blob/a365c64ef765f3677ed50947081d29b863b6ff4c/README.md#L3-L12)

### Implementation implications

- Derive fragments from parsed heading inline content, not a regex over raw
  Markdown, so markup delimiters disappear while their text remains.
- Implement the documented transformations and sequential `-1`, `-2`, ...
  duplicate suffixes as the project's explicit compatibility profile.
- Preserve a literal Unicode code point unless a documented rule clearly
  transforms it; do not silently add NFC/NFKC normalization.
- Pin fixtures for punctuation, non-ASCII text, composed/decomposed Unicode,
  duplicate headings, and pre-suffixed collision cases. These fixtures define
  project behavior where GitHub has no stable public contract.
- Call the result "GitHub-compatible," not an exact implementation of a stable
  GFM algorithm.

## CommonMark Reference-style links

CommonMark defines full, collapsed, and shortcut references. A full reference
is `[visible text][label]`; collapsed `[label][]` and shortcut `[label]` use the
label as visible text.
[Reference forms](https://github.com/commonmark/commonmark-spec/blob/0.31.2/spec.txt#L7958-L7985)
[Collapsed references](https://github.com/commonmark/commonmark-spec/blob/0.31.2/spec.txt#L8308-L8317)
[Shortcut references](https://github.com/commonmark/commonmark-spec/blob/0.31.2/spec.txt#L8361-L8371)

A label must contain a non-whitespace character, may contain at most 999
characters, and cannot contain an unescaped `[` or `]`. `#` and `^` have no
special role in this grammar, so both of these are valid:

```markdown
[Training Results#Key Findings]: topics/Training%20Results.md#key-findings
[Page#^block-id]: topics/Page.md#^block-id
```

[CommonMark label grammar](https://github.com/commonmark/commonmark-spec/blob/0.31.2/spec.txt#L7965-L7974)

Label matching is not byte-exact. It strips the brackets, performs Unicode case
folding, trims leading and trailing spaces, tabs, and line endings, and
collapses consecutive internal instances of those characters to one space.
It does not specify NFC/NFKC normalization and it does not remove `#`, `^`, or
other punctuation. For example, `ẞ` matches `SS`.
[Label normalization](https://github.com/commonmark/commonmark-spec/blob/0.31.2/spec.txt#L7975-L7983)
[Unicode case-fold example](https://spec.commonmark.org/0.31.2/#example-540)

Matching compares normalized source labels, not parsed inline content. Thus an
unnecessary escape can change identity: `foo\!` does not match `foo!`. When
multiple definitions have the same normalized label, the first definition in
the document wins.
[Normalized-source matching and first-wins behavior](https://github.com/commonmark/commonmark-spec/blob/0.31.2/spec.txt#L8201-L8224)

A definition may precede or follow its uses, so a final Reference-style section
is standard. A definition cannot interrupt a paragraph; a blank line before
the final definitions is the safe canonical form.
[Definition grammar and placement](https://github.com/commonmark/commonmark-spec/blob/0.31.2/spec.txt#L3161-L3179)
[Paragraph restriction](https://spec.commonmark.org/0.31.2/#example-213)

Link labels and destinations are separate domains: labels are not URL-encoded.
A bare destination cannot contain spaces, while an angle-bracket destination
may. Existing percent escapes are preserved. CommonMark does not mandate
whether a renderer percent-encodes non-ASCII destination characters or applies
other URL normalization.
[Destination grammar](https://github.com/commonmark/commonmark-spec/blob/0.31.2/spec.txt#L7489-L7502)
[URL escaping policy](https://github.com/commonmark/commonmark-spec/blob/0.31.2/spec.txt#L7722-L7730)

### Implementation implications

- Parse all three standard reference forms, but emitting only shortcut and full
  references is a valid canonical formatting policy.
- Build the definition table using CommonMark label normalization. Detect
  collisions after normalization, including Unicode case-fold collisions, and
  suffix generated labels before emitting them.
- Use the same escaped label spelling at the use and definition. Escape literal
  square brackets; do not escape `#` or `^`.
- Keep URL encoding in the destination serializer. For the chosen bare
  destination form, percent-encode spaces as `%20`, preserve existing escapes,
  and keep the path/fragment `#` delimiter distinct from label text.
- Put generated definitions after a blank line at the end of the document.

## Decision supported by this research

Adopt exact CommonMark semantics for Reference-style parsing and matching.
Adopt a documented, fixture-backed GitHub compatibility profile for heading
fragments, with the undocumented Unicode and duplicate-collision cases treated
as project choices rather than hidden claims of GitHub parity.
