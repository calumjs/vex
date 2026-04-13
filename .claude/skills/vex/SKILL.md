---
name: vex
description: Semantic code search — find code by meaning, not just keywords. Use when the user asks to find code related to a concept, wants to understand how something works across the codebase, or needs to locate implementations they can't find with grep.
argument-hint: "<natural language query>"
disable-model-invocation: false
allowed-tools: Bash(vex *)
---

# Semantic Code Search

Search the codebase by meaning using `vex`, an NPU-accelerated semantic grep tool.

## Step 1: Expand the query with synonyms

Before running vex, think about what vocabulary the code might use for this concept. The neural model handles semantic similarity, but file-level pre-filtering uses keywords — so adding `--literal` hints for synonyms dramatically improves recall.

**Always add `--literal` flags for related technical terms the code might use:**

| Query concept | Likely code terms to add as `--literal` |
|---|---|
| race conditions / thread safety | lock, mutex, semaphore, concurrent, atomic, synchronize |
| sending notifications | email, smtp, push, alert, notify, signal, hub, message |
| background tasks / scheduling | job, worker, queue, cron, hangfire, hosted, scheduler |
| authentication / login | auth, token, jwt, session, identity, credential, oauth |
| caching | cache, redis, memory, ttl, invalidate, expire |
| error handling | exception, catch, retry, fallback, resilience, polly |
| database queries | repository, query, sql, entity, dbcontext, migration |
| API endpoints | controller, endpoint, route, handler, middleware |
| file upload / storage | blob, upload, stream, multipart, storage, s3 |
| logging / observability | logger, serilog, trace, telemetry, diagnostic, monitor |
| dependency injection | service, inject, register, container, autofac, scope |
| validation | validator, fluent, rule, constraint, assert |
| testing | test, mock, fixture, assert, xunit, nunit, fake |
| deployment / CI/CD | pipeline, docker, kubernetes, helm, deploy, release |
| permissions / authorization | role, policy, claim, authorize, permission, access |

## Step 2: Run vex

```bash
vex "$ARGUMENTS" --no-cache --device cpu -k 10 --literal <synonym1> --literal <synonym2>
```

For example, if the user asks about "thread safety":
```bash
vex "thread safety" --no-cache --device cpu -k 10 --literal lock --literal mutex --literal semaphore --literal concurrent
```

## Step 3: Read top results

Read the top 3-5 files from vex output to understand the code in context. Use the file paths and line numbers provided.

## Step 4: Follow up with grep if needed

If vex finds a relevant area but you need more detail, use Grep to find specific identifiers, callers, or implementations in that area.

## Step 5: Summarize

Present findings as:
- What was found and where (file:line)
- Brief description of each relevant result
- Patterns or architectural insights discovered

## Scoring guide

- **> 0.5**: Strong semantic match — highly relevant
- **0.3 - 0.5**: Moderate match — likely relevant
- **0.2 - 0.3**: Weak match — tangentially related
- **< 0.2**: Noise — skip

## Additional options

- `-k N`: Number of results (default 10)
- `-g "*.cs"`: Filter by file glob
- `-C N`: Lines of context around matches
- `--json`: Machine-readable output
- `--fast`: Binary quantization (faster, slightly less precise)
