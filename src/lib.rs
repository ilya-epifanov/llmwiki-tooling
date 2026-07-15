mod cmd;
mod config;
mod edit_plan;
mod error;
mod frontmatter;
mod inventory;
mod link_format;
mod link_index;
mod markdown_document;
mod markdown_links;
mod mention;
mod page;
mod splice;
mod wiki;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use crate::cmd::lint::SeverityFilter;
use crate::config::WikiConfig;
use crate::error::WikiError;
use crate::wiki::{Wiki, WikiRoot};

#[derive(Parser)]
#[command(
    name = "llmwiki-tool",
    about = "Manage LLM wiki knowledge bases",
    version
)]
struct Cli {
    /// Wiki root directory (auto-detected from CWD if omitted)
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Internal link operations
    Links {
        #[command(subcommand)]
        action: LinksAction,
    },
    /// Run all checks (structural + rules)
    Lint {
        /// Filter by severity level
        #[arg(long, value_enum, default_value_t = SeverityArg::All)]
        severity: SeverityArg,
    },
    /// Rename a page with full reference update
    Rename {
        /// Current page name (without .md)
        old: String,
        /// New page name (without .md)
        new: String,
        /// Apply changes (default: dry-run)
        #[arg(long)]
        write: bool,
    },
    /// Move a page to another directory and rebase relative markdown links
    Move {
        /// Page name (without .md)
        page: String,
        /// Destination directory, relative to wiki root
        dest_dir: PathBuf,
        /// Apply changes (default: dry-run)
        #[arg(long)]
        write: bool,
    },
    /// Query the link graph
    Refs {
        #[command(subcommand)]
        action: RefsAction,
    },
    /// Section heading operations
    Sections {
        #[command(subcommand)]
        action: SectionsAction,
    },
    /// Frontmatter operations
    Frontmatter {
        #[command(subcommand)]
        action: FrontmatterAction,
    },
    /// Scan wiki structure and output per-directory statistics
    Scan,
    /// Setup and configuration utilities
    Setup {
        #[command(subcommand)]
        action: SetupAction,
    },
}

