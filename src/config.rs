use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::Path;

use serde::Deserialize;

use crate::error::ConfigError;
use crate::page::LinkStyle;

/// Severity level for a check or rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    /// Causes non-zero exit code.
    #[default]
    Error,
    /// Prints finding but does not affect exit code.
    Warn,
    /// Suppressed entirely.
    Off,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => f.write_str("error"),
            Self::Warn => f.write_str("warn"),
            Self::Off => f.write_str("off"),
        }
    }
}

impl<'de> Deserialize<'de> for Severity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            "off" => Ok(Self::Off),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["error", "warn", "off"],
            )),
        }
    }
}

/// Complete wiki configuration, parsed from `wiki.toml` or auto-detected.
#[derive(Debug, Clone)]
pub struct WikiConfig {
    /// Path to the index file relative to wiki root. `None` means no index file.
    pub index: Option<String>,
    /// Content directories, sorted most-specific-first for resolution.
    pub directories: Vec<DirectoryConfig>,
    /// Markdown walk ignore settings.
    pub ignore: IgnoreConfig,
    /// Linking behavior settings.
    pub linking: LinkingConfig,
    /// Wiki-wide structural check severities.
    pub checks: ChecksConfig,
    /// Parameterized rules scoped to directories and/or frontmatter predicates.
    pub rules: Vec<RuleConfig>,
}

/// A directory containing wiki pages.
#[derive(Debug, Clone)]
pub struct DirectoryConfig {
    /// Path relative to wiki root.
    pub path: String,
    /// Whether pages in this directory feed bare mention detection.
    pub autolink: bool,
}

pub(crate) const EXAMPLE_CONFIG: &str = include_str!("example-config.toml");
const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    ".agents/",
    ".claude/",
    ".clinerules/",
    ".codex/",
    ".continue/",
    ".cursor/",
    ".gemini/",
    ".github/",
    ".gsd/",
    ".kiro/",
    ".kilocode/",
    ".opencode/",
    ".openhands/",
    ".pi/",
    ".qwen/",
    ".roo/",
    ".windsurf/",
];

/// Markdown walk ignore settings.
#[derive(Debug, Clone)]
pub struct IgnoreConfig {
    /// Whether built-in non-wiki tool directory patterns are included.
    pub default_patterns: bool,
    /// Extra gitignore-style patterns, additive with defaults.
    pub patterns: Vec<String>,
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            default_patterns: true,
            patterns: Vec::new(),
        }
    }
}

impl IgnoreConfig {
    pub(crate) fn effective_patterns(&self) -> impl Iterator<Item = &str> {
        DEFAULT_IGNORE_PATTERNS
            .iter()
            .copied()
            .filter(|_| self.default_patterns)
            .chain(self.patterns.iter().map(String::as_str))
    }
}

/// Global linking behavior.
#[derive(Debug, Clone)]
pub struct LinkingConfig {
    /// Page names to never auto-link.
    pub exclude: HashSet<String>,
    /// Frontmatter field for per-page auto-link opt-out.
    pub autolink_field: String,
    /// Style used for generated and explicitly formatted internal links.
    pub link_style: LinkStyle,
    /// Per-document target count that selects Reference-style Markdown links.
    pub reference_style_threshold: Option<NonZeroUsize>,
}

/// Wiki-wide structural check severities.
#[derive(Debug, Clone)]
pub struct ChecksConfig {
    pub broken_links: Severity,
    pub unmanaged_broken_links: Severity,
    pub orphan_pages: Severity,
    pub index_coverage: Severity,
}

/// Frontmatter predicate that restricts a rule to matching pages.
#[derive(Debug, Clone)]
pub struct RulePredicate {
    pub field: String,
    pub value: String,
}

impl RulePredicate {
    fn parse(expr: &str) -> Result<Self, ConfigError> {
        let Some((field, value)) = expr.split_once("==") else {
            return Err(ConfigError::Validation(format!(
                "unsupported rule predicate '{expr}'; expected `field == value`"
            )));
        };
        let field = field.trim();
        let value = value.trim();
        if field.is_empty() || value.is_empty() {
            return Err(ConfigError::Validation(format!(
                "unsupported rule predicate '{expr}'; expected `field == value`"
            )));
        }
        Ok(Self {
            field: field.to_owned(),
            value: strip_quotes(value).to_owned(),
        })
    }
}

fn strip_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(value)
}

