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
use rayon::prelude::*;

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

    /// Pre-filter files by literal keyword (boosts files containing this term)
    #[arg(long)]
    pub literal: Vec<String>,

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

    // Set ORT dynamic library path — look next to our own executable first,
    // then fall back to system search path.
    if std::env::var_os("ORT_DYLIB_PATH").is_none() {
        if let Ok(exe) = std::env::current_exe() {
            let ort_dll = exe.with_file_name("onnxruntime.dll");
            if ort_dll.exists() {
                // SAFETY: called at program startup before any threads are spawned.
                unsafe { std::env::set_var("ORT_DYLIB_PATH", &ort_dll); }
            }
        }
    }

    // Resolve model directory
    let model_dir = resolve_model_dir(&cli)?;

    // Load embedder
    let device = embed::Device::from_str(&cli.device);
    let mut embedder = embed::OnnxEmbedder::load(&model_dir, device, cli.threads)
        .context("Failed to load embedding model")?;

    // Walk files
    let walk_start = Instant::now();
    let files = walk::walk_paths(&cli)?;
    if files.is_empty() {
        eprintln!("vex: no files found");
        return Ok(());
    }
    let walk_time = walk_start.elapsed();

    // Two-phase pipeline: score files by keyword relevance, then only chunk the
    // most promising ones. This avoids tree-sitter parsing thousands of files.
    let chunk_start = Instant::now();
    let chunker = chunk::SmartChunker::new(cli.chunk_size, cli.overlap);

    let mut query_terms: Vec<String> = cli
        .query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| s.len() >= 2)
        .map(|s| s.to_lowercase())
        .collect();

    // --literal hints get added as extra search terms for file scoring
    for lit in &cli.literal {
        let term = lit.to_lowercase();
        if !query_terms.contains(&term) {
            query_terms.push(term);
        }
    }

    // Phase 1: Score files by keyword matches using mmap (no heap allocation
    // for non-matching files). Only read matching files into Strings.
    let max_files_to_chunk = 200;

    let mut scored_files: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            // Skip binary/large files by extension before reading
            if let Some(ext) = file.extension().and_then(|e| e.to_str()) {
                match ext.to_ascii_lowercase().as_str() {
                    "png" | "jpg" | "jpeg" | "gif" | "ico" | "svg" | "bmp" | "webp" |
                    "exe" | "dll" | "pdb" | "obj" | "lib" | "so" | "dylib" |
                    "zip" | "gz" | "tar" | "7z" | "rar" | "nupkg" |
                    "woff" | "woff2" | "ttf" | "eot" | "otf" |
                    "mp3" | "mp4" | "avi" | "mov" | "wav" |
                    "pdf" | "doc" | "docx" | "xls" | "xlsx" | "pptx" |
                    "lock" | "map" | "min" | "snap" | "pyc" | "class" => return None,
                    _ => {}
                }
            }

            let bytes = std::fs::read(file).ok()?;
            // Skip empty/huge files
            if bytes.is_empty() || bytes.len() > 2_000_000 {
                return None;
            }

            // Score by distinct query term matches — first-byte check to skip
            // most bytes without calling to_ascii_lowercase on every one.
            let score: usize = query_terms.iter().filter(|term| {
                let tb = term.as_bytes();
                let c0 = tb[0];
                let c0_upper = c0.to_ascii_uppercase();
                bytes.len() >= tb.len()
                    && (0..=bytes.len() - tb.len()).any(|i| {
                        let b = bytes[i];
                        (b == c0 || b == c0_upper)
                            && bytes[i..i + tb.len()]
                                .iter()
                                .zip(tb)
                                .all(|(a, b)| a.to_ascii_lowercase() == *b)
                    })
            }).count();

            if score == 0 {
                return None;
            }

            let content = String::from_utf8(bytes).ok()?;
            Some((file.clone(), content, score))
        })
        .collect();

    // Phase 2: Take the top files by relevance score, then chunk only those.
    scored_files.sort_unstable_by(|a, b| b.2.cmp(&a.2));
    scored_files.truncate(max_files_to_chunk);

    let file_results: Vec<_> = scored_files
        .par_iter()
        .filter_map(|(path, content, _score)| {
            let file_chunks = chunker.chunk(path, content);
            if file_chunks.is_empty() {
                None
            } else {
                Some((path.clone(), content.clone(), file_chunks))
            }
        })
        .collect();

    let mut all_chunks = Vec::new();
    let mut file_contents: Vec<(PathBuf, String)> = Vec::new();
    for (path, content, chunks) in file_results {
        all_chunks.extend(chunks);
        file_contents.push((path, content));
    }

    if all_chunks.is_empty() {
        eprintln!("vex: no chunks produced from {} files", files.len());
        return Ok(());
    }

    let chunk_time = chunk_start.elapsed();

    // BM25 pre-filter: quickly narrow candidates with lexical matching,
    // then embed only the top candidates for semantic re-ranking.
    // This brings embedding time from ~16s (all 4410 chunks) to ~400ms (top 100).
    let embed_start = Instant::now();
    let bm25_target = 5 * cli.top_k.max(10); // embed 5x more than requested
    let candidate_indices: Vec<usize> = if all_chunks.len() > bm25_target {
        let bm25 = search::bm25::Bm25::new();
        let texts: Vec<&str> = all_chunks.iter().map(|c| c.text.as_str()).collect();
        let ranked = bm25.rank(&cli.query, &texts);

        // Start with BM25 top candidates
        let mut indices: Vec<usize> = if ranked.len() > bm25_target {
            ranked[..bm25_target].iter().map(|(idx, _)| *idx).collect()
        } else if ranked.is_empty() {
            (0..all_chunks.len()).collect()
        } else {
            ranked.iter().map(|(idx, _)| *idx).collect()
        };

        // Hybrid boost: also include chunks where query terms appear as
        // identifiers (in file path or code). This catches exact matches
        // that BM25 might rank low due to high document frequency.
        let mut seen: std::collections::HashSet<usize> = indices.iter().copied().collect();
        let boost_limit = bm25_target / 2; // add up to 50% more from grep matches
        let mut boosted = 0;
        for (i, chunk) in all_chunks.iter().enumerate() {
            if boosted >= boost_limit {
                break;
            }
            if seen.contains(&i) {
                continue;
            }
            // Check if the file path contains any query term (catches class names)
            let path_lower = chunk.file_path.to_lowercase();
            let has_path_match = query_terms.iter().any(|t| path_lower.contains(t.as_str()));
            if has_path_match {
                indices.push(i);
                seen.insert(i);
                boosted += 1;
            }
        }

        indices
    } else {
        (0..all_chunks.len()).collect()
    };

    // Embed query + candidates in a single batch to avoid double session.run() overhead.
    let dim = embedder.dim();
    let mut all_texts: Vec<&str> = Vec::with_capacity(1 + candidate_indices.len());
    all_texts.push(&cli.query);
    for &i in &candidate_indices {
        all_texts.push(all_chunks[i].text.as_str());
    }

    let batch_size = 256;
    let mut all_vecs: Vec<f32> = Vec::with_capacity(all_texts.len() * dim);
    for batch in all_texts.chunks(batch_size) {
        let batch_emb = embedder.embed_batch(batch)?;
        for row in batch_emb.rows() {
            all_vecs.extend(row.iter());
        }
    }

    // First row is query, rest are candidates
    let query_arr = Array1::from_vec(all_vecs[..dim].to_vec());
    let candidate_vecs = all_vecs[dim..].to_vec();
    let corpus = ndarray::Array2::from_shape_vec((candidate_indices.len(), dim), candidate_vecs)?;
    let embed_time = embed_start.elapsed();

    // Search within the candidates
    let search_start = Instant::now();
    let candidate_results = if cli.fast {
        search::search_topk_binary(&query_arr, &corpus, cli.top_k, cli.threshold)
    } else {
        search::search_topk(&query_arr, &corpus, cli.top_k, cli.threshold)
    };

    // Map candidate indices back to original chunk indices
    let results: Vec<search::SearchResult> = candidate_results
        .into_iter()
        .map(|r| search::SearchResult {
            chunk_index: candidate_indices[r.chunk_index],
            score: r.score,
        })
        .collect();
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
    let embedded_count = candidate_indices.len();
    eprintln!(
        "\nvex: {} files, {} chunks (embedded {}) | walk {:.0}ms, chunk {:.0}ms, embed {:.0}ms, search {:.0}ms | total {:.0}ms",
        files.len(),
        all_chunks.len(),
        embedded_count,
        walk_time.as_secs_f64() * 1000.0,
        chunk_time.as_secs_f64() * 1000.0,
        embed_time.as_secs_f64() * 1000.0,
        search_time.as_secs_f64() * 1000.0,
        total_time.as_secs_f64() * 1000.0,
    );

    Ok(())
}
