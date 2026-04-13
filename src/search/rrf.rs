use std::collections::HashMap;

use super::SearchResult;

/// Reciprocal Rank Fusion: combine BM25 ranks with neural similarity scores.
///
/// `score = 1/(k + bm25_rank) + 1/(k + neural_rank)`
///
/// where k=60 (standard RRF constant). Items strong in both signals rise;
/// items that are flukes in one signal drop.
pub fn fuse_rrf(
    neural_results: &[SearchResult],
    bm25_ranked_indices: &[(usize, f32)], // (chunk_index, bm25_score)
    top_k: usize,
) -> Vec<SearchResult> {
    const K: f32 = 60.0;

    // Build BM25 rank lookup: chunk_index → rank (0-based)
    let bm25_ranks: HashMap<usize, usize> = bm25_ranked_indices
        .iter()
        .enumerate()
        .map(|(rank, (idx, _))| (*idx, rank))
        .collect();

    // Build neural rank lookup
    let neural_ranks: HashMap<usize, usize> = neural_results
        .iter()
        .enumerate()
        .map(|(rank, r)| (r.chunk_index, rank))
        .collect();

    // Collect all unique chunk indices from both sources
    let mut all_indices: Vec<usize> = Vec::new();
    for (idx, _) in bm25_ranked_indices {
        if !all_indices.contains(idx) {
            all_indices.push(*idx);
        }
    }
    for r in neural_results {
        if !all_indices.contains(&r.chunk_index) {
            all_indices.push(r.chunk_index);
        }
    }

    // Score each by RRF
    let absent_rank = (bm25_ranked_indices.len() + neural_results.len()) as f32;
    let mut fused: Vec<SearchResult> = all_indices
        .iter()
        .map(|&idx| {
            let bm25_rank = bm25_ranks.get(&idx).copied().unwrap_or(absent_rank as usize) as f32;
            let neural_rank =
                neural_ranks.get(&idx).copied().unwrap_or(absent_rank as usize) as f32;

            let score = 1.0 / (K + bm25_rank) + 1.0 / (K + neural_rank);
            SearchResult {
                chunk_index: idx,
                score,
            }
        })
        .collect();

    fused.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    fused.truncate(top_k);
    fused
}
