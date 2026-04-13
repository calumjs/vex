# vex

Semantic grep for code. Find things by meaning, not just text.

```
vex "error handling with retries" src/
```

```
0.4210  src/client.rs:47
  pub async fn fetch_with_retry(&self, url: &str) -> Result<Response> {
      for attempt in 0..self.max_retries {
          match self.client.get(url).send().await {

0.3891  src/worker.rs:112
  fn handle_failure(&mut self, task: Task) {
      if task.attempts < MAX_RETRIES {
          self.queue.push_back(task);
```

vex embeds your query and every chunk of code using a neural network, then ranks by cosine similarity. It understands what you mean, not just what you typed.

## Install

```bash
cargo install --git https://github.com/calumjs/vex
```

The model (~90 MB) downloads automatically on first run.

## Usage

```bash
# Search a directory
vex "database connection pooling" src/

# Top 3 results only
vex "authentication middleware" src/ -k 3

# Only search Rust files
vex "error handling" . -g "*.rs"

# JSON output for scripting
vex "config parsing" src/ --json

# Fast mode (binary quantization — less precise, but faster on huge codebases)
vex "logging" . --fast
```

## How it works

```
Query ──► Tokenize ──► Embed (ONNX) ──► Cosine Similarity ──► Ranked Results
                            |
Files ──► Chunk (tree-sitter) ──► Embed (ONNX) ──►──┘
                            |
                     Cache (blake3) ──► Skip re-embedding unchanged files
```

1. **Walk** files respecting `.gitignore`
2. **Chunk** using tree-sitter for code (function/class boundaries) or paragraph splitting for prose
3. **Embed** query and chunks with [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2) via ONNX Runtime
4. **Rank** by cosine similarity, return top-k
5. **Cache** embeddings keyed by file content hash — second search is instant

## Performance

Embedding is the bottleneck on cold runs. The cache eliminates it on subsequent searches.

| Scenario | Time (36 chunks) |
|----------|-----------------|
| Cold (first search, no cache) | ~1.5s |
| Warm (cached embeddings) | ~0.2s |

## NPU / GPU acceleration

On machines with DirectML support (Windows), vex auto-detects and uses hardware acceleration:

- **Qualcomm Hexagon NPU** (Snapdragon X Elite/Plus)
- **Any DirectML-compatible GPU**
- Falls back to CPU if neither is available

```bash
# Force a specific device
vex "query" src/ --device npu
vex "query" src/ --device gpu
vex "query" src/ --device cpu
```

## Supported languages (tree-sitter chunking)

Rust, Python, JavaScript, TypeScript, Go, C, C++, Java

Other file types fall back to paragraph/sliding-window chunking.

## Options

```
-k, --top <N>            Number of results [default: 10]
-t, --threshold <SCORE>  Minimum similarity score 0.0-1.0
-C, --context <LINES>    Lines of context around match [default: 2]
-g, --glob <PATTERN>     Only search files matching glob
    --fast               Binary quantization (faster, less precise)
    --no-cache           Skip embedding cache
    --json               JSON output
    --device <DEVICE>    npu, gpu, cpu, auto [default: auto]
    --model-dir <PATH>   Custom model directory
    --no-color           Disable colored output
```

## Advanced: custom models

The default model downloads automatically. To use a custom ONNX model:

```bash
# Export with the included script (requires Python + transformers)
python scripts/export_model.py --output-dir models/my-model

# Use it
vex "query" src/ --model-dir models/my-model
```

## License

MIT
