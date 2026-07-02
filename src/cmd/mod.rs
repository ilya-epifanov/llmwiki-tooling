pub mod agent;
pub mod frontmatter_cmd;
pub mod init;
pub mod links;
pub mod lint;
pub mod refs;
pub mod rename;
pub mod sections;

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;

/// Edits collected during read phase for later application.
pub(crate) type FileEdits = Vec<(PathBuf, String, Vec<(Range<usize>, String)>)>;

/// Per-directory statistics from scanning markdown files.
#[derive(Default)]
pub(crate) struct DirStats {
    pub file_count: usize,
    pub frontmatter_fields: HashMap<String, usize>,
    pub section_headings: HashMap<String, usize>,
}

/// Check if two slash-separated paths share at least one path component.
pub(crate) fn share_name_component(a: &str, b: &str) -> bool {
    let a_parts: HashSet<&str> = a.split('/').collect();
    let b_parts: HashSet<&str> = b.split('/').collect();
    !a_parts.is_disjoint(&b_parts)
}

/// Detect potential mirror directory pairs based on file count and shared name components.
pub(crate) fn detect_mirror_candidates(dirs: &[(String, usize)]) -> Vec<(&str, &str, usize)> {
    let mut candidates = Vec::new();
    for i in 0..dirs.len() {
        for j in (i + 1)..dirs.len() {
            let (dir_a, count_a) = &dirs[i];
            let (dir_b, count_b) = &dirs[j];
            if count_a == count_b && *count_a > 0 && share_name_component(dir_a, dir_b) {
                candidates.push((dir_a.as_str(), dir_b.as_str(), *count_a));
            }
        }
    }
    candidates
}
