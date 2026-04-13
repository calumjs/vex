use std::collections::HashMap;

use super::SearchResult;
use crate::chunk::Chunk;

/// Remove results that overlap with higher-ranked results from the same file,
/// and cap at MAX_PER_FILE results per file to prevent one file dominating.
const MAX_PER_FILE: usize = 2;

pub fn dedup_overlapping(results: &mut Vec<SearchResult>, chunks: &[Chunk]) {
    let mut seen_ranges: Vec<(&str, usize, usize)> = Vec::new();
    let mut file_counts: HashMap<&str, usize> = HashMap::new();

    results.retain(|r| {
        let chunk = &chunks[r.chunk_index];
        let path = chunk.file_path.as_str();
        let start = chunk.line_number;
        let end = start + chunk.text.lines().count();

        // Skip if overlapping with a higher-ranked result from the same file
        for &(p, s, e) in &seen_ranges {
            if p == path && start < e && end > s {
                return false;
            }
        }

        // Skip if this file already has enough results
        let count = file_counts.entry(path).or_insert(0);
        if *count >= MAX_PER_FILE {
            return false;
        }

        *count += 1;
        seen_ranges.push((path, start, end));
        true
    });
}
