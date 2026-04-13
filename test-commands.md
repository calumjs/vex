# vex — Test Commands

Run these from the project root (`C:\DataCalumSimpson\vex`).

## Setup

```bash
# Export the ONNX model (only needed once)
python scripts/export_model.py

# Build release binary
cargo build --release
```

## Basic search (against vex's own source)

```bash
# Semantic search — finds the most relevant code for a natural language query
./target/release/vex.exe "how does the embedding cache work" src/ -k 5

# Find file-walking logic
./target/release/vex.exe "file walking and directory traversal" src/ -k 3

# Find tokenization code
./target/release/vex.exe "tokenization and encoding text" src/ -k 3

# Find search/ranking logic
./target/release/vex.exe "search and ranking results by similarity" src/ -k 5
```

## Cache demo (run the same query twice)

```bash
# First run — cold cache, NPU embeds all chunks (~1.5s)
./target/release/vex.exe "error handling" src/ -k 3

# Second run — warm cache, skips embedding (~0.2s)
./target/release/vex.exe "error handling" src/ -k 3
```

## Fast mode (binary quantization)

```bash
# Uses sign-bit Hamming distance instead of cosine similarity
./target/release/vex.exe "error handling" src/ -k 3 --fast
```

## JSON output

```bash
./target/release/vex.exe "chunking strategy" src/ -k 3 --json
```

## Filter by file type

```bash
# Only search Python files
./target/release/vex.exe "export model" . -g "*.py" -k 3

# Only search Rust files
./target/release/vex.exe "cosine similarity" . -g "*.rs" -k 3
```

## Point at another codebase

```bash
# Replace the path with any project on your machine
./target/release/vex.exe "database connection pooling" C:\path\to\project\src\ -k 10
```

## Useful flags

```
-k, --top <N>           Number of results (default: 10)
-t, --threshold <SCORE>  Minimum similarity score 0.0–1.0
-C, --context <LINES>    Lines of context around match (default: 2)
-g, --glob <PATTERN>     Only search files matching glob
    --fast               Binary quantization (faster, less precise)
    --no-cache           Skip the embedding cache
    --json               Machine-readable output
    --device <DEVICE>    Force: npu, gpu, cpu, auto (default: auto)
    --no-color           Disable colored output
```
