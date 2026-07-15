use std::ops::Range;

/// Apply non-overlapping byte-range replacements to a source string.
///
/// Replacements are applied back-to-front (highest offset first) so that
/// earlier byte offsets remain valid after later replacements.
pub fn apply(source: &str, edits: &[(Range<usize>, String)]) -> String {
    let mut sorted: Vec<_> = edits.iter().collect();
    sorted.sort_by_key(|edit| std::cmp::Reverse(edit.0.start));

    let mut result = source.to_owned();
    for (range, replacement) in sorted {
        result.replace_range(range.clone(), replacement);
    }
    result
}

/// Compute a unified-diff-style display of planned edits.
///
/// Each edit is shown with its surrounding context (the line containing the edit).
pub fn diff(source: &str, path: &std::path::Path, edits: &[(Range<usize>, String)]) -> String {
    if edits.is_empty() {
        return String::new();
    }

    let line_offsets = compute_line_offsets(source);
    let mut sorted: Vec<_> = edits.iter().collect();
    sorted.sort_by_key(|(range, _)| range.start);

    let mut output = format!("--- {}\n+++ {}\n", path.display(), path.display());

    for (range, replacement) in &sorted {
        let line_num = offset_to_line(&line_offsets, range.start);
        let line_start = line_offsets[line_num];
        let end_offset = if range.end > range.start {
            range.end - 1
        } else {
            range.start
        };
        let end_line = offset_to_line(&line_offsets, end_offset);
        let context_end = line_offsets
            .get(end_line + 1)
            .copied()
            .unwrap_or(source.len());
        let original = &source[line_start..context_end];

        let prefix = &source[line_start..range.start];
        let suffix = &source[range.end..context_end];
        let modified = format!("{prefix}{replacement}{suffix}");

        output.push_str(&format!("@@ -{} +{} @@\n", line_num + 1, line_num + 1));
        output.push_str(&format!("-{original}"));
        if !original.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&format!("+{modified}"));
        if !modified.ends_with('\n') {
            output.push('\n');
        }
    }

    output
}

/// Compute byte offsets of each line start in the source.
pub fn compute_line_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, c) in source.char_indices() {
        if c == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Convert a byte offset to a 0-based line number.
pub fn offset_to_line(line_offsets: &[usize], offset: usize) -> usize {
    match line_offsets.binary_search(&offset) {
        Ok(line) => line,
        Err(line) => line.saturating_sub(1),
    }
}

/// Convert a byte offset to (1-based line, 1-based column).
pub fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let line_offsets = compute_line_offsets(source);
    let line = offset_to_line(&line_offsets, offset);
    let col = offset - line_offsets[line];
    (line + 1, col + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_single_replacement() {
        let source = "hello GRPO world";
        let edits = vec![(6..10, "[[GRPO]]".to_owned())];
        assert_eq!(apply(source, &edits), "hello [[GRPO]] world");
    }

    #[test]
    fn apply_multiple_non_overlapping() {
        let source = "DPO and GRPO are methods";
        let edits = vec![(0..3, "[[DPO]]".to_owned()), (8..12, "[[GRPO]]".to_owned())];
        assert_eq!(apply(source, &edits), "[[DPO]] and [[GRPO]] are methods");
    }

    #[test]
    fn apply_preserves_surrounding_text() {
        let source = "before RLHF after";
        let edits = vec![(7..11, "[[RLHF]]".to_owned())];
        let result = apply(source, &edits);
        assert_eq!(result, "before [[RLHF]] after");
    }

    #[test]
    fn offset_to_line_col_first_line() {
        let source = "hello world";
        assert_eq!(offset_to_line_col(source, 0), (1, 1));
        assert_eq!(offset_to_line_col(source, 6), (1, 7));
    }

    #[test]
    fn offset_to_line_col_multiline() {
        let source = "line one\nline two\nline three";
        assert_eq!(offset_to_line_col(source, 9), (2, 1));
        assert_eq!(offset_to_line_col(source, 14), (2, 6));
        assert_eq!(offset_to_line_col(source, 18), (3, 1));
    }
}
