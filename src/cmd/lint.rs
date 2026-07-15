use std::path::{Path, PathBuf};

use crate::config::{MatchMode, RuleConfig, RulePredicate, Severity, WikiConfig};
use crate::error::WikiError;
use crate::frontmatter::Frontmatter;
use crate::link_index::LinkIndex;
use crate::page::PageId;
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

    fn merge(&mut self, other: Self) {
        self.errors += other.errors;
        self.warnings += other.warnings;
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
    let needs_broken_links = should_show(config.checks.broken_links, filter)
        || should_show(config.checks.unmanaged_broken_links, filter);
    let needs_orphan_pages = should_show(config.checks.orphan_pages, filter);
    let link_index = (needs_broken_links || needs_orphan_pages)
        .then(|| LinkIndex::build(wiki))
        .transpose()?;

    if let Some(index) = &link_index {
        if needs_broken_links {
            result.merge(run_broken_links(wiki, filter, index)?);
        }

        if needs_orphan_pages {
            let count = run_orphan_pages(wiki, index);
            result.tally(count, config.checks.orphan_pages);
        }
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
            when,
            sections,
            severity,
            ..
        } => run_required_sections(wiki, dirs, when.as_ref(), sections, *severity),
        RuleConfig::RequiredFrontmatter {
            dirs,
            when,
            fields,
            severity,
            ..
        } => run_required_frontmatter(wiki, dirs, when.as_ref(), fields, *severity),
        RuleConfig::MirrorParity {
            left,
            right,
            severity,
            ..
        } => run_mirror_parity(wiki, left, right, *severity),
        RuleConfig::CitationPattern {
            name,
            dirs,
            when,
            pattern,
            match_in,
            match_mode,
            severity,
        } => run_citation_pattern(
            wiki,
            name,
            dirs,
            when.as_ref(),
            pattern,
            match_in,
            *match_mode,
            *severity,
        ),
    }
}

fn run_broken_links(
    wiki: &Wiki,
    filter: SeverityFilter,
    link_index: &LinkIndex,
) -> Result<LintResult, WikiError> {
    let mut result = LintResult::new();
    for broken in link_index.broken_links() {
        let severity = if wiki.is_managed_file(&broken.source_path) {
            wiki.config().checks.broken_links
        } else {
            wiki.config().checks.unmanaged_broken_links
        };
        if !should_show(severity, filter) {
            continue;
        }

        let source = wiki.file(&broken.source_path)?.source();
        let rel_path = wiki.rel_path(&broken.source_path);
        let ref_text = &source[broken.link.byte_range.clone()];
        eprintln!(
            "{severity}[broken-link]: {} in {}",
            ref_text.trim(),
            rel_path.display(),
        );
        eprintln!("  -> {}", broken.reason);
        result.tally(1, severity);
    }
    Ok(result)
}

fn run_orphan_pages(wiki: &Wiki, link_index: &LinkIndex) -> usize {
    let orphans = link_index.orphans(wiki);
    for page_id in &orphans {
        if let Some(rel_path) = wiki.get(page_id) {
            eprintln!(
                "error[orphan]: {} has no inbound internal links",
                rel_path.display(),
            );
        }
    }
    orphans.len()
}

fn run_index_coverage(wiki: &Wiki, index_path: &std::path::Path) -> Result<usize, WikiError> {
    let referenced: std::collections::HashSet<PageId> = wiki
        .file(index_path)?
        .internal_links()
        .iter()
        .filter_map(|link| wiki.resolve_internal_link(index_path, link))
        .map(|(page, _)| page.clone())
        .collect();

    let mut count = 0;
    for (page_id, rel_path) in wiki.pages() {
        if !referenced.contains(page_id) {
            eprintln!(
                "error[not-in-index]: {} is not listed in index",
                rel_path.display(),
            );
            count += 1;
        }
    }
    Ok(count)
}

struct ScopedPage<'a> {
    rel_path: &'a Path,
    file_path: PathBuf,
}

