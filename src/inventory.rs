use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::config::IgnoreConfig;
use crate::error::WikiError;
use crate::markdown_document::MarkdownDocument;

const INDEX_CANDIDATES: &[&str] = &["index.md", "README.md", "_index.md"];

/// Scannable Markdown files with sources parsed as Markdown documents.
pub(crate) struct MarkdownFileSet {
    files: Vec<MarkdownFile>,
}

pub(crate) struct MarkdownFile {
    pub path: PathBuf,
    pub rel_path: PathBuf,
    pub document: MarkdownDocument,
}

impl MarkdownFileSet {
    pub(crate) fn build(root: &Path, ignore: &IgnoreConfig) -> Result<Self, WikiError> {
        Self::build_under(root, root, ignore)
    }

    pub(crate) fn build_under(
        root: &Path,
        start: &Path,
        ignore: &IgnoreConfig,
    ) -> Result<Self, WikiError> {
        let mut files = Vec::new();
        for path in markdown_files(start, root, ignore)? {
            let source = std::fs::read_to_string(&path).map_err(|e| WikiError::ReadFile {
                path: path.clone(),
                source: e,
            })?;
            files.push(MarkdownFile {
                rel_path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                path,
                document: MarkdownDocument::new(source),
            });
        }
        Ok(Self { files })
    }

    pub(crate) fn files(&self) -> &[MarkdownFile] {
        &self.files
    }

    pub(crate) fn into_files(self) -> Vec<MarkdownFile> {
        self.files
    }
}

