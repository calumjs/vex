# Mode: `digest`

Write a journal post summarizing recent repository activity.

**Manual invocation only.** The scheduled daily Routine uses `update`
mode, which decides for itself whether to write a post based on window
activity (see `update.md` Step 6). `digest` is for when a user
explicitly asks ("Docent, write a post about the auth refactor,"
"Docent, digest the last month") — scoped to a specific topic or time
window the user specifies.

A digest invocation bypasses the update-mode's "is this journal-worthy"
check. The user asked for a post, so write one — even if the activity
bar wouldn't clear on its own.

## Preconditions

- `docent.config.json` exists and `sections.journal` is `true`.
- `/docs/content/journal/` exists.
- User explicitly invoked this mode (not scheduled).

## Procedure

### Step 1 — Determine the time window

Find the most recent existing journal post:

```bash
ls -t docs/content/journal/*.mdx | head -1
```

Read its frontmatter `commitRange`. The window is from the end of that range
to `HEAD`. If no prior posts exist, use the last 30 days.

If the user specified a topic (e.g., "write a digest about the auth
refactor"), the window is the commits relevant to that topic — use `git log
--grep` or `--since` as appropriate.

### Step 2 — Check activity threshold

Count meaningful commits in the window (non-merge, non-trivial). If below
`journal.minCommitsPerPost` in config, and this is a scheduled run, exit
silently. If user-invoked, proceed anyway but warn the output may be thin.

### Step 3 — Gather activity

Collect:

```bash
# Merged PRs in window
gh pr list --state merged --search "merged:>{start-date}" --limit 50 \
  --json number,title,body,labels,mergedAt,author

# Closed issues
gh issue list --state closed --search "closed:>{start-date}" --limit 50 \
  --json number,title,labels,closedAt

# Commits (for authorship and volume, not content)
git log {start-ref}..HEAD --no-merges --format='%h|%an|%s'

# New tags
git tag --contains {start-ref} --sort=creatordate
```

### Step 4 — Identify themes

Load `prompts/journal-system.md`. Apply its guidance to group the collected
activity into coherent narratives. A good post has 2–5 themes, not a bulleted
list of every PR.

Examples of good themes:
- "Auth refactor lands" — groups 8 PRs touching the auth module
- "Performance sweep" — groups commits that collectively improve latency
- "Bug bash" — groups a burst of fix-type commits

### Step 5 — Write the post

Filename: `docs/content/journal/{YYYY-MM-DD}-{slug}.mdx`. Slug is kebab-case,
derived from the headline (e.g., `auth-refactor-and-bug-fixes`).

Frontmatter:

```yaml
---
title: "Headline"
date: "YYYY-MM-DD"
summary: "One-sentence subtitle."
tags: ["weekly"]       # or topic-specific tags
generatedBy: "docent"
generatedAt: "ISO timestamp"
mode: "digest"
commitRange: "abc123..def456"
---
```

Body: use the tone from config. Open with a single-sentence framing. Each
theme becomes a section (H2). Close with a short "coming up" paragraph if
there's clearly in-progress work (many open draft PRs, or a tagged roadmap
issue).

### Step 6 — Open PR

Branch: `docent/digest-{YYYY-MM-DD}`.
PR title: `Docent: journal post — {headline}`.
PR body: include the post's summary so the maintainer can skim.

## Tone guidance shortcut

- `neutral`: plain past tense, minimal adjectives. "Auth was refactored.
  Three bugs were closed."
- `formal`: third-person, newsletter-style.
- `playful`: first-person-plural, light humor, but still substantive.
- `technical`: include file paths, PR numbers inline, assume reader knows
  the stack.

## Exit conditions

- PR opened, or
- Exit silently if activity is below threshold on a scheduled run.
