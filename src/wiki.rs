use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;

use elsa::FrozenMap;
use once_cell::unsync::OnceCell as OnceCellTry;

use crate::config::WikiConfig;
use crate::error::WikiError;
use crate::inventory::{MarkdownFile, MarkdownFileSet};
use crate::markdown_document::MarkdownDocument;
use crate::markdown_links;
use crate::page::{InternalLinkOccurrence, InternalLinkTarget, PageId};

/// Validated wiki root directory.
#[derive(Debug, Clone)]
pub struct WikiRoot(PathBuf);

impl WikiRoot {
    /// Walk ancestors of `start` looking for the wiki root.
    /// Tries `wiki.toml` first, then `index.md` + `wiki/`, then `index.md` alone.
    pub fn discover(start: &Path) -> Result<Self, WikiError> {
        let mut dir = if start.is_file() {
            start.parent().unwrap_or(start).to_path_buf()
        } else {
            start.to_path_buf()
        };
        loop {
            if dir.join("wiki.toml").is_file() {
                return Self::new(dir);
            }
            if dir.join("index.md").is_file() && dir.join("wiki").is_dir() {
                return Self::new(dir);
            }
            if dir.join("index.md").is_file() {
                return Self::new(dir);
            }
            if !dir.pop() {
                return Err(WikiError::RootNotFound {
                    start: start.to_path_buf(),
                });
            }
        }
    }

    pub fn from_path(path: PathBuf) -> Result<Self, WikiError> {
        if path.join("wiki.toml").is_file()
            || path.join("index.md").is_file()
            || path.join("wiki").is_dir()
        {
            Self::new(path)
        } else {
            Err(WikiError::RootNotFound { start: path })
        }
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    fn new(path: PathBuf) -> Result<Self, WikiError> {
        path.canonicalize()
            .map(Self)
            .map_err(|_| WikiError::RootNotFound { start: path })
    }
}

type TargetMap = HashMap<PageId, PathBuf>;
type AliasMap = HashMap<PageId, PageId>;

fn frontmatter_aliases(path: &Path, document: &MarkdownDocument) -> Result<Vec<String>, WikiError> {
    let Some(fm) = document
        .frontmatter()
        .map_err(|source| WikiError::Frontmatter {
            path: path.to_path_buf(),
            source,
        })?
    else {
        return Ok(Vec::new());
    };
    Ok(fm
        .get_str_list("aliases")
        .into_iter()
        .map(str::to_owned)
        .collect())
}

/// Unified wiki structure with lazy-loaded content caching.
pub struct Wiki {
    root: WikiRoot,
    config: WikiConfig,
    pages: HashMap<PageId, PathBuf>,
    targets: TargetMap,
    path_ids: HashMap<PathBuf, PageId>,
    aliases: AliasMap,
    scannable_files: Vec<PathBuf>,
    autolink_candidates: HashSet<PageId>,
    autolink_pages: OnceCellTry<HashSet<PageId>>,
    content: FrozenMap<PathBuf, Box<MarkdownDocument>>,
}

impl Wiki {
    /// Build wiki indexes from root.
    pub fn build(root: WikiRoot, config: WikiConfig) -> Result<Self, WikiError> {
        let file_set = MarkdownFileSet::build(root.path(), &config.ignore)?;
        let scannable_files = file_set
            .files()
            .iter()
            .map(|file| file.path.clone())
            .collect();
        let content = FrozenMap::new();
        let mut pages: HashMap<PageId, PathBuf> = HashMap::new();
        let mut targets: TargetMap = HashMap::new();
        let mut aliases_to_index = Vec::new();
        let mut autolink_candidates = HashSet::new();

        for MarkdownFile {
            path,
            rel_path,
            document,
        } in file_set.into_files()
        {
            let Some(page_id) = PageId::from_path(&path) else {
                content.insert(path, Box::new(document));
                continue;
            };

            if config.index.as_deref() != rel_path.to_str()
                && let Some(dir_config) = config.directory_for(&rel_path)
            {
                if let Some(existing) = pages.get(&page_id) {
                    return Err(WikiError::DuplicatePageId {
                        id: page_id.to_string(),
                        path1: existing.clone(),
                        path2: rel_path.clone(),
                    });
                }
                if dir_config.autolink {
                    autolink_candidates.insert(page_id.clone());
                }
                pages.insert(page_id.clone(), rel_path.clone());
            }

            if let Some(existing) = targets.insert(page_id.clone(), rel_path.clone()) {
                return Err(WikiError::DuplicatePageName {
                    name: page_id.to_string(),
                    path1: existing,
                    path2: rel_path,
                });
            }

            let aliases = frontmatter_aliases(&path, &document)?;
            if !aliases.is_empty() {
                aliases_to_index.push((page_id, rel_path, aliases));
            }
            content.insert(path, Box::new(document));
        }

        let mut aliases = HashMap::new();
        for (page_id, rel_path, alias_names) in aliases_to_index {
            for alias in alias_names {
                let alias_id = PageId::from(alias.as_str());
                if alias_id.as_str().is_empty() || alias_id == page_id {
                    continue;
                }
                if let Some(existing) = targets.get(&alias_id) {
                    return Err(WikiError::DuplicatePageName {
                        name: alias_id.to_string(),
                        path1: existing.clone(),
                        path2: rel_path.clone(),
                    });
                }
                if let Some(existing_id) = aliases.insert(alias_id.clone(), page_id.clone())
                    && existing_id != page_id
                {
                    let existing = targets.get(&existing_id).expect("alias target exists");
                    return Err(WikiError::DuplicatePageName {
                        name: alias_id.to_string(),
                        path1: existing.clone(),
                        path2: rel_path.clone(),
                    });
                }
            }
        }

        let path_ids = targets
            .iter()
            .map(|(page_id, path)| {
                (
                    markdown_links::normalize_path(path.clone()),
                    page_id.clone(),
                )
            })
            .collect();

        Ok(Self {
            root,
            config,
            pages,
            targets,
            path_ids,
            aliases,
            scannable_files,
            autolink_candidates,
            autolink_pages: OnceCellTry::new(),
            content,
        })
    }

