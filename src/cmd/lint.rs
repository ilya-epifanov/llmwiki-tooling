use crate::config::{MatchMode, RuleConfig, Severity, WikiConfig};
use crate::error::WikiError;
use crate::link_index::LinkIndex;
use crate::resolve;
use crate::wiki::Wiki;

/// Severity filter for lint output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityFilter {
    /// Show all non-off checks (default).
    All,
    /// Show only errors.
    ErrorOnly,
    /// Show only warnings.
    WarnOnly,
}

struct LintResult {
    errors: usize,
    warnings: usize,
}

impl LintResult {
    fn new() -> Self {
        Self {
            errors: 0,
            warnings: 0,
        }
    }

    fn tally(&mut self, count: usize, severity: Severity) {
        match severity {
            Severity::Error => self.errors += count,
            Severity::Warn => self.warnings += count,
            Severity::Off => {}
        }
    }
}

fn should_show(severity: Severity, filter: SeverityFilter) -> bool {
    match (severity, filter) {
        (Severity::Off, _) => false,
        (_, SeverityFilter::All) => true,
        (Severity::Error, SeverityFilter::ErrorOnly) => true,
        (Severity::Warn, SeverityFilter::WarnOnly) => true,
        _ => false,
    }
}

/// Run all lint checks. Returns the number of errors (not warnings).
pub fn lint(wiki: &Wiki, filter: SeverityFilter) -> Result<usize, WikiError> {
    let mut result = LintResult::new();
    let config = wiki.config();

    // Wiki-wide structural checks
    if should_show(config.checks.broken_links, filter) {
        let count = run_broken_links(wiki)?;
        result.tally(count, config.checks.broken_links);
    }

    if should_show(config.checks.orphan_pages, filter) {
        let link_index = LinkIndex::build(wiki)?;
        let count = run_orphan_pages(wiki, &link_index);
        result.tally(count, config.checks.orphan_pages);
    }

    if should_show(config.checks.index_coverage, filter)
        && let Some(index_path) = wiki.index_path()
    {
        if index_path.is_file() {
            let count = run_index_coverage(wiki, &index_path)?;
            result.tally(count, config.checks.index_coverage);
        } else {
            eprintln!(
                "warn: index file '{}' not found, skipping index coverage",
                index_path.display()
            );
        }
    }

    // Parameterized rules
    for rule in &config.rules {
        let severity = rule.severity();
        if !should_show(severity, filter) {
            continue;
        }
        let count = run_rule(wiki, rule)?;
        result.tally(count, severity);
    }

    if result.errors > 0 || result.warnings > 0 {
        let mut parts = Vec::new();
        if result.errors > 0 {
            parts.push(format!("{} error(s)", result.errors));
        }
        if result.warnings > 0 {
            parts.push(format!("{} warning(s)", result.warnings));
        }
        eprintln!("{}", parts.join(", "));
    }

    Ok(result.errors)
}

fn run_rule(wiki: &Wiki, rule: &RuleConfig) -> Result<usize, WikiError> {
    match rule {
        RuleConfig::RequiredSections {
            dirs,
            sections,
            severity,
            ..
        } => run_required_sections(wiki, dirs, sections, *severity),
        RuleConfig::RequiredFrontmatter {
            dirs,
            fields,
            severity,
            ..
        } => run_required_frontmatter(wiki, dirs, fields, *severity),
        RuleConfig::MirrorParity {
            left,
            right,
            severity,
            ..
        } => run_mirror_parity(wiki, left, right, *severity),
        RuleConfig::CitationPattern {
            name,
            dirs,
            pattern,
            match_in,
            match_mode,
            severity,
        } => run_citation_pattern(wiki, name, dirs, pattern, match_in, *match_mode, *severity),
    }
}

fn run_broken_links(wiki: &Wiki) -> Result<usize, WikiError> {
    let severity = wiki.config().checks.broken_links;
    let mut count = 0;
    for file_path in wiki.all_scannable_files() {
        let broken = resolve::find_broken_links(wiki, &file_path)?;
        let source = wiki.source(&file_path)?;
        let rel_path = wiki.rel_path(&file_path);
        for (wl, reason) in &broken {
            let ref_text = &source[wl.byte_range.clone()];
            eprintln!(
                "{severity}[broken-link]: {} in {}",
                ref_text.trim(),
                rel_path.display(),
            );
            eprintln!("  -> {reason}");
            count += 1;
        }
    }
    Ok(count)
}

fn run_orphan_pages(wiki: &Wiki, link_index: &LinkIndex) -> usize {
    let orphans = link_index.orphans(wiki);
    for page_id in &orphans {
        if let Some(entry) = wiki.get(page_id) {
            eprintln!(
                "error[orphan]: {} has no inbound wikilinks",
                entry.rel_path.display(),
            );
        }
    }
    orphans.len()
}

fn run_index_coverage(wiki: &Wiki, index_path: &std::path::Path) -> Result<usize, WikiError> {
    let index_wikilinks = wiki.wikilinks(index_path)?;
    let referenced: std::collections::HashSet<&str> =
        index_wikilinks.iter().map(|wl| wl.page.as_str()).collect();

    let mut count = 0;
    for (page_id, entry) in wiki.pages() {
        if !referenced.contains(page_id.as_str()) {
            eprintln!(
                "error[not-in-index]: {} is not listed in index",
                entry.rel_path.display(),
            );
            count += 1;
        }
    }
    Ok(count)
}

