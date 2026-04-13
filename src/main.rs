mod cache;
mod chunk;
mod download;
mod embed;
mod output;
mod search;
mod walk;

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use chunk::Chunker;
use clap::Parser;
use ndarray::Array1;

#[derive(Parser, Debug)]
#[command(name = "vex", version, about = "Semantic grep — find code and text by meaning")]
pub struct Cli {
    /// What to search for (natural language)
    pub query: String,

    /// Files or directories to search [default: .]
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Number of results
    #[arg(short = 'k', long = "top", default_value_t = 10)]
    pub top_k: usize,

    /// Minimum similarity score [0.0–1.0]
    #[arg(short = 't', long)]
    pub threshold: Option<f32>,

    /// Lines of context around match
    #[arg(short = 'C', long = "context", default_value_t = 2)]
    pub context_lines: usize,

    /// Only search files matching glob
    #[arg(short = 'g', long)]
    pub glob: Option<String>,

    /// Approximate chunk size in whitespace-delimited tokens
    #[arg(long, default_value_t = 512)]
    pub chunk_size: usize,

    /// Chunk overlap as a fraction (0.0–1.0)
    #[arg(long, default_value_t = 0.2)]
    pub overlap: f32,

    /// Search hidden files and directories
    #[arg(long)]
    pub hidden: bool,

    /// Don't respect .gitignore files
    #[arg(long)]
    pub no_gitignore: bool,

    /// Inference device: npu, gpu, cpu, auto
    #[arg(long, default_value = "auto")]
    pub device: String,

    /// Binary quantization (faster, less precise)
    #[arg(long)]
    pub fast: bool,

    /// Don't use or write embedding cache
    #[arg(long)]
    pub no_cache: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Disable colored output
    #[arg(long)]
    pub no_color: bool,

    /// Inference threads [default: num_cpus]
    #[arg(long)]
    pub threads: Option<usize>,

    /// Embedding model
    #[arg(long, default_value = "minilm-l6-v2")]
    pub model: String,

    /// Path to model directory (overrides default model resolution)
    #[arg(long)]
    pub model_dir: Option<PathBuf>,
}

