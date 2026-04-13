use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;

use elsa::FrozenMap;
use once_cell::unsync::OnceCell as OnceCellTry;

use crate::cmd::is_markdown_file;
use crate::config::WikiConfig;
use crate::error::{FrontmatterError, WikiError};
use crate::frontmatter::{self, Frontmatter};
use crate::page::{BlockId, Heading, PageId, WikilinkOccurrence};
use crate::parse::{self, ClassifiedRange};

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
                return Ok(Self(dir));
            }
            if dir.join("index.md").is_file() && dir.join("wiki").is_dir() {
                return Ok(Self(dir));
            }
            if dir.join("index.md").is_file() {
                return Ok(Self(dir));
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
            Ok(Self(path))
        } else {
            Err(WikiError::RootNotFound { start: path })
        }
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// A single page's catalog entry.
#[derive(Debug, Clone)]
pub struct PageEntry {
    pub rel_path: PathBuf,
}

/// Cached file with lazy-parsed components.
pub struct CachedFile {
    source: String,
    frontmatter: OnceCell<Result<Option<Frontmatter>, FrontmatterError>>,
    headings: OnceCell<Vec<Heading>>,
    wikilinks: OnceCell<Vec<WikilinkOccurrence>>,
    classified_ranges: OnceCell<Vec<ClassifiedRange>>,
    block_ids: OnceCell<Vec<BlockId>>,
}

