# `llmwiki-tool` - CLI for managing LLM-wikis

A Rust CLI for managing [LLM-wikis](https://github.com/karpathy/LLM-wiki) — markdown knowledge bases with Obsidian-style wikilinks.

Designed to simplify an LLM agent's job of keeping the wiki clean: fix broken links, rename pages with full reference updates, detect orphans, and lint against configurable rules — all through commands that produce structured output and save tokens.

The tool is **not opinionated**. It adapts to your wiki's structure via `wiki.toml` configuration rather than imposing conventions.

See also: [wikidesk](https://github.com/ilya-epifanov/wikidesk) — a companion server that lets multiple AI agents share a wiki and dispatch research requests.

## Installation

> Install inside your LLM-wiki environment.

<details>
<summary>macOS / Linux (pre-built binary)</summary>

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ilya-epifanov/llmwiki-tooling/releases/latest/download/llmwiki-tooling-installer.sh | sh
```

</details>

<details>
<summary>Windows (pre-built binary)</summary>

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/ilya-epifanov/llmwiki-tooling/releases/latest/download/llmwiki-tooling-installer.ps1 | iex"
```

</details>

<details>
<summary>From source (any platform with Rust 1.85+)</summary>

```sh
cargo install llmwiki-tooling
```

</details>

## Setup

Once installed, tell your wiki-maintaining agent:

> Run `llmwiki-tool setup prompt` and follow the instructions to configure the tool for this wiki.

The setup prompt walks the agent through scanning your wiki structure and generating a `wiki.toml` config file.

## Configuration

The tool reads `wiki.toml` from the wiki root. The agent can generate this automatically via the setup prompt, or you can create one manually:

```bash
llmwiki-tool setup init              # Generate wiki.toml from detected structure
llmwiki-tool setup example-config    # Output annotated wiki.toml with all options
```

<details>
<summary>Command reference</summary>

### Links

```bash
llmwiki-tool links check       # Find bare mentions that should be wikilinks
llmwiki-tool links fix         # Auto-link bare mentions (dry-run by default, --write to apply)
llmwiki-tool links broken      # Find wikilinks pointing to non-existent pages/headings/blocks
llmwiki-tool links orphans     # Find pages with no inbound wikilinks
```

### Rename

```bash
llmwiki-tool rename "Old Page" "New Page"         # Rename page with full reference update (dry-run)
llmwiki-tool rename "Old Page" "New Page" --write # Apply changes
```

Updates all `[[Old Page]]`, `[[Old Page#heading]]`, and `[[Old Page|alias]]` references across the wiki.

### Sections

```bash
llmwiki-tool sections rename "Key results" "Key findings"                  # Dry-run
llmwiki-tool sections rename "Key results" "Key findings" --write          # Apply
llmwiki-tool sections rename "Old heading" "New heading" --dirs papers/    # Limit scope
```

Renames headings and updates all `[[page#heading]]` fragment references.

### References

```bash
llmwiki-tool refs to "Page Name"    # Pages that link to the given page
llmwiki-tool refs from "Page Name"  # Pages the given page links to
llmwiki-tool refs graph             # Full link graph
```

### Frontmatter

```bash
llmwiki-tool frontmatter get path/to/file.md           # Extract frontmatter as JSON
llmwiki-tool frontmatter get path/to/file.md tags      # Extract specific field
llmwiki-tool frontmatter set path/to/file.md tags "a,b,c"
```

### Lint

```bash
llmwiki-tool lint                   # Run all checks (structural + configured rules)
llmwiki-tool lint --severity error  # Only errors
llmwiki-tool lint --severity warn   # Only warnings
```

### Scan

```bash
llmwiki-tool scan    # Output per-directory statistics (file counts, frontmatter fields, headings)
```

</details>

## Building from source

```bash
cargo build --release
cargo test
cargo clippy -- -D warnings
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