/// Resolve the model directory path.
/// Search order: --model-dir flag > ./models/<name>/ > system data dir > auto-download
fn resolve_model_dir(cli: &Cli) -> Result<PathBuf> {
    if let Some(ref dir) = cli.model_dir {
        return Ok(dir.clone());
    }

    // Check local models/ directory (for development)
    let local = PathBuf::from("models").join(&cli.model);
    if local.join("tokenizer.json").exists() {
        return Ok(local);
    }

    // Check platform data directory
    if let Some(data_dir) = dirs::data_local_dir() {
        let system = data_dir.join("vex").join("models").join(&cli.model);
        if system.join("tokenizer.json").exists() {
            return Ok(system);
        }
    }

    // Auto-download the default model
    if cli.model == "minilm-l6-v2" {
        return download::download_default_model();
    }

    anyhow::bail!(
        "Model '{}' not found. Use the default model or provide --model-dir.\n\
         To download the default: vex \"query\" path/  (downloads automatically on first run)",
        cli.model,
    )
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let total_start = Instant::now();

    // Resolve model directory
    let model_dir = resolve_model_dir(&cli)?;

    // Load embedder
    let device = embed::Device::from_str(&cli.device);
    let mut embedder = embed::OnnxEmbedder::load(&model_dir, device)
        .context("Failed to load embedding model")?;

    // Walk files
    let walk_start = Instant::now();
    let files = walk::walk_paths(&cli)?;
    if files.is_empty() {
        eprintln!("vex: no files found");
        return Ok(());
    }
    let walk_time = walk_start.elapsed();

    // Chunk all files (smart: tree-sitter for code, prose for md/txt, naive fallback)
    let chunk_start = Instant::now();
    let chunker = chunk::SmartChunker::new(cli.chunk_size, cli.overlap);

    // Read files and chunk them. Track file content for caching.
    let mut all_chunks = Vec::new();
    let mut file_contents: Vec<(PathBuf, String)> = Vec::new();

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let file_chunks = chunker.chunk(file, &content);
        if !file_chunks.is_empty() {
            all_chunks.extend(file_chunks);
            file_contents.push((file.clone(), content));
        }
    }

    if all_chunks.is_empty() {
        eprintln!("vex: no chunks produced from {} files", files.len());
        return Ok(());
    }
    let chunk_time = chunk_start.elapsed();

    // Open embedding cache (unless --no-cache)
    let mut embedding_cache = if !cli.no_cache {
        cache::EmbeddingCache::open(&cli.paths, embedder.dim()).ok()
    } else {
        None
    };

    // Embed query
    let embed_start = Instant::now();
    let query_vec = embedder.embed_one(&cli.query)?;
    let query_arr = Array1::from_vec(query_vec);

    // Embed chunks, using cache where possible
    let batch_size = 64;
    let dim = embedder.dim();
    let mut corpus_vecs: Vec<f32> = Vec::with_capacity(all_chunks.len() * dim);
    let mut cache_hits = 0usize;
    let mut cache_misses = 0usize;

    // Group chunks by file content for cache lookup
    let mut chunk_idx = 0;
    for (_path, content) in &file_contents {
        // Count chunks for this file
        let file_chunk_count = all_chunks[chunk_idx..]
            .iter()
            .take_while(|c| c.file_path == all_chunks[chunk_idx].file_path)
            .count();

        // Try cache lookup
        if let Some(ref cache) = embedding_cache {
            if let Some(cached) = cache.get(content) {
                if cached.len() == file_chunk_count {
                    for emb in cached {
                        corpus_vecs.extend(emb);
                    }
                    cache_hits += file_chunk_count;
                    chunk_idx += file_chunk_count;
                    continue;
                }
            }
        }

        // Cache miss: embed this file's chunks
        let file_chunk_texts: Vec<&str> = all_chunks[chunk_idx..chunk_idx + file_chunk_count]
            .iter()
            .map(|c| c.text.as_str())
            .collect();

        let mut file_embeddings = Vec::new();
        for batch in file_chunk_texts.chunks(batch_size) {
            let batch_emb = embedder.embed_batch(batch)?;
            for row in batch_emb.rows() {
                let vec: Vec<f32> = row.to_vec();
                corpus_vecs.extend(&vec);
                file_embeddings.push(vec);
            }
        }

        // Store in cache
        if let Some(ref mut cache) = embedding_cache {
            cache.put(content, file_embeddings);
        }

        cache_misses += file_chunk_count;
        chunk_idx += file_chunk_count;
    }

    let corpus = ndarray::Array2::from_shape_vec((all_chunks.len(), dim), corpus_vecs)?;
    let embed_time = embed_start.elapsed();

    // Save cache
    if let Some(ref cache) = embedding_cache {
        if let Err(e) = cache.save() {
            eprintln!("vex: warning: failed to save cache: {e}");
        }
    }

    // Search (use binary quantization if --fast)
    let search_start = Instant::now();
    let results = if cli.fast {
        search::search_topk_binary(&query_arr, &corpus, cli.top_k, cli.threshold)
    } else {
        search::search_topk(&query_arr, &corpus, cli.top_k, cli.threshold)
    };
    let search_time = search_start.elapsed();

    // Output results
    if results.is_empty() {
        eprintln!("vex: no results above threshold");
        return Ok(());
    }

    if cli.json {
        output::print_results_json(&results, &all_chunks)?;
    } else {
        output::print_results(&results, &all_chunks, cli.context_lines, cli.no_color)?;
    }

    // Print timing info to stderr
    let total_time = total_start.elapsed();
    let cache_info = if embedding_cache.is_some() {
        format!(" | cache {cache_hits}/{} hits", cache_hits + cache_misses)
    } else {
        String::new()
    };
    eprintln!(
        "\nvex: {} files, {} chunks | walk {:.0}ms, chunk {:.0}ms, embed {:.0}ms, search {:.0}ms{} | total {:.0}ms",
        files.len(),
        all_chunks.len(),
        walk_time.as_secs_f64() * 1000.0,
        chunk_time.as_secs_f64() * 1000.0,
        embed_time.as_secs_f64() * 1000.0,
        search_time.as_secs_f64() * 1000.0,
        cache_info,
        total_time.as_secs_f64() * 1000.0,
    );

    Ok(())
}
