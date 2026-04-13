use std::path::Path;

use super::{Chunk, Chunker};

/// Paragraph-aware chunker for text/markdown files.
/// Splits on paragraph boundaries (double newline) and markdown headers.
pub struct ProseChunker {
    min_chunk_tokens: usize,
    max_chunk_tokens: usize,
}

impl ProseChunker {
    pub fn new(min_chunk_tokens: usize, max_chunk_tokens: usize) -> Self {
        Self {
            min_chunk_tokens,
            max_chunk_tokens,
        }
    }

    /// Check if a file extension is a prose format.
    pub fn is_prose(path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        matches!(
            ext,
            "md" | "markdown" | "txt" | "text" | "rst" | "adoc" | "org" | "wiki"
        )
    }
}

impl Chunker for ProseChunker {
    fn chunk(&self, path: &Path, content: &str) -> Vec<Chunk> {
        if content.trim().is_empty() {
            return Vec::new();
        }

        let file_path = path.to_string_lossy().to_string();
        let mut chunks = Vec::new();
        let mut current_text = String::new();
        let mut current_start = 0usize;
        let mut current_line = 1usize;

        for line in content.lines() {
            let is_header = line.starts_with('#');
            let is_blank = line.trim().is_empty();

            // Start a new chunk at headers or after blank line separating substantial content
            if (is_header || is_blank) && !current_text.trim().is_empty() {
                let token_count = current_text.split_whitespace().count();

                if token_count >= self.min_chunk_tokens {
                    chunks.push(Chunk {
                        file_path: file_path.clone(),
                        byte_offset: current_start,
                        line_number: current_line,
                        text: current_text.trim().to_string(),
                        token_count,
                    });
                    current_text = String::new();
                    // current_start will be set when we add the next line
                }
            }

            if current_text.is_empty() && !line.trim().is_empty() {
                current_start = content[..content.len()]
                    .find(line)
                    .unwrap_or(0);
                // Compute line number from byte offset
                current_line = content[..current_start].matches('\n').count() + 1;
            }

            if !is_blank || !current_text.is_empty() {
                if !current_text.is_empty() {
                    current_text.push('\n');
                }
                current_text.push_str(line);
            }

            // If current chunk is getting too large, flush it
            if current_text.split_whitespace().count() >= self.max_chunk_tokens {
                let token_count = current_text.split_whitespace().count();
                chunks.push(Chunk {
                    file_path: file_path.clone(),
                    byte_offset: current_start,
                    line_number: current_line,
                    text: current_text.trim().to_string(),
                    token_count,
                });
                current_text = String::new();
            }
        }

        // Flush remaining content
        if !current_text.trim().is_empty() {
            let token_count = current_text.split_whitespace().count();
            if token_count >= self.min_chunk_tokens {
                chunks.push(Chunk {
                    file_path: file_path.clone(),
                    byte_offset: current_start,
                    line_number: current_line,
                    text: current_text.trim().to_string(),
                    token_count,
                });
            } else if let Some(last) = chunks.last_mut() {
                // Merge tiny trailing content into the previous chunk
                last.text.push('\n');
                last.text.push_str(current_text.trim());
                last.token_count += token_count;
            } else {
                // Only chunk in the file — keep it even if small
                chunks.push(Chunk {
                    file_path: file_path.clone(),
                    byte_offset: current_start,
                    line_number: current_line,
                    text: current_text.trim().to_string(),
                    token_count,
                });
            }
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_markdown_headers() {
        let chunker = ProseChunker::new(3, 500);
        let content = "# Title\n\nFirst paragraph with some words here.\n\n## Section\n\nSecond paragraph with more content here.\n";
        let chunks = chunker.chunk(Path::new("test.md"), content);
        assert!(chunks.len() >= 2, "Expected at least 2 chunks, got {}: {:?}", chunks.len(), chunks);
    }

    #[test]
    fn test_is_prose() {
        assert!(ProseChunker::is_prose(Path::new("readme.md")));
        assert!(ProseChunker::is_prose(Path::new("notes.txt")));
        assert!(!ProseChunker::is_prose(Path::new("main.rs")));
        assert!(!ProseChunker::is_prose(Path::new("app.py")));
    }

    #[test]
    fn test_empty_content() {
        let chunker = ProseChunker::new(3, 500);
        let chunks = chunker.chunk(Path::new("empty.md"), "");
        assert!(chunks.is_empty());
    }
}
