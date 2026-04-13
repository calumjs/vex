# vex

**Vector Examine** — semantic grep for code. Find things by meaning, not just text.

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

## Why vex?

**Semantic search without indexing.** Most code search tools require you to build an index first, then query it. vex skips that entirely — point it at a directory, ask a question in plain English, get results in under a second. No setup, no database, no server, no background process.

It works because vex embeds your query and the code simultaneously at query time, using a neural network (all-MiniLM-L6-v2) running locally via ONNX Runtime. A BM25 pre-filter narrows candidates before the neural model runs, keeping latency under 1 second even on large codebases.

**Think `ripgrep` but for concepts.** `rg "retry"` finds the word "retry". `vex "error handling with retries"` finds `fetch_with_retry`, `handle_failure`, `CircuitBreakerPolicy`, and the design doc explaining the resilience strategy — even if none of them contain the word "retry".

## Install

### 1. Install vex

Requires [Rust](https://rustup.rs/).

```bash
cargo install --git https://github.com/calumjs/vex
```

Make sure `~/.cargo/bin` is on your PATH:

**Windows (PowerShell, run once):**
```powershell
[Environment]::SetEnvironmentVariable("Path", $env:USERPROFILE + "\.cargo\bin;" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")
```
Then restart your terminal.

**Linux / macOS:** Add to your shell config (`~/.bashrc`, `~/.zshrc`, etc.):
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

The embedding model (~23 MB quantized) downloads automatically on first run.

### 2. Install the Claude Code skill (optional)

vex ships with a `/vex` slash command for [Claude Code](https://claude.com/claude-code) that teaches Claude how to use vex effectively — including synonym expansion, follow-up queries, and flow tracing.

**Global install** (available in all projects):
```bash
# Linux/macOS
mkdir -p ~/.claude/skills/vex
cp .claude/skills/vex/SKILL.md ~/.claude/skills/vex/

# Windows (PowerShell)
New-Item -ItemType Directory -Force "$env:USERPROFILE\.claude\skills\vex"
Copy-Item .claude\skills\vex\SKILL.md "$env:USERPROFILE\.claude\skills\vex\"
```

Or just clone this repo — the skill is automatically available in any Claude Code session within it.

Then in Claude Code:
```
/vex error handling in authentication
/vex how does the billing system work
/vex "race conditions" --literal lock
```

The skill instructs Claude to:
- Expand queries with synonym `--literal` hints for vocabulary bridging
- Run follow-up queries from different angles
- Read top results and trace the code flow
- Combine vex with grep for a complete picture

## Usage

```bash
# Search the current directory
vex "database connection pooling"

# Search a specific directory
vex "authentication middleware" src/

# Top 3 results only
vex "config parsing" -k 3

# Only search C# files
vex "validation logic" -g "*.cs"

# Bridge vocabulary gaps — find locking code when searching for "race conditions"
vex "code that prevents race conditions" --literal lock --literal mutex

# Set a minimum similarity threshold
vex "error handling" -t 0.3

# More context around matches
vex "database migration" -C 5

# JSON output for scripting
vex "API endpoints" --json

# Fast mode (binary quantization — less precise, ~2x faster on huge codebases)
vex "logging" --fast

# Search hidden files and gitignored directories
vex "build config" --hidden --no-gitignore
```

## Sync GitHub issues as searchable files

Vex can sync GitHub issues and pull requests into local Markdown files, so you can search engineering discussions with the same workflow you use for code.

```bash
# Auto-detect repo from current directory
vex sync github

# Explicit repo
vex sync github calumjs/vex

# Sync issues and PRs, open only
vex sync github --include issues,prs --state open

# Search synced issues
vex "keyboard shortcuts" ~/.local/share/vex/sources/github/anthropics/claude-code/
```

Auth: vex uses `gh auth token`, `GITHUB_TOKEN`, or `GH_TOKEN`. Credentials are only needed for sync — search runs locally over the materialized Markdown files.

Subsequent syncs are incremental — only fetches items updated since the last sync.

## How it works

```
                    ┌─────────────────────────────────────────┐
  "retry logic" ──► │  1. Walk files (.gitignore-aware)        │
                    │  2. Score files by keyword matches        │
                    │  3. Chunk top files (tree-sitter / prose) │
                    │  4. BM25 pre-filter → top candidates     │
                    │  5. Neural embed (ONNX, INT8, 12 cores)  │
                    │  6. Cosine similarity → ranked results    │
                    └─────────────────────────────────────────┘
```

1. **Walk** files in parallel, respecting `.gitignore` and `.vexignore`
2. **Score** files by query keyword matches — skip files with no keyword overlap
3. **Chunk** the top 200 files using tree-sitter for code (function/class boundaries) or paragraph splitting for prose/markdown
4. **BM25 pre-filter** narrows to the top ~50 candidates lexically
5. **Embed** query + candidates with [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2) (384-dim, INT8 quantized) via ONNX Runtime on CPU
6. **Rank** by cosine similarity, return top-k

The key insight: steps 2-4 eliminate >99% of chunks before the neural model runs. This is why vex doesn't need pre-indexing — it's fast enough to embed at query time.

## Performance

Benchmarked on Snapdragon X Elite (12-core Oryon, ARM64):

| Codebase | Files | Chunks | Time |
|----------|-------|--------|------|
| Small (.NET app) | 585 | 3,227 | **468ms** |
| Large (monorepo) | 19,366 | 35,579 | **729ms** |

No pre-indexing. No cache. Cold search, every time.

## Supported languages (tree-sitter chunking)

Rust, Python, JavaScript, TypeScript, Go, C, C++, Java

Other file types fall back to paragraph/sliding-window chunking — vex works on any text file.

## Options

```
ARGUMENTS:
  <QUERY>                    Natural language search query
  [PATHS]                    Directories to search [default: .]

FILTERING:
  -k, --top <N>              Number of results [default: 10]
  -t, --threshold <SCORE>    Minimum similarity score 0.0-1.0
  -g, --glob <PATTERN>       Only search files matching glob
      --literal <TERM>       Boost files containing this keyword (repeatable)
      --hidden               Include hidden files/directories
      --no-gitignore         Don't respect .gitignore

OUTPUT:
  -C, --context <LINES>      Lines of context around match [default: 2]
      --json                 JSON output
      --no-color             Disable colored output

PERFORMANCE:
      --fast                 Binary quantization (faster, less precise)
      --no-cache             Skip embedding cache
      --device <DEVICE>      npu, cpu, auto [default: auto]
      --threads <N>          Inference threads [default: num_cpus]
      --chunk-size <N>       Chunk size in tokens [default: 512]
      --overlap <FRAC>       Chunk overlap fraction [default: 0.2]

MODEL:
      --model <NAME>         Embedding model [default: minilm-l6-v2]
      --model-dir <PATH>     Custom model directory
```

## License

MIT
