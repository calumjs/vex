use rayon::prelude::*;

/// BM25 ranking for fast pre-filtering before neural re-ranking.
/// Optimized to avoid per-document tokenization — scans documents directly.
pub struct Bm25 {
    k1: f32,
    b: f32,
}

impl Bm25 {
    pub fn new() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }

    /// Score all documents against a query, returning (doc_index, score) sorted descending.
    /// Only returns documents with score > 0.
    pub fn rank(&self, query: &str, documents: &[&str]) -> Vec<(usize, f32)> {
        if documents.is_empty() {
            return Vec::new();
        }

        // Tokenize just the query (tiny)
        let query_terms: Vec<String> = query
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| s.len() >= 2)
            .map(|s| s.to_lowercase())
            .collect();
        if query_terms.is_empty() {
            return Vec::new();
        }

        let n = documents.len() as f32;

        // Compute doc lengths and average length in parallel (just count words, no allocation)
        let doc_lengths: Vec<f32> = documents
            .par_iter()
            .map(|d| word_count(d) as f32)
            .collect();
        let avgdl: f32 = doc_lengths.iter().sum::<f32>() / n;

        // Compute document frequency for each query term in parallel
        let dfs: Vec<f32> = query_terms
            .iter()
            .map(|term| {
                documents
                    .par_iter()
                    .filter(|doc| contains_term_ci(doc, term))
                    .count() as f32
            })
            .collect();

        // Precompute IDF for each query term
        let idfs: Vec<f32> = dfs
            .iter()
            .map(|&df| {
                if df == 0.0 {
                    0.0
                } else {
                    ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
                }
            })
            .collect();

        // Score each document in parallel — scan for term frequencies without tokenizing
        let mut scores: Vec<(usize, f32)> = documents
            .par_iter()
            .enumerate()
            .filter_map(|(idx, doc)| {
                let dl = doc_lengths[idx];
                let mut score = 0.0f32;

                for (i, term) in query_terms.iter().enumerate() {
                    let idf = idfs[i];
                    if idf == 0.0 {
                        continue;
                    }

                    let tf = count_term_ci(doc, term) as f32;
                    if tf == 0.0 {
                        continue;
                    }

                    score += idf * (tf * (self.k1 + 1.0))
                        / (tf + self.k1 * (1.0 - self.b + self.b * dl / avgdl));
                }

                if score > 0.0 {
                    Some((idx, score))
                } else {
                    None
                }
            })
            .collect();

        scores.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }
}

/// Count whitespace-delimited words without allocating.
#[inline]
fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Check if a document contains a term (case-insensitive, word-boundary aware).
#[inline]
fn contains_term_ci(doc: &str, term: &str) -> bool {
    // Fast path: if the term bytes aren't present at all, skip
    let doc_lower_iter = doc.as_bytes();
    let term_bytes = term.as_bytes(); // already lowercase

    if doc_lower_iter.len() < term_bytes.len() {
        return false;
    }

    // Scan for the term using case-insensitive byte matching
    for window_start in 0..=(doc_lower_iter.len() - term_bytes.len()) {
        // Check word boundary before
        if window_start > 0 {
            let prev = doc_lower_iter[window_start - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                continue;
            }
        }

        // Check word boundary after
        let end = window_start + term_bytes.len();
        if end < doc_lower_iter.len() {
            let next = doc_lower_iter[end];
            if next.is_ascii_alphanumeric() || next == b'_' {
                continue;
            }
        }

        // Compare bytes case-insensitively
        let mut matched = true;
        for i in 0..term_bytes.len() {
            if doc_lower_iter[window_start + i].to_ascii_lowercase() != term_bytes[i] {
                matched = false;
                break;
            }
        }
        if matched {
            return true;
        }
    }
    false
}

/// Count occurrences of a term in a document (case-insensitive, word-boundary aware).
#[inline]
fn count_term_ci(doc: &str, term: &str) -> usize {
    let doc_bytes = doc.as_bytes();
    let term_bytes = term.as_bytes();
    let mut count = 0;

    if doc_bytes.len() < term_bytes.len() {
        return 0;
    }

    for window_start in 0..=(doc_bytes.len() - term_bytes.len()) {
        // Word boundary before
        if window_start > 0 {
            let prev = doc_bytes[window_start - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                continue;
            }
        }

        // Word boundary after
        let end = window_start + term_bytes.len();
        if end < doc_bytes.len() {
            let next = doc_bytes[end];
            if next.is_ascii_alphanumeric() || next == b'_' {
                continue;
            }
        }

        // Case-insensitive byte compare
        let mut matched = true;
        for i in 0..term_bytes.len() {
            if doc_bytes[window_start + i].to_ascii_lowercase() != term_bytes[i] {
                matched = false;
                break;
            }
        }
        if matched {
            count += 1;
        }
    }
    count
}
