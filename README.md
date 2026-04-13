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

Requires [Rust](https://rustup.rs/).

```bash
cargo install --git https://github.com/calumjs/vex
```

Then make sure `~/.cargo/bin` is on your PATH:

**Windows (PowerShell, run once):**
```powershell
[Environment]::SetEnvironmentVariable("Path", $env:USERPROFILE + "\.cargo\bin;" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")
```
Then restart your terminal.

**Linux / macOS:** Add to your shell config (`~/.bashrc`, `~/.zshrc`, etc.):
```bash
export PATH="$HOME/.cargo/bin:$PATH"
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

# Bridge vocabulary gaps with --literal
vex "code that prevents race conditions" --literal lock --literal mutex

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

Benchmarked on Snapdragon X Elite (12-core Oryon, ARM64):

| Codebase | Files | Chunks | Time |
|----------|-------|--------|------|
| Small (.NET app) | 585 | 3,227 | **468ms** |
| Large (monorepo) | 19,366 | 35,579 | **729ms** |

Uses BM25 pre-filtering + neural re-ranking: only the top candidates get embedded, keeping latency under 1 second regardless of codebase size.

## Claude Code integration

vex ships with a skill plugin for [Claude Code](https://claude.com/claude-code). Use `/vex` to search by meaning from any Claude session.

**Project-level** (automatic for anyone cloning this repo):
```
.claude/skills/vex/SKILL.md  ← already included
```

**Global install** (available in all projects):
```bash
# Linux/macOS
mkdir -p ~/.claude/skills/vex
cp .claude/skills/vex/SKILL.md ~/.claude/skills/vex/

# Windows (PowerShell)
New-Item -ItemType Directory -Force "$env:USERPROFILE\.claude\skills\vex"
Copy-Item .claude\skills\vex\SKILL.md "$env:USERPROFILE\.claude\skills\vex\"
```

Then in Claude Code:
```
/vex error handling in authentication
/vex database connection pooling
/vex "race conditions" --literal lock
```

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
    --literal <TERM>     Boost files containing this keyword (repeatable)
    --fast               Binary quantization (faster, less precise)
    --no-cache           Skip embedding cache
    --json               JSON output
    --device <DEVICE>    npu, cpu, auto [default: auto]
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
