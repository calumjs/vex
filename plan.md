# vex — JIT Semantic Grep

## Implementation Plan

---

## Target Machine

| Component | Detail |
|-----------|--------|
| **SoC** | Snapdragon X Elite (X1E80100) — ARM64 |
| **CPU** | 12× Qualcomm Oryon cores @ 3.40 GHz (no SMT) |
| **NPU** | Qualcomm Hexagon NPU — **45 TOPS INT8** |
| **GPU** | Qualcomm Adreno X1-85 (DX12, Vulkan, DirectML) |
| **RAM** | 64 GB LPDDR5X @ 8448 MT/s |
| **OS** | Windows 11 ARM64 (Build 26220) |
| **Rust** | 1.88.0, target: `aarch64-pc-windows-msvc` |
| **Python** | 3.12.10 ARM64 (transformers, tokenizers, onnxruntime installed) |

> **NPU Strategy:** The Hexagon NPU's 45 TOPS INT8 throughput is the primary acceleration target.
> ONNX Runtime's **DirectML execution provider** exposes the NPU on ARM64 Windows.
> INT8 quantized models are prioritized to maximize NPU utilization.
> Fallback chain: NPU (DirectML) → GPU (DirectML) → CPU (ARM64 NEON).

---

## Overview

`vex` is a command-line tool that performs semantic search over local files without pre-indexing. It embeds a query and a corpus of text chunks at query time, then ranks results by meaning similarity. Think `ripgrep`, but it understands what you mean, not just what you typed.

```
vex "error handling in authentication" src/ --top 5
```

---

## Architecture

```
                         ┌──────────────────────────────────┐
    CLI input            │           vex binary             │
   ──────────►           │        (aarch64-msvc)            │
                         │                                  │
                         │  1. Walk ──► 2. Chunk ──► 3. Embed ──► 4. Rank ──► 5. Output
                         │  (ignore)   (treesitter)  (ort)      (ndarray)   (stream)
                         │                   DirectML ──┘
                         │              NPU ◄──┘ GPU ◄──┘ CPU ◄──┘
                         └──────────────────────────────────┘
                                         │
                                    model.onnx (INT8 quantized)
                              (bundled or auto-downloaded, ~23 MB)
```

### Core Pipeline

| Phase | What | Crate / Tool | Bound by |
|-------|------|-------------|----------|
| Walk | Recursive file discovery, .gitignore-aware | `ignore` | I/O |
| Chunk | Split files into meaningful segments | `tree-sitter`, custom | CPU (light) |
| Embed | Encode query + chunks into vectors | `ort` + **DirectML** (NPU/GPU) | **NPU** / CPU |
| Rank | Brute-force cosine similarity | `ndarray` (ARM64 NEON) | Memory bandwidth |
| Output | Stream top-k results to terminal | `termcolor` | Trivial |

---

## Milestones

### M0 — Skeleton

**Goal:** CLI that walks files and prints chunk boundaries. No ML yet.

**Deliverables:**
- Rust project scaffold (`cargo init vex`) — target `aarch64-pc-windows-msvc`
- CLI argument parsing via `clap`
  - Positional: `<query>`, `<paths...>`
  - Flags: `--top <k>`, `--chunk-size <n>`, `--glob <pattern>`, `--hidden`, `--no-gitignore`
  - NPU flag: `--device <npu|gpu|cpu>` (default: auto-detect)
- File walker using `ignore` crate
  - Respects `.gitignore`, `.vexignore` (custom)
  - Follows symlinks optionally
  - Filters by extension or glob
- Naive chunker: fixed-size sliding window over UTF-8 text
  - Configurable chunk size (default 512 tokens, approximated by whitespace split)
  - Configurable overlap (default 20%)
- Output: prints each chunk with file path and byte offset

**Validation:** `vex "anything" src/` prints chunks from all discovered files.

---

### M1 — Embedding & Search (NPU-Accelerated)

**Goal:** Functional semantic search with a bundled model, accelerated by the Hexagon NPU.

