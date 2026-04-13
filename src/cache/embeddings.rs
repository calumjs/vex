use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// On-disk embedding cache.
///
/// Cache structure: `~/.cache/vex/<project_hash>/`
///   - `index.bin`: maps blake3(file_content) → byte offset in embeddings.bin
///   - `embeddings.bin`: flat f32 matrix (num_chunks × dim)
///
/// Invalidation: by file content hash (blake3). Only re-embeds changed files.
pub struct EmbeddingCache {
    cache_dir: PathBuf,
    dim: usize,
    /// Maps blake3 content hash → Vec<f32> (one embedding per chunk, flattened)
    entries: HashMap<String, Vec<Vec<f32>>>,
    dirty: bool,
}

/// Header format for the cache file.
const CACHE_MAGIC: &[u8; 4] = b"SEMC";
const CACHE_VERSION: u32 = 1;

impl EmbeddingCache {
    /// Open or create a cache for the given project paths.
    pub fn open(project_paths: &[PathBuf], dim: usize) -> Result<Self> {
        let project_hash = Self::hash_project_paths(project_paths);
        let cache_dir = Self::cache_base_dir()?.join(&project_hash);
        fs::create_dir_all(&cache_dir)?;

        let mut cache = Self {
            cache_dir,
            dim,
            entries: HashMap::new(),
            dirty: false,
        };

        cache.load_from_disk().ok(); // ignore errors on first load
        Ok(cache)
    }

    /// Get cached embeddings for a file, or None if not cached / stale.
    pub fn get(&self, content: &str) -> Option<&Vec<Vec<f32>>> {
        let hash = blake3::hash(content.as_bytes()).to_hex().to_string();
        self.entries.get(&hash)
    }

    /// Store embeddings for a file's content.
    pub fn put(&mut self, content: &str, embeddings: Vec<Vec<f32>>) {
        let hash = blake3::hash(content.as_bytes()).to_hex().to_string();
        self.entries.insert(hash, embeddings);
        self.dirty = true;
    }

    /// Flush the cache to disk.
    pub fn save(&self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let path = self.cache_dir.join("cache.bin");
        let mut file = fs::File::create(&path)
            .context("Failed to create cache file")?;

        // Write header
        file.write_all(CACHE_MAGIC)?;
        file.write_all(&CACHE_VERSION.to_le_bytes())?;
        file.write_all(&(self.dim as u32).to_le_bytes())?;
        file.write_all(&(self.entries.len() as u32).to_le_bytes())?;

        // Write entries
        for (hash, chunks) in &self.entries {
            // Hash string (64 hex chars)
            let hash_bytes = hash.as_bytes();
            file.write_all(&(hash_bytes.len() as u16).to_le_bytes())?;
            file.write_all(hash_bytes)?;

            // Number of chunk embeddings for this file
            file.write_all(&(chunks.len() as u32).to_le_bytes())?;

            // Write each embedding
            for embedding in chunks {
                for &val in embedding {
                    file.write_all(&val.to_le_bytes())?;
                }
            }
        }

        Ok(())
    }

    fn load_from_disk(&mut self) -> Result<()> {
        let path = self.cache_dir.join("cache.bin");
        if !path.exists() {
            return Ok(());
        }

        let data = fs::read(&path)?;
        let mut pos = 0;

        // Read header
        if data.len() < 16 {
            return Ok(()); // too small, ignore
        }
        if &data[0..4] != CACHE_MAGIC {
            return Ok(()); // wrong magic
        }
        pos += 4;

        let version = u32::from_le_bytes(data[pos..pos + 4].try_into()?);
        if version != CACHE_VERSION {
            return Ok(()); // wrong version, ignore
        }
        pos += 4;

        let dim = u32::from_le_bytes(data[pos..pos + 4].try_into()?) as usize;
        if dim != self.dim {
            return Ok(()); // dimension mismatch
        }
        pos += 4;

        let num_entries = u32::from_le_bytes(data[pos..pos + 4].try_into()?) as usize;
        pos += 4;

        // Read entries
        for _ in 0..num_entries {
            if pos + 2 > data.len() {
                break;
            }
            let hash_len = u16::from_le_bytes(data[pos..pos + 2].try_into()?) as usize;
            pos += 2;

            if pos + hash_len > data.len() {
                break;
            }
            let hash = String::from_utf8_lossy(&data[pos..pos + hash_len]).to_string();
            pos += hash_len;

            if pos + 4 > data.len() {
                break;
            }
            let num_chunks = u32::from_le_bytes(data[pos..pos + 4].try_into()?) as usize;
            pos += 4;

            let mut chunks = Vec::with_capacity(num_chunks);
            for _ in 0..num_chunks {
                let bytes_needed = dim * 4;
                if pos + bytes_needed > data.len() {
                    return Ok(()); // truncated
                }
                let mut embedding = Vec::with_capacity(dim);
                for _ in 0..dim {
                    let val = f32::from_le_bytes(data[pos..pos + 4].try_into()?);
                    embedding.push(val);
                    pos += 4;
                }
                chunks.push(embedding);
            }

            self.entries.insert(hash, chunks);
        }

        Ok(())
    }

    fn hash_project_paths(paths: &[PathBuf]) -> String {
        let mut hasher = blake3::Hasher::new();
        for p in paths {
            hasher.update(p.to_string_lossy().as_bytes());
        }
        hasher.finalize().to_hex()[..16].to_string()
    }

    fn cache_base_dir() -> Result<PathBuf> {
        let base = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"));
        Ok(base.join("vex"))
    }
}