#[derive(Subcommand)]
enum SetupAction {
    /// Output setup workflow prompt for an LLM agent
    Prompt,
    /// Output a complete annotated wiki.toml with all options
    ExampleConfig,
    /// Generate a minimal wiki.toml from detected structure
    Init {
        /// Print to stdout instead of writing wiki.toml
        #[arg(long)]
        show: bool,
        /// Overwrite existing wiki.toml
        #[arg(long, short)]
        force: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum SeverityArg {
    All,
    Error,
    Warn,
}

impl From<SeverityArg> for SeverityFilter {
    fn from(arg: SeverityArg) -> Self {
        match arg {
            SeverityArg::All => Self::All,
            SeverityArg::Error => Self::ErrorOnly,
            SeverityArg::Warn => Self::WarnOnly,
        }
    }
}

#[derive(Subcommand)]
enum LinksAction {
    /// Find bare mentions that should be internal links
    Check,
    /// Auto-link bare mentions
    Fix {
        /// Apply changes (default: dry-run showing diff)
        #[arg(long)]
        write: bool,
    },
    /// Convert internal links to the repository's preferred style
    Format {
        /// Apply changes (default: dry-run showing diff)
        #[arg(long)]
        write: bool,
    },
    /// Find internal links pointing to non-existent pages/headings/blocks
    Broken,
    /// Find pages with no inbound internal links
    Orphans,
}

#[derive(Subcommand)]
enum RefsAction {
    /// Pages that link to the given page
    To { page: String },
    /// Pages the given page links to
    From { page: String },
    /// Full link graph
    Graph,
}

#[derive(Subcommand)]
enum FrontmatterAction {
    /// Extract frontmatter (JSON output)
    Get {
        file: PathBuf,
        /// Specific field to extract
        field: Option<String>,
    },
    /// Modify a frontmatter field
    Set {
        file: PathBuf,
        field: String,
        value: String,
    },
}

#[derive(Subcommand)]
enum SectionsAction {
    /// Rename a heading and its Obsidian and Markdown references
    Rename {
        /// Current heading text
        old: String,
        /// New heading text
        new: String,
        /// Only rename in these directories (path prefix)
        #[arg(long)]
        dirs: Option<Vec<String>>,
        /// Apply changes (default: dry-run)
        #[arg(long)]
        write: bool,
    },
}

fn resolve_root(cli_root: Option<PathBuf>) -> Result<WikiRoot, WikiError> {
    match cli_root {
        Some(path) => WikiRoot::from_path(path),
        None => {
            let cwd = std::env::current_dir().map_err(|_| WikiError::RootNotFound {
                start: PathBuf::from("."),
            })?;
            WikiRoot::discover(&cwd)
        }
    }
}

fn run_inner() -> Result<ExitCode, anyhow::Error> {
    let cli = Cli::parse();
    let root = resolve_root(cli.root)?;

    // Commands that don't need config/catalog
    match &cli.command {
        Command::Scan => {
            let config = WikiConfig::load_or_detect(root.path())?;
            crate::cmd::agent::scan(&root, &config.ignore)?;
            return Ok(ExitCode::SUCCESS);
        }
        Command::Setup { action } => {
            match action {
                SetupAction::Prompt => crate::cmd::agent::setup(&root)?,
                SetupAction::ExampleConfig => crate::cmd::agent::example_config(),
                SetupAction::Init { force, show } => crate::cmd::init::init(&root, *force, *show)?,
            }
            return Ok(ExitCode::SUCCESS);
        }
        _ => {}
    }

    // Commands that need config and wiki
    let config = WikiConfig::load_or_detect(root.path())?;
    let mut wiki = Wiki::build(root, config)?;

    match cli.command {
        Command::Links { action } => match action {
            LinksAction::Check => {
                let count = crate::cmd::links::check(&wiki)?;
                if count > 0 {
                    eprintln!("{count} bare mention(s) found");
                }
            }
            LinksAction::Fix { write } => {
                let count = crate::cmd::links::fix(&mut wiki, write)?;
                if count > 0 && !write {
                    eprintln!("{count} bare mention(s) to fix. Use --write to apply.");
                } else if count == 0 {
                    eprintln!("no bare mentions found");
                }
            }
            LinksAction::Format { write } => {
                let count = crate::cmd::links::format(&mut wiki, write)?;
                if count == 0 {
                    eprintln!("no links to format");
                } else if !write {
                    eprintln!("{count} link(s) to format. Use --write to apply.");
                }
            }
            LinksAction::Broken => {
                let count = crate::cmd::links::broken(&wiki)?;
                if count > 0 {
                    eprintln!("{count} broken link(s) found");
                    return Ok(ExitCode::from(1));
                }
            }
            LinksAction::Orphans => {
                let count = crate::cmd::links::orphans(&wiki)?;
                if count > 0 {
                    eprintln!("{count} orphan page(s) found");
                }
            }
        },

        Command::Lint { severity } => {
            let errors = crate::cmd::lint::lint(&wiki, severity.into())?;
            if errors > 0 {
                return Ok(ExitCode::from(2));
            }
        }

        Command::Rename { old, new, write } => {
            crate::cmd::rename::rename(&mut wiki, &old, &new, write)?;
        }

        Command::Move {
            page,
            dest_dir,
            write,
        } => {
            crate::cmd::move_page::move_page(&mut wiki, &page, &dest_dir, write)?;
        }

        Command::Refs { action } => match action {
            RefsAction::To { page } => {
                crate::cmd::refs::refs_to(&wiki, &page)?;
            }
            RefsAction::From { page } => {
                crate::cmd::refs::refs_from(&wiki, &page)?;
            }
            RefsAction::Graph => {
                crate::cmd::refs::refs_graph(&wiki)?;
            }
        },

        Command::Sections { action } => match action {
            SectionsAction::Rename {
                old,
                new,
                dirs,
                write,
            } => {
                let count = crate::cmd::sections::rename(&mut wiki, &old, &new, &dirs, write)?;
                if count > 0 && !write {
                    eprintln!("{count} occurrence(s) to rename. Use --write to apply.");
                } else if count == 0 {
                    eprintln!("no occurrences of '{}' found", old);
                }
            }
        },

        Command::Frontmatter { action } => match action {
            FrontmatterAction::Get { file, field } => {
                crate::cmd::frontmatter_cmd::get(&wiki, &file, field.as_deref())?;
            }
            FrontmatterAction::Set { file, field, value } => {
                crate::cmd::frontmatter_cmd::set(&mut wiki, &file, &field, &value)?;
            }
        },

        // Handled in the early match above
        Command::Scan | Command::Setup { .. } => unreachable!(),
    }

    Ok(ExitCode::SUCCESS)
}

pub fn run() -> ExitCode {
    match run_inner() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