**Deliverables:**
- ONNX model integration
  - Export `all-MiniLM-L6-v2` to ONNX (Python script in `scripts/export_model.py`)
  - **INT8 dynamic quantization** — aligns with Hexagon NPU's 45 TOPS INT8 throughput
  - Tokenizer: use `tokenizers` crate (HuggingFace's Rust tokenizer library) to run the WordPiece tokenizer natively
  - Model loading via `ort` crate with **DirectML execution provider**
  - **EP fallback chain:** DirectML (NPU/GPU) → CPU, selected by `--device` flag or auto-detected
- Embedding pipeline
  - Batch encode chunks (batch size 256, configurable)
  - Normalize embeddings to unit vectors
  - Encode query separately (single-item batch)
- Similarity search
  - Matrix multiply via `ndarray` (ARM64 NEON autovectorization, no external BLAS needed with 64 GB RAM)
  - Top-k extraction via partial sort (no need to fully sort)
  - Return results as `(chunk, file_path, byte_offset, score)` tuples
- Output formatting
  - Print results in grep-like format: `score  path:offset  preview...`
  - Highlight the matched chunk in context (show surrounding lines)
  - Color output via `termcolor` (disable with `--no-color`)
  - Machine-readable mode: `--json` flag

**Validation:** `vex "database connection pooling" src/` returns semantically relevant results, ranked by similarity score.

**Performance target:** 10k chunks in under 1 second on Snapdragon X Elite (NPU path). Sub-500ms target with INT8 quantized model on NPU.

---

### M2 — Smart Chunking

**Goal:** Replace naive chunker with language-aware strategies.

**Deliverables:**
- Chunking strategy trait
  ```rust
  trait Chunker: Send + Sync {
      fn chunk(&self, path: &Path, content: &str) -> Vec<Chunk>;
  }
  ```
- Tree-sitter chunker for code
  - Supported languages (initial): Rust, Python, TypeScript/JavaScript, Go, C/C++, Java
  - Split on function/method/class boundaries
  - Preserve the function signature as a prefix for context
  - Fall back to naive chunker for unsupported languages
- Prose chunker for text/markdown
  - Split on paragraph boundaries (double newline)
  - Respect markdown headers — start new chunk at each `##`+
  - Minimum chunk size to avoid embedding one-liners
- Context enrichment
  - Prepend file path and parent scope to each chunk before embedding
  - Format: `[path/to/file.rs::function_name] <chunk text>`
  - This dramatically improves embedding relevance

**Validation:** Searching for "authentication middleware" ranks a function named `verify_token` higher than a comment that mentions auth in an unrelated file.

---

### M3 — Performance

**Goal:** Make it fast enough to feel instant on typical codebases (<50k chunks).

**Deliverables:**
- Embedding cache
  - Cache doc embeddings to disk: `~/.cache/vex/<project_hash>/embeddings.bin`
  - Invalidation by file content hash (blake3) — only re-embed changed files
  - `--no-cache` flag to force full re-embed
  - Cache format: memory-mappable flat file (header + f32 matrix)
- Binary quantization mode
  - `--fast` flag: `sign(embedding)` → 384-bit binary vectors
  - Hamming distance via ARM64 NEON bit-count intrinsics (equivalent to x86 POPCNT)
  - 32× memory reduction, 10–20× search speedup
  - Accuracy drops ~5-10%, acceptable for exploratory search
- Parallel embedding
  - Use `rayon` to parallelize chunking across files
  - ONNX Runtime internal threading for model inference (configurable via `--threads`)
  - DirectML NPU handles batch inference while CPU handles chunking — natural pipeline overlap
- Matryoshka dimensionality reduction
  - If model supports it (e.g., `nomic-embed-text`), search first 64 dims for candidates, rescore with full 384
  - `--precision <low|medium|high>` flag

**Performance targets (Snapdragon X Elite, NPU via DirectML):**

| Corpus | Cold — NPU | Cold — CPU | Warm (cached) |
|--------|-----------|-----------|---------------|
| 1k chunks | <60ms | <100ms | <10ms |
| 10k chunks | <500ms | <1s | <15ms |
| 50k chunks | <2s | <4s | <50ms |
| 100k chunks | <4s | <8s | <100ms |

---

### M4 — Query Intelligence

**Goal:** Make queries smarter without requiring user effort.

**Deliverables:**
- Negative queries
  - `vex "error handling" --not "logging" src/`
  - Vector subtraction: `q_final = q_pos - 0.5 * q_neg`
  - Configurable weight via `--not-weight <0.0-1.0>`
- Pseudo-relevance feedback
  - `--expand` flag: run search, extract top terms from top-3 results, re-run
  - Term extraction via TF-IDF over the result set vs. corpus
  - Single re-ranking pass, no user interaction needed
- Multi-query fusion
  - `vex "auth,login,credentials" src/` — split on comma, embed each, max-pool per doc
  - Also accept `--also "related query"` for secondary queries
- Score thresholding
  - `--threshold 0.5` to suppress low-confidence results
  - Default: no threshold (show top-k regardless)
- Context window
  - `--context <n>` — show n lines above/below the matched chunk (like `grep -C`)

---

### M5 — Hybrid Search

**Goal:** Combine literal and semantic search for best-of-both-worlds.

**Deliverables:**
- Literal pre-filter
  - `--literal <hint>` — run ripgrep first, semantic-rank only matching files
  - Auto-detect: if query contains a likely identifier (camelCase, snake_case, ALL_CAPS), run literal search in parallel and boost results that match both
- Reciprocal rank fusion
  - Run literal search (BM25-style) and semantic search independently
  - Combine rankings via RRF: `score = Σ 1/(k + rank_i)` across systems
  - This handles queries like `"handleAuthError retry"` where you want both exact identifier match AND semantic understanding
- Regex filter
  - `--filter <regex>` — post-filter semantic results by regex on content
  - Useful for: `vex "error handling" --filter "fn |def |func "` (only show function definitions)

---

### M6 — Cross-Encoder Reranking

**Goal:** High-precision mode for when accuracy matters more than speed.

**Deliverables:**
- Cross-encoder reranker
  - Bundle `ms-marco-MiniLM-L-6-v2` as ONNX (separate model file)
  - `--rerank` flag: bi-encoder gets top 100, cross-encoder rescores to top-k
  - ~3-5× slower but significantly more accurate for nuanced queries
- Lazy model loading
  - Don't load the cross-encoder unless `--rerank` is specified
  - Download on first use: `vex --rerank` triggers one-time model download

---

### M7 — Polish & Distribution

**Goal:** Ship it.

**Deliverables:**
- Model management
  - `vex --download-model <name>` — fetch model from HuggingFace Hub
  - Default model bundled or auto-downloaded on first run
  - `vex --list-models` — show available models
  - Model storage: `%LOCALAPPDATA%/vex/models/` (Windows), `~/.local/share/vex/models/` (Unix)
- Shell completions
  - Generate for bash, zsh, fish, PowerShell via `clap_complete`
- Installation
  - `cargo install vex`
  - Prebuilt binaries via GitHub Actions for:
    - x86_64 Linux (glibc + musl)
    - aarch64 Linux
    - x86_64 macOS
    - aarch64 macOS (Apple Silicon)
    - x86_64 Windows
    - **aarch64 Windows** (primary dev target — Snapdragon X Elite)
  - Homebrew formula
  - **winget package**
- Documentation
  - README with usage examples, benchmarks, comparison to grep/rg/ag
  - `--help` text that's actually helpful
  - NPU acceleration guide for Windows on ARM

---

## Project Structure

```
vex/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE                    # MIT
│
├── src/
│   ├── main.rs                # CLI entry point, clap setup
│   ├── walk.rs                # File discovery
│   ├── chunk/
│   │   ├── mod.rs             # Chunker trait + dispatcher
│   │   ├── naive.rs           # Fixed-size sliding window
│   │   ├── treesitter.rs      # Syntax-aware (code)
│   │   └── prose.rs           # Paragraph-aware (text/markdown)
│   ├── embed/
│   │   ├── mod.rs             # Embedding trait
│   │   ├── onnx.rs            # ONNX Runtime backend
│   │   ├── tokenizer.rs       # WordPiece tokenization
│   │   └── quantize.rs        # Binary quantization
│   ├── search/
│   │   ├── mod.rs             # Search trait
│   │   ├── brute.rs           # Brute-force cosine similarity
│   │   ├── hamming.rs         # Binary vector search
│   │   └── rerank.rs          # Cross-encoder reranking
│   ├── query/
│   │   ├── mod.rs             # Query processing
│   │   ├── expand.rs          # Pseudo-relevance feedback
│   │   ├── negative.rs        # Negative query handling
│   │   └── hybrid.rs          # Literal + semantic fusion
│   ├── cache/
│   │   ├── mod.rs             # Cache management
│   │   └── embeddings.rs      # Embedding cache (mmap)
│   └── output/
│       ├── mod.rs             # Output formatting
│       ├── terminal.rs        # Colored terminal output
│       └── json.rs            # JSON output
│
├── scripts/
│   ├── export_model.py        # Export & quantize ONNX model
│   └── benchmark.py           # Benchmark against baselines
│
├── tests/
│   ├── integration/           # End-to-end CLI tests
│   └── fixtures/              # Sample files for testing
│
└── .github/
    └── workflows/
        ├── ci.yml             # Test + lint on PR
        └── release.yml        # Build + publish binaries
```

---

## Key Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
ignore = "0.4"                   # File walking (.gitignore aware)
tree-sitter = "0.24"             # Syntax parsing
tree-sitter-rust = "0.24"        # + language grammars
tree-sitter-python = "0.23"
ort = { version = "2", features = ["load-dynamic"] }  # ONNX Runtime + DirectML
tokenizers = "0.21"              # HuggingFace tokenizers
ndarray = "0.16"                 # No external BLAS — ARM64 NEON autovectorizes well
rayon = "1.10"
blake3 = "1"                     # Content hashing for cache
memmap2 = "0.9"                  # Memory-mapped files
termcolor = "1.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
dirs = "6"                       # Platform-appropriate data/cache dirs (Windows %LOCALAPPDATA%)
```

> **ARM64 note:** Dropped `blas-src`/OpenBLAS. On aarch64-pc-windows-msvc, ndarray's
> built-in loops autovectorize to NEON via LLVM. With 64 GB RAM and 384-dim embeddings,
> brute-force cosine sim is memory-bandwidth bound, not compute bound — external BLAS
> adds linking complexity for negligible gain at this vector size.
>
> **DirectML note:** The `ort` crate's `load-dynamic` feature loads `onnxruntime.dll` at
> runtime. To use the NPU, we need `DirectML.dll` and `onnxruntime-directml` variant.
> The Python export script will install `onnxruntime-directml` for model validation.

---

## CLI Interface

```
vex 0.1.0
Semantic grep — find code and text by meaning

USAGE:
    vex [OPTIONS] <QUERY> [PATHS...]

ARGS:
    <QUERY>       What to search for (natural language)
    [PATHS...]    Files or directories to search [default: .]

OPTIONS:
    -k, --top <N>              Number of results [default: 10]
    -t, --threshold <SCORE>    Minimum similarity score [0.0–1.0]
    -C, --context <LINES>      Lines of context around match [default: 2]
    -g, --glob <PATTERN>       Only search files matching glob
        --not <QUERY>          Exclude results similar to this
        --expand               Auto-expand query via pseudo-relevance feedback
        --rerank               Use cross-encoder for higher precision
        --fast                 Binary quantization (faster, less precise)
        --literal <HINT>       Pre-filter files by literal string
        --filter <REGEX>       Post-filter results by regex
        --no-cache             Don't use or write embedding cache
        --json                 Output as JSON
        --no-color             Disable colored output
        --threads <N>          Inference threads [default: num_cpus]
        --device <DEVICE>      Inference device: npu, gpu, cpu, auto [default: auto]
        --model <NAME>         Embedding model [default: minilm-l6-v2]
    -h, --help                 Print help
    -V, --version              Print version
```

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| ONNX Runtime linking is painful cross-platform | Blocks distribution | Use `load-dynamic` feature, bundle `onnxruntime.dll` per platform in CI |
| DirectML NPU support varies by driver version | NPU path silently broken | Auto-detect available EPs at startup, log which EP was selected, fallback to CPU |
| ARM64 Windows ecosystem gaps | Missing native builds of deps | Prefer pure-Rust deps; `ort` and `tree-sitter` both compile natively on aarch64-msvc |
| Model file size bloats the binary | Bad install UX | Auto-download on first run, cache in platform data dir (~23 MB for MiniLM) |
| Disk space tight on dev machine (~27 GB free) | Can't store many models | Default to single small model; lazy-download only when requested |
| Tree-sitter grammar coverage gaps | Bad chunking for niche languages | Fall back to prose chunker gracefully, accept community grammar PRs |
| Embedding quality varies by domain | Poor results on specialized codebases | Support swappable models via `--model`, document recommendations |
| Cache invalidation bugs | Stale results | Conservative: hash file content, not mtime. `--no-cache` escape hatch |

---

## Success Criteria

The tool is done when:

1. `vex "what retries on failure" src/` returns the right functions in a real codebase
2. Cold search over 10k chunks completes in under 1 second (NPU path on Snapdragon X Elite)
3. Warm search (cached embeddings) completes in under 50ms
4. `cargo install sem` works on Linux, macOS, Windows (x86_64 + ARM64)
5. NPU is auto-detected and used when available — zero configuration needed
6. Someone uses it instead of `grep -r` and doesn't go back