use std::path::Path;

use tree_sitter::{Language, Parser};

use super::{Chunk, Chunker};

/// Language-aware chunker using tree-sitter.
/// Splits code files on function/method/class boundaries.
pub struct TreeSitterChunker {
    min_chunk_tokens: usize,
    max_chunk_tokens: usize,
}

impl TreeSitterChunker {
    pub fn new(min_chunk_tokens: usize, max_chunk_tokens: usize) -> Self {
        Self {
            min_chunk_tokens,
            max_chunk_tokens,
        }
    }

    /// Detect language from file extension and return its tree-sitter Language.
    fn detect_language(path: &Path) -> Option<Language> {
        let ext = path.extension()?.to_str()?;
        match ext {
            "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
            "py" | "pyi" => Some(tree_sitter_python::LANGUAGE.into()),
            "js" | "jsx" | "mjs" | "cjs" => Some(tree_sitter_javascript::LANGUAGE.into()),
            "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            "go" => Some(tree_sitter_go::LANGUAGE.into()),
            "c" | "h" => Some(tree_sitter_c::LANGUAGE.into()),
            "cc" | "cpp" | "cxx" | "hpp" | "hxx" | "hh" => {
                Some(tree_sitter_cpp::LANGUAGE.into())
            }
            "java" => Some(tree_sitter_java::LANGUAGE.into()),
            "cs" => Some(tree_sitter_c_sharp::LANGUAGE.into()),
            _ => None,
        }
    }

    /// Node types that represent "top-level definitions" we want to extract as chunks.
    fn is_definition_node(kind: &str) -> bool {
        matches!(
            kind,
            // Rust
            // Rust
            "function_item"
                | "impl_item"
                | "struct_item"
                | "enum_item"
                | "trait_item"
                | "mod_item"
                | "macro_definition"
                // Python, C/C++
                | "function_definition"
                | "class_definition"
                // JS/TS, Go, Java
                | "function_declaration"
                | "class_declaration"
                | "method_definition"
                | "method_declaration"
                | "arrow_function"
                | "export_statement"
                // Go
                | "type_declaration"
                // C/C++
                | "struct_specifier"
                | "class_specifier"
                | "namespace_definition"
                // Java / C#
                | "interface_declaration"
                | "enum_declaration"
                | "constructor_declaration"
                // C#
                | "namespace_declaration"
                | "record_declaration"
                | "struct_declaration"
        )
    }

    fn extract_definitions<'a>(
        &self,
        content: &'a str,
        tree: &tree_sitter::Tree,
        file_path: &str,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let root = tree.root_node();

        // Walk top-level children and their immediate children (for impl blocks, classes, etc.)
        self.collect_definitions(root, content, file_path, &mut chunks, 0);

        // If no definitions were found, treat the whole file as one chunk
        if chunks.is_empty() && !content.trim().is_empty() {
            let token_count = content.split_whitespace().count();
            chunks.push(Chunk {
                file_path: file_path.to_string(),
                byte_offset: 0,
                line_number: 1,
                text: content.to_string(),
                token_count,
            });
        }

