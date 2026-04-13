mod naive;
mod prose;
mod treesitter;

pub use naive::NaiveChunker;
pub use prose::ProseChunker;
pub use treesitter::TreeSitterChunker;

use serde::Serialize;
use std::path::Path;

/// A chunk of text extracted from a file.
#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    /// Source file path
    pub file_path: String,
    /// Byte offset into the file where this chunk starts
    pub byte_offset: usize,
    /// Line number (1-based) where this chunk starts
    pub line_number: usize,
    /// The chunk text content
    pub text: String,
    /// Number of whitespace-delimited tokens in this chunk
    pub token_count: usize,
}

/// Trait for chunking strategies.
pub trait Chunker: Send + Sync {
    fn chunk(&self, path: &Path, content: &str) -> Vec<Chunk>;
}

/// Smart chunker that dispatches to the appropriate strategy based on file type:
/// - Tree-sitter for supported code languages (Rust, Python, JS/TS, Go, C/C++, Java)
/// - Prose chunker for markdown/text files
/// - Naive chunker as fallback
pub struct SmartChunker {
    treesitter: TreeSitterChunker,
    prose: ProseChunker,
    naive: NaiveChunker,
}

impl SmartChunker {
    pub fn new(chunk_size: usize, overlap: f32) -> Self {
        Self {
            treesitter: TreeSitterChunker::new(5, chunk_size),
            prose: ProseChunker::new(5, chunk_size),
            naive: NaiveChunker::new(chunk_size, overlap),
        }
    }
}

impl Chunker for SmartChunker {
    fn chunk(&self, path: &Path, content: &str) -> Vec<Chunk> {
        // Try tree-sitter first for code files
        let ts_chunks = self.treesitter.chunk(path, content);
        if !ts_chunks.is_empty() {
            return ts_chunks;
        }

        // Try prose chunker for text/markdown
        if ProseChunker::is_prose(path) {
            let prose_chunks = self.prose.chunk(path, content);
            if !prose_chunks.is_empty() {
                return prose_chunks;
            }
        }

        // Fall back to naive chunker
        self.naive.chunk(path, content)
    }
}
