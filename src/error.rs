use std::path::PathBuf;

use crate::page::PageId;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("reading config '{path}'")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("parsing config '{path}'")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("unknown citation preset: '{0}'")]
    UnknownPreset(String),

    #[error("invalid citation pattern '{name}'")]
    InvalidPattern {
        name: String,
        #[source]
        source: regex_lite::Error,
    },

    #[error("config validation: {0}")]
    Validation(String),
}

#[derive(Debug, thiserror::Error)]
pub enum WikiError {
    #[error("wiki root not found (searched ancestors of {start})")]
    RootNotFound { start: PathBuf },

    #[error("reading '{path}'")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("writing '{path}'")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("walking '{path}'")]
    Walk {
        path: PathBuf,
        #[source]
        source: ignore::Error,
    },

    #[error("parsing frontmatter in '{path}'")]
    Frontmatter {
        path: PathBuf,
        #[source]
        source: FrontmatterError,
    },

    #[error("page not found: {0}")]
    PageNotFound(PageId),

    #[error("duplicate page ID '{id}' in '{path1}' and '{path2}'")]
    DuplicatePageId {
        id: String,
        path1: PathBuf,
        path2: PathBuf,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    #[error("YAML parse error near '{context}'")]
    Yaml {
        #[source]
        source: Box<serde_yml::Error>,
        context: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum RenameError {
    #[error(transparent)]
    Wiki(#[from] WikiError),

    #[error("source page not found: {0}")]
    SourceNotFound(PageId),

    #[error("target page already exists: '{path}'")]
    TargetExists { path: PathBuf },
}

#[derive(Debug, thiserror::Error)]
pub enum LintError {
    #[error(transparent)]
    Wiki(#[from] WikiError),

    #[error("{count} lint error(s) found")]
    Failed { count: usize },
}
