---
name: vex
description: Semantic code search — find code by meaning, not just keywords. Use when the user asks to find code related to a concept, wants to understand how something works across the codebase, or needs to locate implementations they can't find with grep.
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
vex "$ARGUMENTS" --no-cache --device cpu -k 10 --literal <term1> --literal <term2> --literal <term3>
```

## Step 3: Read top results

Read the top 3-5 files from vex output using Read. Focus on files scoring > 0.3.

## Step 4: Run follow-up queries

This is critical — don't stop at one vex call. The initial results reveal the codebase's vocabulary for this concept. Use that to run better follow-up searches:

**a) Rephrase from a different angle.** If the first query was abstract ("how are users notified"), try a concrete angle ("email sending service") or an implementation angle ("SignalR hub push message"):

```bash
vex "email sending service" --no-cache --device cpu -k 5 --literal smtp --literal sendgrid
```

**b) Search for specific identifiers found in the initial results.** If vex found `WebhookDelivery.cs`, grep for its callers:

```bash
# Use Grep tool, not bash grep
Grep: WebhookDelivery
```

**c) Search for the interface/abstraction if you found an implementation** (or vice versa):

```bash
vex "webhook delivery abstraction" --no-cache --device cpu -k 5 --literal IWebhook --literal delivery
```

**d) Narrow by file type** if results are noisy:

```bash
vex "$ARGUMENTS" --no-cache --device cpu -k 10 -g "*.cs"
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

## Quick reference

```
-k N              Number of results (default 10)
-g "*.cs"         Filter by file glob
-C N              Lines of context around matches
--literal <term>  Boost files containing this keyword (repeatable)
--fast            Binary quantization (faster, slightly less precise)
--json            Machine-readable output
```