impl CachedFile {
    fn new(source: String) -> Self {
        Self {
            source,
            frontmatter: OnceCell::new(),
            headings: OnceCell::new(),
            wikilinks: OnceCell::new(),
            classified_ranges: OnceCell::new(),
            block_ids: OnceCell::new(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn frontmatter(&self) -> &Result<Option<Frontmatter>, FrontmatterError> {
        self.frontmatter
            .get_or_init(|| frontmatter::parse_frontmatter(&self.source))
    }

    pub fn headings(&self) -> &[Heading] {
        self.headings
            .get_or_init(|| parse::extract_headings(&self.source))
    }

    pub fn wikilinks(&self) -> &[WikilinkOccurrence] {
        self.wikilinks
            .get_or_init(|| parse::extract_wikilinks(&self.source))
    }

    pub fn classified_ranges(&self) -> &[ClassifiedRange] {
        self.classified_ranges
            .get_or_init(|| parse::classify_ranges(&self.source))
    }

    pub fn block_ids(&self) -> &[BlockId] {
        self.block_ids
            .get_or_init(|| parse::extract_block_ids(&self.source))
    }
}

/// Unified wiki structure with lazy-loaded content caching.
pub struct Wiki {
    root: WikiRoot,
    config: WikiConfig,
    pages: HashMap<PageId, PageEntry>,
    autolink_candidates: HashSet<PageId>,
    autolink_pages: OnceCellTry<HashSet<PageId>>,
    content: FrozenMap<PathBuf, Box<CachedFile>>,
}

impl Wiki {
    /// Build wiki from root — discovers paths only, no file reads.
    pub fn build(root: WikiRoot, config: WikiConfig) -> Result<Self, WikiError> {
        let (pages, autolink_candidates) = Self::discover_pages(&root, &config)?;

        Ok(Self {
            root,
            config,
            pages,
            autolink_candidates,
            autolink_pages: OnceCellTry::new(),
            content: FrozenMap::new(),
        })
    }

    fn discover_pages(
        root: &WikiRoot,
        config: &WikiConfig,
    ) -> Result<(HashMap<PageId, PageEntry>, HashSet<PageId>), WikiError> {
        let mut pages: HashMap<PageId, PageEntry> = HashMap::new();
        let mut autolink_candidates = HashSet::new();

        for dir_config in &config.directories {
            let dir_path = root.path().join(&dir_config.path);
            if !dir_path.is_dir() {
                continue;
            }

            for entry in ignore::WalkBuilder::new(&dir_path).hidden(false).build() {
                let entry = entry.map_err(|e| WikiError::Walk {
                    path: dir_path.clone(),
                    source: e,
                })?;
                let path = entry.path();
                if !is_markdown_file(path) {
                    continue;
                }
                let Some(page_id) = PageId::from_path(path) else {
                    continue;
                };
                let rel_path = path.strip_prefix(root.path()).unwrap_or(path).to_path_buf();

                // Skip index file
                if let Some(index) = &config.index
                    && rel_path.to_str().is_some_and(|s| s == index)
                {
                    continue;
                }

                // Skip files owned by a more-specific directory config
                let owning_dir = config.directory_for(&rel_path);
                if owning_dir.map(|d| d.path.as_str()) != Some(dir_config.path.as_str()) {
                    continue;
                }

                // Check for duplicate page IDs
                if let Some(existing) = pages.get(&page_id) {
                    return Err(WikiError::DuplicatePageId {
                        id: page_id.to_string(),
                        path1: existing.rel_path.clone(),
                        path2: rel_path,
                    });
                }

                if dir_config.autolink {
                    autolink_candidates.insert(page_id.clone());
                }

                pages.insert(page_id, PageEntry { rel_path });
            }
        }

        Ok((pages, autolink_candidates))
    }

    pub fn root(&self) -> &WikiRoot {
        &self.root
    }

    pub fn config(&self) -> &WikiConfig {
        &self.config
    }

    pub fn pages(&self) -> &HashMap<PageId, PageEntry> {
        &self.pages
    }

    pub fn get(&self, id: &PageId) -> Option<&PageEntry> {
        self.pages.get(id)
    }

    pub fn contains(&self, id: &PageId) -> bool {
        self.pages.contains_key(id)
    }

    /// Find a page by name. Always O(1) since PageIds are normalized to lowercase.
    pub fn find(&self, name: &str) -> Option<(&PageId, &PageEntry)> {
        let id = PageId::from(name);
        self.pages.get_key_value(&id)
    }

    /// Get the display name for a page (original filename case from rel_path).
    pub fn display_name(&self, id: &PageId) -> Option<&str> {
        self.pages
            .get(id)
            .and_then(|e| e.rel_path.file_stem())
            .and_then(|s| s.to_str())
    }

    pub fn index_path(&self) -> Option<PathBuf> {
        self.config.index.as_ref().map(|idx| self.root.path().join(idx))
    }

    /// Get the absolute path for a page entry.
    pub fn entry_path(&self, entry: &PageEntry) -> PathBuf {
        self.root.path().join(&entry.rel_path)
    }

    /// Convert an absolute path to a path relative to the wiki root.
    pub fn rel_path<'a>(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(self.root.path()).unwrap_or(path)
    }

    /// All wiki page files that should be scanned for wikilink content.
    pub fn all_scannable_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = self
            .pages
            .values()
            .map(|entry| self.root.path().join(&entry.rel_path))
            .collect();
        if let Some(index_path) = self.index_path()
            && index_path.is_file()
        {
            files.push(index_path);
        }
        files
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
            if let Some(entry) = self.pages.get(page_id) {
                let file_path = self.entry_path(entry);
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

    /// Get cached file, loading on first access.
    pub fn file(&self, path: &Path) -> Result<&CachedFile, WikiError> {
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
            .insert(abs_path, Box::new(CachedFile::new(source))))
    }

    /// Get source content for a file.
    pub fn source(&self, path: &Path) -> Result<&str, WikiError> {
        Ok(self.file(path)?.source())
    }

    /// Get frontmatter for a file.
    pub fn frontmatter(
        &self,
        path: &Path,
    ) -> Result<&Result<Option<Frontmatter>, FrontmatterError>, WikiError> {
        Ok(self.file(path)?.frontmatter())
    }

    /// Get headings for a file.
    pub fn headings(&self, path: &Path) -> Result<&[Heading], WikiError> {
        Ok(self.file(path)?.headings())
    }

    /// Get wikilinks for a file.
    pub fn wikilinks(&self, path: &Path) -> Result<&[WikilinkOccurrence], WikiError> {
        Ok(self.file(path)?.wikilinks())
    }

    /// Get classified ranges for a file.
    pub fn classified_ranges(&self, path: &Path) -> Result<&[ClassifiedRange], WikiError> {
        Ok(self.file(path)?.classified_ranges())
    }

    /// Get block IDs for a file.
    pub fn block_ids(&self, path: &Path) -> Result<&[BlockId], WikiError> {
        Ok(self.file(path)?.block_ids())
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