    pub fn root(&self) -> &WikiRoot {
        &self.root
    }

    pub fn config(&self) -> &WikiConfig {
        &self.config
    }

    pub fn pages(&self) -> &HashMap<PageId, PathBuf> {
        &self.pages
    }

    pub fn get(&self, id: &PageId) -> Option<&PathBuf> {
        self.pages.get(id)
    }

    pub fn canonical_id(&self, id: &PageId) -> Option<&PageId> {
        self.targets
            .get_key_value(id)
            .map(|(canonical, _)| canonical)
            .or_else(|| self.aliases.get(id))
    }

    /// Find a page target by filename stem or Obsidian alias.
    pub fn find(&self, name: &str) -> Option<(&PageId, &PathBuf)> {
        let id = PageId::from(name);
        let canonical = self.canonical_id(&id)?;
        Some((canonical, self.targets.get(canonical)?))
    }

    pub fn resolve_internal_link(
        &self,
        source_path: &Path,
        link: &InternalLinkOccurrence,
    ) -> Option<(&PageId, &PathBuf)> {
        match &link.target {
            InternalLinkTarget::PageName(page) => {
                let canonical = self.canonical_id(page)?;
                Some((canonical, self.targets.get(canonical)?))
            }
            InternalLinkTarget::Path(path) => {
                let source = self.rel_path(source_path);
                let target = if path.is_empty() {
                    source.to_path_buf()
                } else {
                    let decoded = markdown_links::decode_url_path(path)?;
                    if Path::new(&decoded).is_absolute() {
                        return None;
                    }
                    markdown_links::normalize_path(
                        source
                            .parent()
                            .unwrap_or_else(|| Path::new(""))
                            .join(decoded),
                    )
                };
                let page = self.path_ids.get(&target)?;
                Some((page, self.targets.get(page)?))
            }
        }
    }

    /// Get the display name for a page target (original filename case from rel_path).
    pub fn display_name(&self, id: &PageId) -> Option<&str> {
        let canonical = self.canonical_id(id)?;
        self.targets
            .get(canonical)
            .and_then(|rel_path| rel_path.file_stem())
            .and_then(|s| s.to_str())
    }

    pub fn index_path(&self) -> Option<PathBuf> {
        self.config
            .index
            .as_ref()
            .map(|idx| self.root.path().join(idx))
    }

    /// Convert an absolute path to a path relative to the wiki root.
    pub fn rel_path<'a>(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(self.root.path()).unwrap_or(path)
    }

    /// All markdown files that should be scanned for wikilink content.
    pub fn scannable_files(&self) -> &[PathBuf] {
        &self.scannable_files
    }