/// A parameterized rule scoped to specific directories and/or frontmatter.
#[derive(Debug, Clone)]
pub enum RuleConfig {
    RequiredSections {
        dirs: Vec<String>,
        when: Option<RulePredicate>,
        sections: Vec<String>,
        severity: Severity,
    },
    RequiredFrontmatter {
        dirs: Vec<String>,
        when: Option<RulePredicate>,
        fields: Vec<String>,
        severity: Severity,
    },
    MirrorParity {
        left: String,
        right: String,
        severity: Severity,
    },
    CitationPattern {
        name: String,
        dirs: Vec<String>,
        when: Option<RulePredicate>,
        pattern: String,
        match_in: String,
        match_mode: MatchMode,
        severity: Severity,
    },
}

impl RuleConfig {
    pub fn severity(&self) -> Severity {
        match self {
            Self::RequiredSections { severity, .. }
            | Self::RequiredFrontmatter { severity, .. }
            | Self::MirrorParity { severity, .. }
            | Self::CitationPattern { severity, .. } => *severity,
        }
    }
}

/// How a citation pattern match is verified against `match_in` pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchMode {
    /// Search page file contents for the captured ID string.
    #[default]
    Content,
    /// Check if a page with the captured ID as its filename exists.
    Filename,
}

impl<'de> Deserialize<'de> for MatchMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "content" => Ok(Self::Content),
            "filename" => Ok(Self::Filename),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["content", "filename"],
            )),
        }
    }
}

// --- TOML deserialization types ---

#[derive(Deserialize)]
struct RawConfig {
    index: Option<String>,
    #[serde(default)]
    verbatim: Vec<String>,
    #[serde(default)]
    directories: Vec<RawDirectoryConfig>,
    #[serde(default)]
    ignore: RawIgnoreConfig,
    #[serde(default)]
    linking: RawLinkingConfig,
    #[serde(default)]
    checks: RawChecksConfig,
    #[serde(default)]
    rules: Vec<RawRuleConfig>,
}

#[derive(Deserialize)]
struct RawDirectoryConfig {
    path: String,
    #[serde(default = "default_true")]
    autolink: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct RawIgnoreConfig {
    #[serde(default = "default_true")]
    default_patterns: bool,
    #[serde(default)]
    patterns: Vec<String>,
}

impl Default for RawIgnoreConfig {
    fn default() -> Self {
        Self {
            default_patterns: true,
            patterns: Vec::new(),
        }
    }
}

#[derive(Deserialize, Default)]
struct RawLinkingConfig {
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default = "default_autolink_field")]
    autolink_field: String,
    #[serde(default)]
    link_style: LinkStyle,
    #[serde(default)]
    reference_style_threshold: Option<NonZeroUsize>,
}

fn default_autolink_field() -> String {
    "autolink".to_owned()
}

#[derive(Deserialize, Default)]
struct RawChecksConfig {
    #[serde(default)]
    broken_links: Option<Severity>,
    #[serde(default)]
    unmanaged_broken_links: Option<Severity>,
    #[serde(default)]
    orphan_pages: Option<Severity>,
    #[serde(default)]
    index_coverage: Option<Severity>,
}

#[derive(Deserialize)]
#[serde(tag = "check")]
enum RawRuleConfig {
    #[serde(rename = "required-sections")]
    RequiredSections {
        #[serde(default)]
        dirs: Vec<String>,
        #[serde(default)]
        when: Option<String>,
        sections: Vec<String>,
        #[serde(default)]
        severity: Option<Severity>,
    },
    #[serde(rename = "required-frontmatter")]
    RequiredFrontmatter {
        #[serde(default)]
        dirs: Vec<String>,
        #[serde(default)]
        when: Option<String>,
        fields: Vec<String>,
        #[serde(default)]
        severity: Option<Severity>,
    },
    #[serde(rename = "mirror-parity")]
    MirrorParity {
        left: String,
        right: String,
        #[serde(default)]
        severity: Option<Severity>,
    },
    #[serde(rename = "citation-pattern")]
    CitationPattern {
        name: String,
        #[serde(default)]
        dirs: Vec<String>,
        #[serde(default)]
        when: Option<String>,
        #[serde(default)]
        pattern: Option<String>,
        #[serde(default)]
        preset: Option<String>,
        match_in: String,
        #[serde(default)]
        match_mode: Option<MatchMode>,
        #[serde(default)]
        severity: Option<Severity>,
    },
}

