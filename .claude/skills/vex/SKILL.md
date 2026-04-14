---
name: vex
description: Semantic search for code and engineering context — find implementations, docs, and synced GitHub issues/PRs by meaning, not just keywords. Use when searching for concepts, understanding how things work, or locating code that grep can't find.
argument-hint: "<natural language query>"
disable-model-invocation: false
allowed-tools: Bash(vex *) Grep Read Glob
---

# Semantic Code Search

Search the codebase by meaning using `vex`, then drill into the results with follow-up queries and targeted grep. One vex call replaces 10-15 grep/glob/read calls — use it as your starting point, then narrow down.

## Step 1: Expand the query with synonyms

Before running vex, think about what vocabulary the code might use for this concept. Add `--literal` hints for 3-5 related technical terms to bridge vocabulary gaps between the query and the code.

| Query concept | Add as `--literal` |
|---|---|
| race conditions / thread safety | lock, mutex, semaphore, concurrent, atomic |
| sending notifications | email, smtp, push, notify, hub, message |
| background tasks / scheduling | job, worker, queue, cron, hangfire, hosted |
| authentication / login | auth, token, jwt, session, identity, oauth |
| caching | cache, redis, memory, invalidate, expire |
| error handling / resilience | exception, retry, fallback, polly, circuit |
| database queries | repository, query, entity, dbcontext, migration |
| API endpoints | controller, endpoint, route, handler, middleware |
| file upload / storage | blob, upload, stream, multipart, storage |
| logging / observability | logger, serilog, trace, telemetry, monitor |
| validation | validator, fluent, rule, constraint |
| permissions / authorization | role, policy, claim, authorize, permission |

## Step 2: Run vex

```bash
vex "$ARGUMENTS" --device cpu -k 10 --literal <term1> --literal <term2> --literal <term3>
```

## Step 3: Read top results

Read the top 3-5 files from vex output using Read. Focus on files scoring > 0.3.

## Step 4: Run follow-up queries

This is critical — don't stop at one vex call. The initial results reveal the codebase's vocabulary for this concept. Use that to run better follow-up searches:

**a) Rephrase from a different angle.** If the first query was abstract ("how are users notified"), try a concrete angle ("email sending service") or an implementation angle ("SignalR hub push message"):

```bash
vex "email sending service" --device cpu -k 5 --literal smtp --literal sendgrid
```

**b) Search for specific identifiers found in the initial results.** If vex found `WebhookDelivery.cs`, grep for its callers:

```bash
# Use Grep tool, not bash grep
Grep: WebhookDelivery
```

**c) Search for the interface/abstraction if you found an implementation** (or vice versa):

```bash
vex "webhook delivery abstraction" --device cpu -k 5 --literal IWebhook --literal delivery
```

**d) Narrow by file type** if results are noisy:

```bash
vex "$ARGUMENTS" --device cpu -k 10 -g "*.cs"
```

## Step 5: Trace the flow

Once you've found key files, trace the full flow by reading connected code:
- Found an endpoint? Read the service it calls.
- Found a service? Grep for what injects/uses it.
- Found a domain event? Find its handler.
- Found an interface? Find its implementations.

## Step 6: Summarize

Present findings as:
- Architecture overview (how the pieces connect)
- Key files and their roles (file:line)
- Patterns used (e.g., "outbox pattern for eventual consistency")
- Anything surprising or noteworthy

## Scoring guide

- **> 0.5**: Strong semantic match
- **0.3 - 0.5**: Moderate match — worth reading
- **0.2 - 0.3**: Weak — check only if other results are sparse
- **< 0.2**: Skip

## Searching synced GitHub issues and PRs

Vex can sync GitHub issues and PRs as local Markdown. If the project uses `vex sync github`, search these when the user asks about:
- Why something was built a certain way
- Bug history or prior reports
- Design decisions and debates
- Implementation rationale from PR discussions
- Epic/feature planning

### How to search synced issues

```bash
# Find the synced source path
ls "$LOCALAPPDATA/vex/sources/github/"  # Windows
ls ~/.local/share/vex/sources/github/   # Linux/Mac

# Search all synced content (issues + PRs)
vex "PII redaction strategy" "$LOCALAPPDATA/vex/sources/github/<owner>/<repo>/"

# Search only issues
vex "login timeout bug" "$LOCALAPPDATA/vex/sources/github/<owner>/<repo>/issues/"

# Search only PRs
vex "auth refactor" "$LOCALAPPDATA/vex/sources/github/<owner>/<repo>/prs/"
```

### When to sync first

If the user asks about issue discussions but you haven't synced yet, run:
```bash
vex sync github              # auto-detects repo from git remote
vex sync github owner/repo   # explicit repo
```

### Combining code + issue search

For a complete picture, search both the codebase AND synced GitHub artifacts:
1. Search code first: `vex "query" .`
2. Search issues: `vex "query" "$LOCALAPPDATA/vex/sources/github/<owner>/<repo>/"`
3. Combine findings — code shows the *what*, issues show the *why*

If code search results are weak, the explanation often lives in an issue discussion rather than in comments.

## Filtering & scoping options

Use these to narrow results and reduce noise:

**`-g "*.cs"` — Filter by file glob.** Essential when results mix code with docs/configs.
```bash
vex "validation logic" -g "*.cs" ...          # only C# files
vex "API routes" -g "*.ts" ...                # only TypeScript
vex "build config" -g "*.{yml,yaml,json}" ... # only config files
```

**`-t 0.3` — Minimum similarity threshold.** Cuts low-scoring noise. Use 0.3 for focused results, 0.2 for broader exploration.
```bash
vex "authentication" -t 0.35 ...   # only strong matches
```

**`-k N` — Number of results** (default 10). Use `-k 5` for focused searches, `-k 20` for broad exploration.

**`[PATHS]` — Restrict to specific directories.** Defaults to `.` (current dir).
```bash
vex "error handling" src/Services/ ...         # only search Services
vex "data access" src/Core/ src/CQRS/ ...     # search multiple dirs
```

**`--literal <term>` — Boost files containing this keyword.** Repeatable. Bridges vocabulary gaps between your query and the code. See the synonym table above.
```bash
vex "thread safety" --literal lock --literal mutex --literal concurrent ...
```

**`--hidden` — Include hidden files/dirs** (dotfiles). Useful for searching `.github/`, `.config/`, etc.

**`--no-gitignore` — Include gitignored files.** Useful for searching `node_modules/`, build output, or vendor directories.

## Output options

**`-C N` — Lines of context** around each match (default 2). Use `-C 5` for more surrounding code, `-C 0` for compact output.

**`--json` — JSON output.** Use when piping results to another tool or script:
```bash
vex "database queries" --json ... | jq '.[] | .file_path'
```

**`--no-color` — Plain text output.** Use when redirecting to a file or in CI.

## Performance options

**`--fast` — Binary quantization.** Uses sign-bit hashing for ~2x faster search at ~5-10% accuracy cost. Good for initial exploration on huge codebases.

**`--device cpu|npu|auto` — Inference device** (default auto, which uses CPU). NPU requires QNN runtime and pre-compiled context.

**`--threads N` — Inference threads** (default: all CPU cores).

**`--chunk-size N` — Chunk size in tokens** (default 512). Larger chunks give more context per result but fewer total chunks. Smaller chunks are more precise but may split relevant code.

**`--overlap 0.2` — Chunk overlap fraction** (default 0.2). Higher overlap reduces the chance of splitting relevant code across chunk boundaries.