/// Return scannable markdown files under `start`.
fn markdown_files(
    start: &Path,
    wiki_root: &Path,
    ignore: &IgnoreConfig,
) -> Result<Vec<PathBuf>, WikiError> {
    let mut files = Vec::new();
    for entry in wiki_walk_builder(start, wiki_root, ignore)?.build() {
        let entry = entry.map_err(|e| WikiError::Walk {
            path: start.to_path_buf(),
            source: e,
        })?;
        if is_markdown_file(entry.path()) {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort_by(|a, b| {
        a.strip_prefix(wiki_root)
            .unwrap_or(a)
            .cmp(b.strip_prefix(wiki_root).unwrap_or(b))
    });
    Ok(files)
}

fn wiki_walk_builder(
    start: &Path,
    wiki_root: &Path,
    ignore: &IgnoreConfig,
) -> Result<ignore::WalkBuilder, WikiError> {
    let matcher = ignore_matcher(wiki_root, ignore)?;
    let mut builder = ignore::WalkBuilder::new(start);
    // DECISION: wiki folders are often copied without .git; still honor their .gitignore.
    builder
        .hidden(false)
        .require_git(false)
        .filter_entry(move |entry| !is_ignored(entry, &matcher));
    Ok(builder)
}

fn ignore_matcher(
    wiki_root: &Path,
    ignore: &IgnoreConfig,
) -> Result<ignore::gitignore::Gitignore, WikiError> {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(wiki_root);
    for pattern in ignore.effective_patterns() {
        builder
            .add_line(None, pattern)
            .map_err(|source| WikiError::InvalidIgnorePattern {
                pattern: pattern.to_owned(),
                source,
            })?;
    }
    builder
        .build()
        .map_err(|source| WikiError::CompileIgnorePatterns { source })
}

fn is_ignored(entry: &ignore::DirEntry, matcher: &ignore::gitignore::Gitignore) -> bool {
    entry.depth() > 0
        && matcher
            .matched(
                entry.path(),
                entry
                    .file_type()
                    .is_some_and(|file_type| file_type.is_dir()),
            )
            .is_ignore()
}

/// Check if a path is a markdown file.
fn is_markdown_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md") && path.is_file()
}

/// Inventory of the wiki's markdown shape.
#[derive(Debug, Clone)]
pub(crate) struct WikiInventory {
    directories: Vec<DirectoryInventory>,
    mirror_candidates: Vec<MirrorCandidate>,
    index: Option<IndexInventory>,
}

/// Inventory for one exact containing directory.
#[derive(Debug, Clone)]
pub(crate) struct DirectoryInventory {
    pub path: String,
    pub file_count: usize,
    pub frontmatter_fields: BTreeMap<String, usize>,
    pub section_headings: BTreeMap<String, usize>,
    stems: BTreeSet<String>,
}

/// Pair of exact directories with the same non-empty markdown filename stems.
#[derive(Debug, Clone)]
pub(crate) struct MirrorCandidate {
    pub left: String,
    pub right: String,
    pub file_count: usize,
}

/// Index-like markdown file and its unique wikilink target count.
#[derive(Debug, Clone)]
pub(crate) struct IndexInventory {
    pub path: String,
    pub unique_refs: usize,
}

impl WikiInventory {
    pub(crate) fn build(root: &Path, ignore: &IgnoreConfig) -> Result<Self, WikiError> {
        let file_set = MarkdownFileSet::build(root, ignore)?;
        let mut directories: BTreeMap<String, DirectoryInventory> = BTreeMap::new();
        let mut index_counts = BTreeMap::new();

        for file in file_set.files() {
            let rel_path_str = file.rel_path.to_string_lossy().replace('\\', "/");

            if INDEX_CANDIDATES.contains(&rel_path_str.as_str()) {
                let unique_refs: BTreeSet<_> = file
                    .document
                    .internal_links()
                    .iter()
                    .map(|link| &link.target)
                    .collect();
                index_counts.insert(rel_path_str.clone(), unique_refs.len());
            }

            let dir = directory_key(&file.rel_path);
            let inventory = directories
                .entry(dir.clone())
                .or_insert_with(|| DirectoryInventory::new(dir));
            inventory.file_count += 1;

            if let Some(stem) = file.path.file_stem().and_then(|stem| stem.to_str()) {
                inventory.stems.insert(stem.to_owned());
            }

            if let Ok(Some(fm)) = file.document.frontmatter()
                && let serde_yml::Value::Mapping(map) = fm.data()
            {
                for key in map.keys() {
                    *inventory
                        .frontmatter_fields
                        .entry(key.to_owned())
                        .or_insert(0) += 1;
                }
            }

            for heading in file.document.headings() {
                if heading.level == 2 {
                    *inventory
                        .section_headings
                        .entry(heading.text.clone())
                        .or_insert(0) += 1;
                }
            }
        }

        let directories: Vec<_> = directories.into_values().collect();
        let mirror_candidates = mirror_candidates(&directories);
        let index = INDEX_CANDIDATES.iter().find_map(|candidate| {
            index_counts
                .get(*candidate)
                .map(|unique_refs| IndexInventory {
                    path: (*candidate).to_owned(),
                    unique_refs: *unique_refs,
                })
        });

        Ok(Self {
            directories,
            mirror_candidates,
            index,
        })
    }

    pub(crate) fn directories(&self) -> &[DirectoryInventory] {
        &self.directories
    }

    pub(crate) fn mirror_candidates(&self) -> &[MirrorCandidate] {
        &self.mirror_candidates
    }

    pub(crate) fn index(&self) -> Option<&IndexInventory> {
        self.index.as_ref()
    }
}

impl DirectoryInventory {
    fn new(path: String) -> Self {
        Self {
            path,
            file_count: 0,
            frontmatter_fields: BTreeMap::new(),
            section_headings: BTreeMap::new(),
            stems: BTreeSet::new(),
        }
    }
}

fn directory_key(rel_path: &Path) -> String {
    rel_path
        .parent()
        .and_then(Path::to_str)
        .filter(|path| !path.is_empty())
        .unwrap_or(".")
        .replace('\\', "/")
}

fn mirror_candidates(directories: &[DirectoryInventory]) -> Vec<MirrorCandidate> {
    let mut candidates = Vec::new();
    for i in 0..directories.len() {
        for right in &directories[i + 1..] {
            let left = &directories[i];
            if !left.stems.is_empty() && left.stems == right.stems {
                candidates.push(MirrorCandidate {
                    left: left.path.clone(),
                    right: right.path.clone(),
                    file_count: left.stems.len(),
                });
            }
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walked_markdown(root: &Path, ignore: &IgnoreConfig) -> Vec<String> {
        MarkdownFileSet::build(root, ignore)
            .unwrap()
            .files()
            .iter()
            .map(|file| {
                file.path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn skips_gitignored_and_default_agent_tool_dirs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.md"), "# Keep\n").unwrap();
        std::fs::write(dir.path().join("ignored.md"), "# Ignored\n").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "ignored.md\n").unwrap();

        for pattern in IgnoreConfig::default().effective_patterns() {
            let name = pattern.trim_end_matches('/');
            let path = dir.path().join(name);
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("skip.md"), "# Skip\n").unwrap();
        }

        assert_eq!(
            walked_markdown(dir.path(), &IgnoreConfig::default()),
            ["keep.md"]
        );
    }

    #[test]
    fn custom_patterns_are_additive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.md"), "# Keep\n").unwrap();
        std::fs::create_dir(dir.path().join("generated")).unwrap();
        std::fs::write(dir.path().join("generated/skip.md"), "# Skip\n").unwrap();
        std::fs::create_dir(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude/skip.md"), "# Skip\n").unwrap();

        let ignore = IgnoreConfig {
            default_patterns: true,
            patterns: vec!["generated/".to_owned()],
        };

        assert_eq!(walked_markdown(dir.path(), &ignore), ["keep.md"]);
    }

    #[test]
    fn default_patterns_can_be_discarded() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude/keep.md"), "# Keep\n").unwrap();
        std::fs::create_dir(dir.path().join("generated")).unwrap();
        std::fs::write(dir.path().join("generated/skip.md"), "# Skip\n").unwrap();

        let ignore = IgnoreConfig {
            default_patterns: false,
            patterns: vec!["generated/".to_owned()],
        };

        assert_eq!(walked_markdown(dir.path(), &ignore), [".claude/keep.md"]);
    }

    #[test]
    fn patterns_are_matched_from_wiki_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("wiki/generated")).unwrap();
        std::fs::write(dir.path().join("wiki/generated/skip.md"), "# Skip\n").unwrap();
        std::fs::write(dir.path().join("wiki/keep.md"), "# Keep\n").unwrap();

        let ignore = IgnoreConfig {
            default_patterns: false,
            patterns: vec!["wiki/generated/".to_owned()],
        };

        let files: Vec<_> =
            MarkdownFileSet::build_under(dir.path(), &dir.path().join("wiki"), &ignore)
                .unwrap()
                .files()
                .iter()
                .map(|file| file.rel_path.to_string_lossy().replace('\\', "/"))
                .collect();

        assert_eq!(files, ["wiki/keep.md"]);
    }

    #[test]
    fn inventories_unmanaged_markdown_and_excludes_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("wiki")).unwrap();
        std::fs::create_dir(dir.path().join("ignored")).unwrap();
        std::fs::write(dir.path().join("loose.md"), "# Loose\n").unwrap();
        std::fs::write(
            dir.path().join("wiki/Managed.md"),
            "---\ntype: concept\n---\n\n# Managed\n\n## Notes\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("ignored/Skip.md"), "# Skip\n").unwrap();

        let ignore = IgnoreConfig {
            default_patterns: false,
            patterns: vec!["ignored/".to_owned()],
        };
        let inventory = WikiInventory::build(dir.path(), &ignore).unwrap();
        let dirs: Vec<_> = inventory
            .directories()
            .iter()
            .map(|dir| dir.path.as_str())
            .collect();

        assert_eq!(dirs, [".", "wiki"]);
        let wiki = inventory
            .directories()
            .iter()
            .find(|dir| dir.path == "wiki")
            .unwrap();
        assert_eq!(wiki.file_count, 1);
        assert_eq!(wiki.frontmatter_fields.get("type"), Some(&1));
        assert_eq!(wiki.section_headings.get("Notes"), Some(&1));
    }

    #[test]
    fn mirror_candidates_require_matching_stems() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("wiki/papers")).unwrap();
        std::fs::create_dir_all(dir.path().join("raw/papers")).unwrap();
        std::fs::create_dir_all(dir.path().join("other")).unwrap();
        std::fs::write(dir.path().join("index.md"), "# Index\n").unwrap();
        for name in ["A.md", "B.md"] {
            std::fs::write(dir.path().join("wiki/papers").join(name), "# Page\n").unwrap();
            std::fs::write(dir.path().join("raw/papers").join(name), "# Raw\n").unwrap();
        }
        std::fs::write(dir.path().join("other/A.md"), "# Other\n").unwrap();
        std::fs::write(dir.path().join("other/C.md"), "# Other\n").unwrap();

        let inventory = WikiInventory::build(dir.path(), &IgnoreConfig::default()).unwrap();

        assert!(inventory.mirror_candidates().iter().any(|candidate| {
            candidate.left == "raw/papers"
                && candidate.right == "wiki/papers"
                && candidate.file_count == 2
        }));
        assert!(
            !inventory
                .mirror_candidates()
                .iter()
                .any(|candidate| candidate.left == "other" || candidate.right == "other")
        );
    }

    #[test]
    fn index_uses_first_scannable_candidate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("wiki")).unwrap();
        std::fs::write(dir.path().join("index.md"), "[[A]] [[A]] [[B]]\n").unwrap();
        std::fs::write(dir.path().join("README.md"), "[[C]]\n").unwrap();
        std::fs::write(dir.path().join("wiki/A.md"), "# A\n").unwrap();

        let inventory = WikiInventory::build(dir.path(), &IgnoreConfig::default()).unwrap();
        let index = inventory.index().unwrap();

        assert_eq!(index.path, "index.md");
        assert_eq!(index.unique_refs, 2);
    }
}
