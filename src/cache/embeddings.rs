use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Chunk-level embedding cache.
///
/// Caches individual chunk embeddings keyed by blake3 hash of the chunk text.
/// Any chunk embedded in a previous query is instantly reusable in future queries,
/// regardless of which BM25 candidates were selected.
///
/// On-disk format: simple binary — [magic][version][dim][count][entries...]
/// Each entry: [hash: 32 bytes][embedding: dim * 4 bytes]
pub struct EmbeddingCache {
    cache_path: PathBuf,
    dim: usize,
    entries: HashMap<[u8; 32], Vec<f32>>,
    dirty: bool,
}

const CACHE_MAGIC: &[u8; 4] = b"VXEC";
const CACHE_VERSION: u32 = 2;

impl EmbeddingCache {
    /// Open or create a chunk embedding cache.
    pub fn open(dim: usize) -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("vex");
        fs::create_dir_all(&cache_dir)?;
        let cache_path = cache_dir.join("chunk_embeddings.bin");

        let mut cache = Self {
            cache_path,
            dim,
            entries: HashMap::new(),
            dirty: false,
        };
        cache.load().ok(); // ignore errors on first load
        Ok(cache)
    }

    /// Look up a cached embedding by chunk text.
    pub fn get(&self, chunk_text: &str) -> Option<&Vec<f32>> {
        let hash = blake3::hash(chunk_text.as_bytes());
        self.entries.get(hash.as_bytes())
    }

    /// Store an embedding for a chunk.
    pub fn put(&mut self, chunk_text: &str, embedding: Vec<f32>) {
        let hash = *blake3::hash(chunk_text.as_bytes()).as_bytes();
        self.entries.insert(hash, embedding);
        self.dirty = true;
    }

    /// Number of cached embeddings.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Save cache to disk.
    pub fn save(&self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let mut data = Vec::with_capacity(16 + self.entries.len() * (32 + self.dim * 4));

        // Header
        data.extend_from_slice(CACHE_MAGIC);
        data.extend_from_slice(&CACHE_VERSION.to_le_bytes());
        data.extend_from_slice(&(self.dim as u32).to_le_bytes());
        data.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());

        // Entries
        for (hash, embedding) in &self.entries {
            data.extend_from_slice(hash);
            for &val in embedding {
                data.extend_from_slice(&val.to_le_bytes());
            }
        }

        fs::write(&self.cache_path, &data)
            .context("Failed to write embedding cache")?;

        Ok(())
    }

    fn load(&mut self) -> Result<()> {
        let data = fs::read(&self.cache_path)?;
        let mut pos = 0;

        // Header
        if data.len() < 16 || &data[0..4] != CACHE_MAGIC {
            return Ok(());
        }
        pos += 4;

        let version = u32::from_le_bytes(data[pos..pos + 4].try_into()?);
        if version != CACHE_VERSION {
            return Ok(());
        }
        pos += 4;

        let dim = u32::from_le_bytes(data[pos..pos + 4].try_into()?) as usize;
        if dim != self.dim {
            return Ok(());
        }
        pos += 4;

        let count = u32::from_le_bytes(data[pos..pos + 4].try_into()?) as usize;
        pos += 4;

        let entry_size = 32 + dim * 4;
        for _ in 0..count {
            if pos + entry_size > data.len() {
                break;
            }

            let mut hash = [0u8; 32];
            hash.copy_from_slice(&data[pos..pos + 32]);
            pos += 32;

            let mut embedding = Vec::with_capacity(dim);
            for _ in 0..dim {
                let val = f32::from_le_bytes(data[pos..pos + 4].try_into()?);
                embedding.push(val);
                pos += 4;
            }

            self.entries.insert(hash, embedding);
        }

        Ok(())
    }
}
