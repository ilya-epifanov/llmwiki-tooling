# Wiki CLI Tool

A Rust CLI for managing LLM-wikis with Obsidian and Markdown internal links.

## Design principles

### No bulk editorial shortcuts

Automate execution of decisions, not editorial decisions. Commands may apply bounded mechanical changes whose correct result follows from repository configuration or an explicit user choice.

Allowed examples:

- `llmwiki-tool links fix` wraps known page mentions in the configured link style.
- `llmwiki-tool links format` applies the explicitly configured link style without changing link targets or prose.
- `llmwiki-tool rename`, `move`, and `sections rename` execute an explicit structural decision and update references.

Do not add batch content generation, bulk editorial metadata changes, or commands that let an agent avoid reading content needed to make a judgment.

## Link authoring

Support both Obsidian and regular Markdown internal links. Follow the repository's configured style when generating links. Prefer links to stable headings; avoid block links when a heading can identify the target.

## Building

```sh
cargo build --release
cargo test
cargo clippy -- -D warnings
```

## Configuration

The tool reads `wiki.toml` from the wiki root. Run `llmwiki-tool setup example-config` for the full schema and `llmwiki-tool setup init` for a detected starting point.
