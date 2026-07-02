use std::path::Path;

use crate::config::IgnoreConfig;
use crate::error::WikiError;

/// Build the default wiki file walker.
pub(crate) fn wiki_walk_builder(
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
pub(crate) fn is_markdown_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md") && path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walked_markdown(root: &Path, ignore: &IgnoreConfig) -> Vec<String> {
        let mut files: Vec<_> = wiki_walk_builder(root, root, ignore)
            .unwrap()
            .build()
            .filter_map(Result::ok)
            .filter(|entry| is_markdown_file(entry.path()))
            .map(|entry| {
                entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        files.sort();
        files
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

        let mut files: Vec<_> = wiki_walk_builder(&dir.path().join("wiki"), dir.path(), &ignore)
            .unwrap()
            .build()
            .filter_map(Result::ok)
            .filter(|entry| is_markdown_file(entry.path()))
            .map(|entry| {
                entry
                    .path()
                    .strip_prefix(dir.path())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        files.sort();

        assert_eq!(files, ["wiki/keep.md"]);
    }
}
