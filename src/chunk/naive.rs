use std::path::Path;

use super::{Chunk, Chunker};

/// Fixed-size sliding window chunker.
///
/// Splits text into chunks of approximately `chunk_size` whitespace-delimited tokens
/// with configurable overlap between consecutive chunks.
pub struct NaiveChunker {
    chunk_size: usize,
    overlap_fraction: f32,
}

impl NaiveChunker {
    pub fn new(chunk_size: usize, overlap_fraction: f32) -> Self {
        Self {
            chunk_size,
            overlap_fraction: overlap_fraction.clamp(0.0, 0.9),
        }
    }
}

impl Chunker for NaiveChunker {
    fn chunk(&self, path: &Path, content: &str) -> Vec<Chunk> {
        if content.is_empty() {
            return Vec::new();
        }

        let file_path = path.to_string_lossy().to_string();

        // Collect token boundaries (byte offsets of whitespace-delimited tokens)
        let tokens: Vec<(usize, usize)> = TokenIter::new(content).collect();

        if tokens.is_empty() {
            return Vec::new();
        }

        let overlap_tokens = (self.chunk_size as f32 * self.overlap_fraction) as usize;
        let stride = self.chunk_size.saturating_sub(overlap_tokens).max(1);

        let mut chunks = Vec::new();
        let mut start_idx = 0;

        while start_idx < tokens.len() {
            let end_idx = (start_idx + self.chunk_size).min(tokens.len());
            let byte_start = tokens[start_idx].0;
            let byte_end = tokens[end_idx - 1].1;

            let text = &content[byte_start..byte_end];
            let line_number = content[..byte_start].matches('\n').count() + 1;

            chunks.push(Chunk {
                file_path: file_path.clone(),
                byte_offset: byte_start,
                line_number,
                text: text.to_string(),
                token_count: end_idx - start_idx,
            });

            start_idx += stride;

            // Don't create a tiny trailing chunk
            if start_idx < tokens.len() && tokens.len() - start_idx < stride / 2 {
                // Extend the last chunk to the end instead
                let byte_start = tokens[start_idx].0;
                let byte_end = tokens[tokens.len() - 1].1;
                let text = &content[byte_start..byte_end];
                let line_number = content[..byte_start].matches('\n').count() + 1;

                chunks.push(Chunk {
                    file_path: file_path.clone(),
                    byte_offset: byte_start,
                    line_number,
                    text: text.to_string(),
                    token_count: tokens.len() - start_idx,
                });
                break;
            }
        }

        chunks
    }
}

/// Iterator over whitespace-delimited token boundaries (byte_start, byte_end).
struct TokenIter<'a> {
    content: &'a str,
    pos: usize,
}

impl<'a> TokenIter<'a> {
    fn new(content: &'a str) -> Self {
        Self { content, pos: 0 }
    }
}

impl Iterator for TokenIter<'_> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.content.as_bytes();

        // Skip whitespace
        while self.pos < bytes.len() && bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }

        if self.pos >= bytes.len() {
            return None;
        }

        let start = self.pos;

        // Advance through non-whitespace
        while self.pos < bytes.len() && !bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }

        Some((start, self.pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_empty_content() {
        let chunker = NaiveChunker::new(10, 0.2);
        let chunks = chunker.chunk(Path::new("test.rs"), "");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_small_content_fits_one_chunk() {
        let chunker = NaiveChunker::new(100, 0.2);
        let content = "hello world foo bar";
        let chunks = chunker.chunk(Path::new("test.rs"), content);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].token_count, 4);
        assert_eq!(chunks[0].byte_offset, 0);
        assert_eq!(chunks[0].line_number, 1);
    }

    #[test]
    fn test_multiple_chunks_with_overlap() {
        let chunker = NaiveChunker::new(4, 0.5);
        // 10 tokens, chunk_size=4, overlap=2, stride=2
        let content = "a b c d e f g h i j";
        let chunks = chunker.chunk(Path::new("test.rs"), content);
        assert!(chunks.len() > 1);
        // First chunk starts at offset 0
        assert_eq!(chunks[0].byte_offset, 0);
    }

    #[test]
    fn test_line_numbers() {
        let chunker = NaiveChunker::new(3, 0.0);
        let content = "line1 word1 word2\nline2 word3 word4\nline3 word5 word6";
        let chunks = chunker.chunk(Path::new("test.rs"), content);
        assert_eq!(chunks[0].line_number, 1);
        if chunks.len() > 1 {
            assert!(chunks[1].line_number >= 2);
        }
    }
}
