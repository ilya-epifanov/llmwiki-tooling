# Wiki CLI Tool

A Rust CLI for managing LLM-wikis — markdown knowledge bases with Obsidian-style wikilinks.

## Design principles

### No bulk editorial shortcuts

The tool automates _execution_ of decisions, not the decisions themselves. Every command operates on an explicit, bounded input (a page name, a section name, a specific text pattern). No command should apply editorial changes across files without the agent examining each file.

**Why this matters:** The value of an LLM-wiki is that every note is maintained carefully. A `set-all` or `apply-everywhere` command lets the agent skip thinking about individual pages. That produces shallow, homogenized content — exactly what the wiki exists to avoid.

**What's allowed:**
- `llmwiki-tool links fix` — mechanical correctness. If a concept exists and its name appears bare in prose, wrapping it in `[[]]` is objectively correct. No editorial judgment.
- `llmwiki-tool rename` — executes a naming decision the agent/user already made. The tool's job is to update references correctly, not to decide whether to rename.
- `llmwiki-tool sections rename` — same. Converging "Key results" → "Key findings" is a style/consistency decision made upfront. The tool handles the mechanical find-and-replace including heading fragment references.

**What's not allowed:**
- Batch frontmatter modification across all files in a directory
- Auto-generating content (summaries, sections, boilerplate)
- Any command where the agent wouldn't need to read the target files to decide whether the operation is appropriate

**Test for new commands:** "Could an agent use this command responsibly without reading the affected files?" If yes (links fix, rename, sections rename), the command is fine. If no (batch set, auto-fill), the command enables laziness.

## Building

```
cargo build --release
cargo test
cargo clippy -- -D warnings
```

## Configuration

The tool reads `wiki.toml` from the wiki root. Run `llmwiki-tool setup example-config` for the full schema reference. Run `llmwiki-tool setup init` to generate a minimal config from detected structure.