/// Built-in citation pattern presets.
fn resolve_preset(name: &str) -> Result<(String, MatchMode), ConfigError> {
    match name {
        "bold-method-year" => Ok((
            r"\*\*(?P<id>[A-Za-z][A-Za-z0-9-]+)\*\*\s*\([^)]*\d{4}[^)]*\)".to_owned(),
            MatchMode::Filename,
        )),
        other => Err(ConfigError::UnknownPreset(other.to_owned())),
    }
}

impl WikiConfig {
    /// Load config from `wiki.toml` in the given root directory.
    /// Returns `None` if `wiki.toml` doesn't exist.
    pub fn load(root: &Path) -> Result<Option<Self>, ConfigError> {
        let config_path = root.join("wiki.toml");
        let content = match std::fs::read_to_string(&config_path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(ConfigError::Read {
                    path: config_path,
                    source: e,
                });
            }
        };
        let raw: RawConfig = toml::from_str(&content).map_err(|e| ConfigError::Parse {
            path: config_path,
            source: e,
        })?;
        Self::from_raw(raw).map(Some)
    }

    /// Auto-detect config when no `wiki.toml` exists.
    pub fn auto_detect(root: &Path) -> Self {
        let has_wiki_dir = root.join("wiki").is_dir();
        let dir_path = if has_wiki_dir { "wiki" } else { "." };

        Self {
            index: Some("index.md".to_owned()),
            directories: vec![DirectoryConfig {
                path: dir_path.to_owned(),
                autolink: true,
            }],
            ignore: IgnoreConfig::default(),
            linking: LinkingConfig {
                exclude: HashSet::new(),
                autolink_field: default_autolink_field(),
                link_style: LinkStyle::Obsidian,
                reference_style_threshold: None,
            },
            checks: ChecksConfig {
                broken_links: Severity::Error,
                unmanaged_broken_links: Severity::Warn,
                orphan_pages: Severity::Error,
                index_coverage: Severity::Error,
            },
            rules: Vec::new(),
        }
    }

    /// Load config from wiki.toml if present, otherwise auto-detect.
    pub fn load_or_detect(root: &Path) -> Result<Self, ConfigError> {
        match Self::load(root)? {
            Some(config) => Ok(config),
            None => Ok(Self::auto_detect(root)),
        }
    }

    fn from_raw(raw: RawConfig) -> Result<Self, ConfigError> {
        let mut directories: Vec<DirectoryConfig> = if raw.directories.is_empty() {
            // No directories declared — auto-detect
            vec![DirectoryConfig {
                path: "wiki".to_owned(),
                autolink: true,
            }]
        } else {
            raw.directories
                .into_iter()
                .map(|d| DirectoryConfig {
                    path: normalize_path(&d.path),
                    autolink: d.autolink,
                })
                .collect()
        };

        // Sort most-specific first (longest path) for resolution
        directories.sort_by_key(|dir| std::cmp::Reverse(dir.path.len()));

        let mut ignore_patterns = raw.ignore.patterns;
        ignore_patterns.extend(raw.verbatim);
        let ignore = IgnoreConfig {
            default_patterns: raw.ignore.default_patterns,
            patterns: ignore_patterns,
        };
        validate_ignore_patterns(&ignore)?;

        if raw.linking.reference_style_threshold.is_some()
            && raw.linking.link_style != LinkStyle::Markdown
        {
            return Err(ConfigError::Validation(
                "reference_style_threshold requires markdown link_style".to_owned(),
            ));
        }
        let linking = LinkingConfig {
            exclude: raw.linking.exclude.into_iter().collect(),
            autolink_field: raw.linking.autolink_field,
            link_style: raw.linking.link_style,
            reference_style_threshold: raw.linking.reference_style_threshold,
        };

        let checks = ChecksConfig {
            broken_links: raw.checks.broken_links.unwrap_or(Severity::Error),
            unmanaged_broken_links: raw.checks.unmanaged_broken_links.unwrap_or(Severity::Warn),
            orphan_pages: raw.checks.orphan_pages.unwrap_or(Severity::Error),
            index_coverage: raw.checks.index_coverage.unwrap_or(Severity::Error),
        };

        let mut rules = Vec::new();
        for raw_rule in raw.rules {
            rules.push(convert_rule(raw_rule)?);
        }

        // Validate citation patterns compile as regex
        for rule in &rules {
            if let RuleConfig::CitationPattern { pattern, name, .. } = rule {
                regex_lite::Regex::new(pattern).map_err(|e| ConfigError::InvalidPattern {
                    name: name.clone(),
                    source: e,
                })?;
            }
        }

        Ok(Self {
            index: match raw.index {
                Some(s) if s.is_empty() => None,
                Some(s) => Some(s),
                None => Some("index.md".to_owned()),
            },
            directories,
            ignore,
            linking,
            checks,
            rules,
        })
    }

    /// Get the directory config that applies to a given relative path.
    /// Returns the most-specific matching directory (longest prefix match).
    pub fn directory_for(&self, rel_path: &Path) -> Option<&DirectoryConfig> {
        // Directories are sorted most-specific first
        self.directories
            .iter()
            .find(|d| path_matches_prefix(rel_path, &d.path))
    }

    /// Check if a relative path matches a directory prefix from a rule's `dirs` list.
    pub fn matches_dirs(rel_path: &Path, dirs: &[String]) -> bool {
        dirs.iter().any(|d| path_matches_prefix(rel_path, d))
    }

    /// All mirror-parity rules' `right` paths (non-wiki directories used for parity checks).
    pub fn mirror_paths(&self) -> Vec<(&str, &str)> {
        self.rules
            .iter()
            .filter_map(|r| match r {
                RuleConfig::MirrorParity { left, right, .. } => {
                    Some((left.as_str(), right.as_str()))
                }
                _ => None,
            })
            .collect()
    }
}