        chunks
    }

    fn collect_definitions(
        &self,
        node: tree_sitter::Node,
        content: &str,
        file_path: &str,
        chunks: &mut Vec<Chunk>,
        depth: usize,
    ) {
        let cursor = &mut node.walk();

        for child in node.children(cursor) {
            if Self::is_definition_node(child.kind()) {
                let start = child.start_byte();
                let end = child.end_byte();
                let text = &content[start..end];
                let token_count = text.split_whitespace().count();

                // Skip tiny definitions (e.g., single-line type aliases)
                if token_count < self.min_chunk_tokens && depth == 0 {
                    continue;
                }

                // If the definition is too large, try recursing into child definitions.
                // If no children found, split by blank-line-separated blocks.
                if token_count > self.max_chunk_tokens {
                    if depth < 2 {
                        let before = chunks.len();
                        self.collect_definitions(child, content, file_path, chunks, depth + 1);
                        if chunks.len() > before {
                            continue; // found child definitions
                        }
                    }
                    // No child definitions — split by blank-line blocks
                    self.split_large_node(child, content, file_path, chunks);
                    continue;
                }

                let line_number = child.start_position().row + 1;

                // Build context prefix: [file_path::parent_scope]
                let parent_context = self.get_parent_context(child, content);
                let context_prefix = if let Some(ctx) = parent_context {
                    format!("[{}::{}] ", file_path, ctx)
                } else {
                    format!("[{}] ", file_path)
                };

                chunks.push(Chunk {
                    file_path: file_path.to_string(),
                    byte_offset: start,
                    line_number,
                    text: format!("{}{}", context_prefix, text),
                    token_count: token_count + context_prefix.split_whitespace().count(),
                });
            } else if depth == 0 {
                // Check children of non-definition top-level nodes (e.g., for nested classes)
                self.collect_definitions(child, content, file_path, chunks, depth);
            }
        }
    }

    /// Split a large node (e.g. 500-line function) into sub-chunks by blank-line blocks.
    fn split_large_node(
        &self,
        node: tree_sitter::Node,
        content: &str,
        file_path: &str,
        chunks: &mut Vec<Chunk>,
    ) {
        let start = node.start_byte();
        let end = node.end_byte();
        let text = &content[start..end];
        let base_line = node.start_position().row + 1;

        // Build context prefix with parent scope
        let parent_context = self.get_parent_context(node, content);
        let context_prefix = if let Some(ctx) = parent_context {
            format!("[{}::{}] ", file_path, ctx)
        } else {
            format!("[{}] ", file_path)
        };

        // Split on double newlines (blank lines)
        let mut current_block = String::new();
        let mut block_start_line = 0usize;
        let mut line_offset = 0usize;

        for line in text.lines() {
            if line.trim().is_empty() && !current_block.trim().is_empty() {
                let token_count = current_block.split_whitespace().count();
                if token_count >= self.min_chunk_tokens {
                    let prefixed = format!("{}{}", context_prefix, current_block);
                    chunks.push(super::Chunk {
                        file_path: file_path.to_string(),
                        byte_offset: start,
                        line_number: base_line + block_start_line,
                        text: prefixed,
                        token_count: token_count + context_prefix.split_whitespace().count(),
                    });
                }
                current_block.clear();
                block_start_line = line_offset + 1;
            } else {
                if current_block.is_empty() {
                    block_start_line = line_offset;
                }
                current_block.push_str(line);
                current_block.push('\n');
            }
            line_offset += 1;
        }

        // Last block
        if !current_block.trim().is_empty() {
            let token_count = current_block.split_whitespace().count();
            if token_count >= self.min_chunk_tokens {
                let prefixed = format!("{}{}", context_prefix, current_block);
                chunks.push(super::Chunk {
                    file_path: file_path.to_string(),
                    byte_offset: start,
                    line_number: base_line + block_start_line,
                    text: prefixed,
                    token_count: token_count + context_prefix.split_whitespace().count(),
                });
            }
        }
    }

    /// Extract the name of the parent scope (impl block, class, module, etc.)
    fn get_parent_context(&self, node: tree_sitter::Node, content: &str) -> Option<String> {
        let parent = node.parent()?;
        let kind = parent.kind();

        // For impl blocks, classes, etc., extract the name
        match kind {
            "impl_item" | "class_definition" | "class_declaration" | "class_specifier"
            | "interface_declaration" | "namespace_definition" | "mod_item" | "trait_item" => {
                // Find the name child
                let cursor = &mut parent.walk();
                for child in parent.children(cursor) {
                    if child.kind() == "identifier"
                        || child.kind() == "type_identifier"
                        || child.kind() == "name"
                        || child.kind() == "scoped_type_identifier"
                    {
                        return Some(content[child.start_byte()..child.end_byte()].to_string());
                    }
                }
                None
            }
            _ => None,
        }
    }
}

impl Chunker for TreeSitterChunker {
    fn chunk(&self, path: &Path, content: &str) -> Vec<Chunk> {
        let language = match Self::detect_language(path) {
            Some(lang) => lang,
            None => return Vec::new(), // unsupported language — caller will use fallback
        };

        let mut parser = Parser::new();
        if parser.set_language(&language).is_err() {
            return Vec::new();
        }

        let tree = match parser.parse(content, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let file_path = path.to_string_lossy().to_string();
        self.extract_definitions(content, &tree, &file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_rust_functions() {
        let chunker = TreeSitterChunker::new(3, 1000);
        let content = r#"
fn hello() {
    println!("hello");
}

fn world(x: i32) -> i32 {
    x + 1
}

struct Foo {
    bar: String,
}
"#;
        let chunks = chunker.chunk(Path::new("test.rs"), content);
        assert!(chunks.len() >= 2, "Expected at least 2 chunks, got {}", chunks.len());
        // Check that function names appear in chunks
        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("fn hello")));
        assert!(texts.iter().any(|t| t.contains("fn world")));
    }

    #[test]
    fn test_python_functions() {
        let chunker = TreeSitterChunker::new(3, 1000);
        let content = r#"
def greet(name):
    print(f"Hello, {name}")

class Calculator:
    def add(self, a, b):
        return a + b

    def subtract(self, a, b):
        return a - b
"#;
        let chunks = chunker.chunk(Path::new("test.py"), content);
        assert!(!chunks.is_empty());
        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("def greet")));
    }

    #[test]
    fn test_unsupported_extension() {
        let chunker = TreeSitterChunker::new(3, 1000);
        let chunks = chunker.chunk(Path::new("readme.txt"), "some text content");
        assert!(chunks.is_empty());
    }
}