fn scoped_pages<'a>(
    wiki: &'a Wiki,
    dirs: &[String],
    when: Option<&RulePredicate>,
) -> Result<Vec<ScopedPage<'a>>, WikiError> {
    let mut pages = Vec::new();
    for rel_path in wiki.pages().values() {
        if !dirs.is_empty() && !WikiConfig::matches_dirs(rel_path, dirs) {
            continue;
        }
        let file_path = wiki.abs_path(rel_path);
        if let Some(predicate) = when
            && !matches!(wiki.file(&file_path)?.frontmatter(), Ok(Some(fm)) if predicate_matches(predicate, fm))
        {
            continue;
        }
        pages.push(ScopedPage {
            rel_path,
            file_path,
        });
    }
    Ok(pages)
}

fn predicate_matches(predicate: &RulePredicate, fm: &Frontmatter) -> bool {
    fm.get_str_list(&predicate.field)
        .iter()
        .any(|value| *value == predicate.value)
}

fn run_required_sections(
    wiki: &Wiki,
    dirs: &[String],
    when: Option<&RulePredicate>,
    sections: &[String],
    severity: Severity,
) -> Result<usize, WikiError> {
    let mut count = 0;
    for page in scoped_pages(wiki, dirs, when)? {
        let headings = wiki.file(&page.file_path)?.headings();
        for required in sections {
            if !headings
                .iter()
                .any(|h| h.text.eq_ignore_ascii_case(required))
            {
                eprintln!(
                    "{severity}[missing-section]: {} is missing '## {required}'",
                    page.rel_path.display(),
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
    when: Option<&RulePredicate>,
    fields: &[String],
    severity: Severity,
) -> Result<usize, WikiError> {
    let mut count = 0;
    for page in scoped_pages(wiki, dirs, when)? {
        match wiki.file(&page.file_path)?.frontmatter() {
            Ok(Some(fm)) => {
                for field in fields {
                    if !fm.has_field(field) {
                        eprintln!(
                            "{severity}[missing-frontmatter]: {} is missing '{field}'",
                            page.rel_path.display(),
                        );
                        count += 1;
                    }
                }
            }
            Ok(None) => {
                eprintln!(
                    "{severity}[no-frontmatter]: {} has no frontmatter",
                    page.rel_path.display(),
                );
                count += 1;
            }
            Err(e) => {
                eprintln!(
                    "{severity}[bad-frontmatter]: {}: {e}",
                    page.rel_path.display(),
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

    let left_stems = collect_md_stems(wiki, &left_dir)?;
    let right_stems = collect_md_stems(wiki, &right_dir)?;

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

fn collect_md_stems(
    wiki: &Wiki,
    dir: &Path,
) -> Result<std::collections::HashSet<String>, WikiError> {
    let mut stems = std::collections::HashSet::new();
    if !dir.is_dir() {
        return Ok(stems);
    }
    for path in wiki
        .scannable_files()
        .iter()
        .filter(|path| path.starts_with(dir))
    {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
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
    fn load(wiki: &Wiki, dir: &Path, mode: MatchMode) -> Result<Self, WikiError> {
        if !dir.is_dir() {
            return Ok(match mode {
                MatchMode::Content => Self::Contents(Vec::new()),
                MatchMode::Filename => Self::Stems(std::collections::HashSet::new()),
            });
        }
        let mut contents = Vec::new();
        let mut stems = std::collections::HashSet::new();
        for path in wiki
            .scannable_files()
            .iter()
            .filter(|path| path.starts_with(dir))
        {
            match mode {
                MatchMode::Content => contents.push(wiki.file(path)?.source().to_owned()),
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
    when: Option<&RulePredicate>,
    pattern: &str,
    match_in: &str,
    match_mode: MatchMode,
    severity: Severity,
) -> Result<usize, WikiError> {
    let regex = regex_lite::Regex::new(pattern).expect("regex pre-validated at config load");

    let match_dir = wiki.root().path().join(match_in);
    let cache = MatchDirCache::load(wiki, &match_dir, match_mode)?;

    let mut count = 0;

    for page in scoped_pages(wiki, dirs, when)? {
        let source = wiki.file(&page.file_path)?.source();

        for cap in regex.captures_iter(source) {
            let Some(id) = cap.name("id").map(|m| m.as_str()) else {
                continue;
            };

            if !cache.contains(id) {
                eprintln!(
                    "{severity}[{name}]: {} references '{id}' but no matching page in {match_in}",
                    page.rel_path.display(),
                );
                count += 1;
            }
        }
    }

    Ok(count)
}
