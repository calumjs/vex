use super::SearchResult;
use crate::chunk::Chunk;

/// Remove results that overlap with higher-ranked results from the same file.
/// Keeps the first (highest-scored) result for each file region.
pub fn dedup_overlapping(results: &mut Vec<SearchResult>, chunks: &[Chunk]) {
    let mut seen: Vec<(&str, usize, usize)> = Vec::new();

    results.retain(|r| {
        let chunk = &chunks[r.chunk_index];
        let start = chunk.line_number;
        let end = start + chunk.text.lines().count();

        for &(path, s, e) in &seen {
            if path == chunk.file_path && start < e && end > s {
                return false; // overlaps with a higher-ranked result
            }
        }
        seen.push((&chunk.file_path, start, end));
        true
    });
}