fn validate_ignore_patterns(ignore: &IgnoreConfig) -> Result<(), ConfigError> {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(Path::new(""));
    for pattern in ignore.effective_patterns() {
        if pattern.starts_with('!') {
            return Err(ConfigError::Validation(format!(
                "ignore pattern '{pattern}' cannot start with '!'"
            )));
        }
        builder
            .add_line(None, pattern)
            .map_err(|source| ConfigError::InvalidIgnorePattern {
                pattern: pattern.to_owned(),
                source,
            })?;
    }
    builder
        .build()
        .map_err(|source| ConfigError::CompileIgnorePatterns { source })?;
    Ok(())
}

fn convert_rule(raw: RawRuleConfig) -> Result<RuleConfig, ConfigError> {
    match raw {
        RawRuleConfig::RequiredSections {
            dirs,
            when,
            sections,
            severity,
        } => Ok(RuleConfig::RequiredSections {
            dirs: dirs.into_iter().map(|d| normalize_path(&d)).collect(),
            when: parse_when(when)?,
            sections,
            severity: severity.unwrap_or(Severity::Error),
        }),
        RawRuleConfig::RequiredFrontmatter {
            dirs,
            when,
            fields,
            severity,
        } => Ok(RuleConfig::RequiredFrontmatter {
            dirs: dirs.into_iter().map(|d| normalize_path(&d)).collect(),
            when: parse_when(when)?,
            fields,
            severity: severity.unwrap_or(Severity::Error),
        }),
        RawRuleConfig::MirrorParity {
            left,
            right,
            severity,
        } => Ok(RuleConfig::MirrorParity {
            left: normalize_path(&left),
            right: normalize_path(&right),
            severity: severity.unwrap_or(Severity::Error),
        }),
        RawRuleConfig::CitationPattern {
            name,
            dirs,
            when,
            pattern,
            preset,
            match_in,
            match_mode,
            severity,
        } => {
            let (resolved_pattern, resolved_mode) = match (pattern, preset) {
                (Some(p), None) => (p, match_mode.unwrap_or(MatchMode::Content)),
                (None, Some(preset_name)) => {
                    let (p, m) = resolve_preset(&preset_name)?;
                    (p, match_mode.unwrap_or(m))
                }
                (Some(_), Some(_)) => {
                    return Err(ConfigError::Validation(format!(
                        "citation-pattern '{name}': cannot specify both 'pattern' and 'preset'"
                    )));
                }
                (None, None) => {
                    return Err(ConfigError::Validation(format!(
                        "citation-pattern '{name}': must specify either 'pattern' or 'preset'"
                    )));
                }
            };
            Ok(RuleConfig::CitationPattern {
                name,
                dirs: dirs.into_iter().map(|d| normalize_path(&d)).collect(),
                when: parse_when(when)?,
                pattern: resolved_pattern,
                match_in: normalize_path(&match_in),
                match_mode: resolved_mode,
                severity: severity.unwrap_or(Severity::Warn),
            })
        }
    }
}

fn parse_when(when: Option<String>) -> Result<Option<RulePredicate>, ConfigError> {
    when.as_deref().map(RulePredicate::parse).transpose()
}

fn path_matches_prefix(rel_path: &Path, prefix: &str) -> bool {
    prefix == "." || rel_path.starts_with(Path::new(prefix))
}