    pub fn is_managed_file(&self, path: &Path) -> bool {
        let rel_path = self.rel_path(path);
        PageId::from_path(path)
            .and_then(|id| self.pages.get(&id))
            .is_some_and(|managed_path| managed_path == rel_path)
    }

    /// Autolink pages — lazily computed on first access.
    pub fn autolink_pages(&self) -> Result<&HashSet<PageId>, WikiError> {
        self.autolink_pages
            .get_or_try_init(|| self.compute_autolink_pages())
    }

    fn compute_autolink_pages(&self) -> Result<HashSet<PageId>, WikiError> {
        let mut result = HashSet::new();
        for page_id in &self.autolink_candidates {
            if self.config.linking.exclude.contains(page_id.as_str()) {
                continue;
            }
            if let Some(rel_path) = self.pages.get(page_id) {
                let file_path = self.abs_path(rel_path);
                let cached = self.file(&file_path)?;
                if let Ok(Some(fm)) = cached.frontmatter()
                    && let Some(val) = fm.get(&self.config.linking.autolink_field)
                    && val == &serde_yml::Value::Bool(false)
                {
                    continue;
                }
            }
            result.insert(page_id.clone());
        }
        Ok(result)
    }

    pub fn abs_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.path().join(path)
        }
    }

    /// Get cached Markdown document, loading on first access.
    pub fn file(&self, path: &Path) -> Result<&MarkdownDocument, WikiError> {
        let abs_path = self.abs_path(path);

        if let Some(cached) = self.content.get(&abs_path) {
            return Ok(cached);
        }

        let source = std::fs::read_to_string(&abs_path).map_err(|e| WikiError::ReadFile {
            path: abs_path.clone(),
            source: e,
        })?;

        Ok(self
            .content
            .insert(abs_path, Box::new(MarkdownDocument::new(source))))
    }

    /// Write file content. Takes `&mut self` to ensure no outstanding borrows.
    pub fn write_file(&mut self, path: &Path, content: &str) -> Result<(), WikiError> {
        let abs_path = self.abs_path(path);
        std::fs::write(&abs_path, content).map_err(|e| WikiError::WriteFile {
            path: abs_path,
            source: e,
        })
    }

    /// Rename a file. Takes `&mut self` to ensure no outstanding borrows.
    pub fn rename_file(&mut self, old: &Path, new: &Path) -> Result<(), WikiError> {
        let old_abs = self.abs_path(old);
        let new_abs = self.abs_path(new);
        std::fs::rename(&old_abs, &new_abs).map_err(|e| WikiError::WriteFile {
            path: new_abs,
            source: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_roots_are_canonicalized_before_building_paths() {
        let cwd = std::env::current_dir().unwrap();
        let dir = tempfile::Builder::new()
            .prefix(".wiki-root-")
            .tempdir_in(&cwd)
            .unwrap();
        std::fs::create_dir(dir.path().join("wiki")).unwrap();
        std::fs::write(dir.path().join("index.md"), "# Index\n").unwrap();
        std::fs::write(dir.path().join("wiki/Foo.md"), "# Foo\n").unwrap();

        let relative_root = PathBuf::from(dir.path().file_name().unwrap());
        let root = WikiRoot::from_path(relative_root).unwrap();

        assert!(root.path().is_absolute());

        let config = WikiConfig::auto_detect(root.path());
        let wiki = Wiki::build(root, config).unwrap();
        let foo = wiki
            .scannable_files()
            .iter()
            .find(|path| path.ends_with("Foo.md"))
            .unwrap();

        assert!(foo.is_absolute());
        assert_eq!(wiki.file(foo).unwrap().source(), "# Foo\n");
    }

    #[test]
    fn aliases_conflict_with_later_targets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("wiki")).unwrap();
        std::fs::write(dir.path().join("index.md"), "# Index\n").unwrap();
        std::fs::write(dir.path().join("wiki/A.md"), "---\naliases: [B]\n---\n").unwrap();
        std::fs::write(dir.path().join("wiki/B.md"), "# B\n").unwrap();

        let root = WikiRoot::from_path(dir.path().to_path_buf()).unwrap();
        let config = WikiConfig::auto_detect(root.path());
        let err = match Wiki::build(root, config) {
            Ok(_) => panic!("expected alias conflict"),
            Err(err) => err,
        };

        assert!(matches!(err, WikiError::DuplicatePageName { name, .. } if name == "b"));
    }
}