fn run_required_sections(
    wiki: &Wiki,
    dirs: &[String],
    sections: &[String],
    severity: Severity,
) -> Result<usize, WikiError> {
    let mut count = 0;
    for entry in wiki.pages().values() {
        if !WikiConfig::matches_dirs(&entry.rel_path, dirs) {
            continue;
        }
        let file_path = wiki.entry_path(entry);
        let headings = wiki.headings(&file_path)?;
        for required in sections {
            if !headings
                .iter()
                .any(|h| h.text.eq_ignore_ascii_case(required))
            {
                eprintln!(
                    "{severity}[missing-section]: {} is missing '## {required}'",
                    entry.rel_path.display(),
                );
                count += 1;
            }
        }
    }
    Ok(count)
}

fn run_required_frontmatter(
    wiki: &Wiki,
    dirs: &[String],
    fields: &[String],
    severity: Severity,
) -> Result<usize, WikiError> {
    let mut count = 0;
    for entry in wiki.pages().values() {
        if !WikiConfig::matches_dirs(&entry.rel_path, dirs) {
            continue;
        }
        let file_path = wiki.entry_path(entry);
        match wiki.frontmatter(&file_path)? {
            Ok(Some(fm)) => {
                for field in fields {
                    if !fm.has_field(field) {
                        eprintln!(
                            "{severity}[missing-frontmatter]: {} is missing '{field}'",
                            entry.rel_path.display(),
                        );
                        count += 1;
                    }
                }
            }
            Ok(None) => {
                eprintln!(
                    "{severity}[no-frontmatter]: {} has no frontmatter",
                    entry.rel_path.display(),
                );
                count += 1;
            }
            Err(e) => {
                eprintln!(
                    "{severity}[bad-frontmatter]: {}: {e}",
                    entry.rel_path.display(),
                );
                count += 1;
            }
        }
    }
    Ok(count)
}

fn run_mirror_parity(
    wiki: &Wiki,
    left: &str,
    right: &str,
    severity: Severity,
) -> Result<usize, WikiError> {
    let mut count = 0;

    let left_dir = wiki.root().path().join(left);
    let right_dir = wiki.root().path().join(right);

    let left_stems = collect_md_stems(&left_dir)?;
    let right_stems = collect_md_stems(&right_dir)?;

    for stem in &left_stems {
        if !right_stems.contains(stem) {
            eprintln!("{severity}[missing-mirror]: {left}/{stem}.md has no {right}/{stem}.md",);
            count += 1;
        }
    }
    for stem in &right_stems {
        if !left_stems.contains(stem) {
            eprintln!("{severity}[missing-mirror]: {right}/{stem}.md has no {left}/{stem}.md",);
            count += 1;
        }
    }

    Ok(count)
}

fn collect_md_stems(dir: &std::path::Path) -> Result<std::collections::HashSet<String>, WikiError> {
    let mut stems = std::collections::HashSet::new();
    if !dir.is_dir() {
        return Ok(stems);
    }
    for entry in ignore::WalkBuilder::new(dir).hidden(false).build() {
        let entry = entry.map_err(|e| WikiError::Walk {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if super::is_markdown_file(path)
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            stems.insert(stem.to_owned());
        }
    }
    Ok(stems)
}

/// Pre-loaded match directory data to avoid re-walking per citation capture.
enum MatchDirCache {
    Contents(Vec<String>),
    /// Stems stored lowercase for O(1) case-insensitive lookup.
    Stems(std::collections::HashSet<String>),
}

impl MatchDirCache {
    fn load(dir: &std::path::Path, mode: MatchMode) -> Result<Self, WikiError> {
        if !dir.is_dir() {
            return Ok(match mode {
                MatchMode::Content => Self::Contents(Vec::new()),
                MatchMode::Filename => Self::Stems(std::collections::HashSet::new()),
            });
        }
        let mut contents = Vec::new();
        let mut stems = std::collections::HashSet::new();
        for entry in ignore::WalkBuilder::new(dir).hidden(false).build() {
            let entry = entry.map_err(|e| WikiError::Walk {
                path: dir.to_path_buf(),
                source: e,
            })?;
            let path = entry.path();
            if !super::is_markdown_file(path) {
                continue;
            }
            match mode {
                MatchMode::Content => {
                    let content =
                        std::fs::read_to_string(path).map_err(|e| WikiError::ReadFile {
                            path: path.to_path_buf(),
                            source: e,
                        })?;
                    contents.push(content);
                }
                MatchMode::Filename => {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        stems.insert(stem.to_lowercase());
                    }
                }
            }
        }
        Ok(match mode {
            MatchMode::Content => Self::Contents(contents),
            MatchMode::Filename => Self::Stems(stems),
        })
    }

    fn contains(&self, needle: &str) -> bool {
        match self {
            Self::Contents(pages) => pages.iter().any(|c| c.contains(needle)),
            Self::Stems(stems) => stems.contains(&needle.to_lowercase()),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_citation_pattern(
    wiki: &Wiki,
    name: &str,
    dirs: &[String],
    pattern: &str,
    match_in: &str,
    match_mode: MatchMode,
    severity: Severity,
) -> Result<usize, WikiError> {
    let regex = regex_lite::Regex::new(pattern).expect("regex pre-validated at config load");

    let match_dir = wiki.root().path().join(match_in);
    let cache = MatchDirCache::load(&match_dir, match_mode)?;

    let mut count = 0;

    for entry in wiki.pages().values() {
        if !WikiConfig::matches_dirs(&entry.rel_path, dirs) {
            continue;
        }
        let file_path = wiki.entry_path(entry);
        let source = wiki.source(&file_path)?;

        for cap in regex.captures_iter(source) {
            let Some(id) = cap.name("id").map(|m| m.as_str()) else {
                continue;
            };

            if !cache.contains(id) {
                eprintln!(
                    "{severity}[{name}]: {} references '{id}' but no matching page in {match_in}",
                    entry.rel_path.display(),
                );
                count += 1;
            }
        }
    }

    Ok(count)
}