/// Strip trailing slashes for consistent prefix matching.
fn normalize_path(path: &str) -> String {
    path.trim_end_matches('/').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config() {
        let toml = r#"
[[directories]]
path = "wiki"
"#;
        let raw: RawConfig = toml::from_str(toml).unwrap();
        let config = WikiConfig::from_raw(raw).unwrap();
        assert_eq!(config.directories.len(), 1);
        assert_eq!(config.directories[0].path, "wiki");
        assert!(config.directories[0].autolink);
        assert_eq!(config.checks.broken_links, Severity::Error);
        assert_eq!(config.checks.unmanaged_broken_links, Severity::Warn);
    }

    #[test]
    fn parses_markdown_link_style() {
        let raw: RawConfig =
            toml::from_str("[linking]\nlink_style = \"markdown\"\nreference_style_threshold = 2\n")
                .unwrap();
        let config = WikiConfig::from_raw(raw).unwrap();

        assert_eq!(config.linking.link_style, LinkStyle::Markdown);
        assert_eq!(
            config
                .linking
                .reference_style_threshold
                .map(NonZeroUsize::get),
            Some(2)
        );
    }

    #[test]
    fn rejects_reference_style_for_obsidian_links() {
        let raw: RawConfig =
            toml::from_str("[linking]\nlink_style = \"obsidian\"\nreference_style_threshold = 1\n")
                .unwrap();

        assert!(matches!(
            WikiConfig::from_raw(raw),
            Err(ConfigError::Validation(message))
                if message.contains("reference_style_threshold requires markdown")
        ));
    }

    #[test]
    fn parses_full_config() {
        let toml = r#"
index = "contents.md"

[[directories]]
path = "wiki"

[[directories]]
path = "wiki/papers"
autolink = false

[linking]
exclude = ["the", "a"]
autolink_field = "auto"

[checks]
broken_links = "error"
unmanaged_broken_links = "warn"
orphan_pages = "warn"
index_coverage = "off"

[[rules]]
check = "required-sections"
dirs = ["wiki/concepts"]
sections = ["See also"]
severity = "error"

[[rules]]
check = "mirror-parity"
left = "wiki/papers"
right = "raw/papers"
severity = "warn"

[[rules]]
check = "citation-pattern"
name = "arxiv"
dirs = ["wiki"]
pattern = 'arxiv\.org/abs/(?P<id>\d{4}\.\d{4,5})'
match_in = "wiki/papers"
severity = "warn"

[[rules]]
check = "citation-pattern"
name = "bold-method"
preset = "bold-method-year"
dirs = ["wiki"]
match_in = "wiki/papers"
severity = "warn"
"#;
        let raw: RawConfig = toml::from_str(toml).unwrap();
        let config = WikiConfig::from_raw(raw).unwrap();

        assert_eq!(config.index.as_deref(), Some("contents.md"));
        assert!(config.linking.exclude.contains("the"));
        assert_eq!(config.linking.autolink_field, "auto");
        assert_eq!(config.checks.unmanaged_broken_links, Severity::Warn);
        assert_eq!(config.checks.orphan_pages, Severity::Warn);
        assert_eq!(config.checks.index_coverage, Severity::Off);
        assert_eq!(config.rules.len(), 4);

        // Most specific directory first
        assert_eq!(config.directories[0].path, "wiki/papers");
        assert!(!config.directories[0].autolink);
        assert_eq!(config.directories[1].path, "wiki");
        assert!(config.directories[1].autolink);
    }

    #[test]
    fn example_config_is_parseable() {
        let raw: RawConfig = toml::from_str(EXAMPLE_CONFIG).unwrap();
        WikiConfig::from_raw(raw).unwrap();
    }

    #[test]
    fn directory_resolution_most_specific_wins() {
        let config = WikiConfig {
            index: None,
            directories: vec![
                DirectoryConfig {
                    path: "wiki/papers".to_owned(),
                    autolink: false,
                },
                DirectoryConfig {
                    path: "wiki".to_owned(),
                    autolink: true,
                },
            ],
            ignore: IgnoreConfig::default(),
            linking: LinkingConfig {
                exclude: HashSet::new(),
                autolink_field: "autolink".to_owned(),
                link_style: LinkStyle::Obsidian,
                reference_style_threshold: None,
            },
            checks: ChecksConfig {
                broken_links: Severity::Error,
                unmanaged_broken_links: Severity::Warn,
                orphan_pages: Severity::Error,
                index_coverage: Severity::Error,
            },
            rules: Vec::new(),
        };

        assert!(
            config
                .directory_for(Path::new("wiki/concepts/GRPO.md"))
                .is_some_and(|dir| dir.autolink)
        );
        assert!(
            config
                .directory_for(Path::new("wiki/papers/deepseek.md"))
                .is_some_and(|dir| !dir.autolink)
        );
    }

    #[test]
    fn auto_detect_with_wiki_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("wiki")).unwrap();
        std::fs::write(dir.path().join("index.md"), "# Index").unwrap();

        let config = WikiConfig::auto_detect(dir.path());
        assert_eq!(config.directories[0].path, "wiki");
        assert_eq!(config.index.as_deref(), Some("index.md"));
    }

    #[test]
    fn auto_detect_flat_wiki() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.md"), "# Index").unwrap();

        let config = WikiConfig::auto_detect(dir.path());
        assert_eq!(config.directories[0].path, ".");
    }

    #[test]
    fn rejects_pattern_and_preset_together() {
        let toml = r#"
[[rules]]
check = "citation-pattern"
name = "test"
dirs = ["wiki"]
pattern = "foo"
preset = "bold-method-year"
match_in = "wiki"
"#;
        let raw: RawConfig = toml::from_str(toml).unwrap();
        let err = WikiConfig::from_raw(raw).unwrap_err();
        assert!(err.to_string().contains("cannot specify both"));
    }

    #[test]
    fn rejects_unknown_preset() {
        let toml = r#"
[[rules]]
check = "citation-pattern"
name = "test"
dirs = ["wiki"]
preset = "nonexistent"
match_in = "wiki"
"#;
        let raw: RawConfig = toml::from_str(toml).unwrap();
        let err = WikiConfig::from_raw(raw).unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn parses_rule_when_predicate() {
        let toml = r#"
[[rules]]
check = "required-frontmatter"
when = "type == concept"
fields = ["owner"]
"#;
        let raw: RawConfig = toml::from_str(toml).unwrap();
        let config = WikiConfig::from_raw(raw).unwrap();

        let RuleConfig::RequiredFrontmatter { dirs, when, .. } = &config.rules[0] else {
            panic!("expected required-frontmatter rule");
        };
        assert!(dirs.is_empty());
        let when = when.as_ref().unwrap();
        assert_eq!(when.field, "type");
        assert_eq!(when.value, "concept");
    }

    #[test]
    fn parses_ignore_config() {
        let toml = r#"
[ignore]
default_patterns = false
patterns = ["generated/"]
"#;
        let raw: RawConfig = toml::from_str(toml).unwrap();
        let config = WikiConfig::from_raw(raw).unwrap();

        assert!(!config.ignore.default_patterns);
        assert_eq!(config.ignore.patterns, ["generated/"]);
        assert_eq!(
            config.ignore.effective_patterns().collect::<Vec<_>>(),
            ["generated/"]
        );
    }

    #[test]
    fn verbatim_patterns_are_additive_ignores() {
        let toml = r#"
verbatim = ["raw-inputs/", "scraped/"]

[ignore]
patterns = ["generated/"]
"#;
        let raw: RawConfig = toml::from_str(toml).unwrap();
        let config = WikiConfig::from_raw(raw).unwrap();

        assert!(config.ignore.patterns.contains(&"generated/".to_owned()));
        assert!(config.ignore.patterns.contains(&"raw-inputs/".to_owned()));
        assert!(config.ignore.patterns.contains(&"scraped/".to_owned()));
    }

    #[test]
    fn rejects_unignore_patterns() {
        let toml = r#"
[ignore]
patterns = ["!.claude/"]
"#;
        let raw: RawConfig = toml::from_str(toml).unwrap();
        let err = WikiConfig::from_raw(raw).unwrap_err();
        assert!(err.to_string().contains("cannot start with '!'"));
    }

    #[test]
    fn matches_dirs_prefix() {
        assert!(WikiConfig::matches_dirs(
            Path::new("wiki/concepts/GRPO.md"),
            &["wiki/concepts".to_owned()]
        ));
        assert!(WikiConfig::matches_dirs(
            Path::new("wiki/concepts/GRPO.md"),
            &["wiki".to_owned()]
        ));
        assert!(!WikiConfig::matches_dirs(
            Path::new("wiki/papers/foo.md"),
            &["wiki/concepts".to_owned()]
        ));
        assert!(!WikiConfig::matches_dirs(
            Path::new("wikiology/foo.md"),
            &["wiki".to_owned()]
        ));
    }
}
