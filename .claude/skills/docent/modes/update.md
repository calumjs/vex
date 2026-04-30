# Mode: `update`

The daily routine. Refreshes status / overview / changelog when their
sources have changed, and — on days where enough interesting activity
has landed — also writes a new journal post summarizing the period.
One mode, one Routine, adaptive output.

Runs on a daily schedule, or when the user says "update Docent" /
"refresh the site."

## Outputs at a glance

Depending on what's happened since the last run, one update can produce:

1. **Nothing.** No anchors drifted and no journal-worthy activity → no
   PR, silent exit (honors SKILL.md invariant 5 idempotency).
2. **Content refresh only.** Status/overview/changelog anchors drifted
   but recent activity isn't substantial enough for a journal post →
   one PR with the stale content regenerated.
3. **Content refresh + journal post.** Anchors drifted AND enough has
   happened to tell a story → one PR combining both.
4. **Journal post only** (rare). Anchors fresh but a lot has shipped
   (e.g., all in the same day, all touching things unrelated to
   README / issues / tags) → one PR with just the new journal post.

Journal posts never overwrite existing posts; they always append.
Existing posts are immutable (SKILL.md invariant 8).

## Preconditions

- `docent.config.json` exists. If not, direct the user to `init` mode.
- `/docs/content/` exists.

## Procedure

### Step 1 — Read config

Load `docent.config.json`. Note:
- Which sections are enabled.
- `status.excludeLabels`.
- `tone`.

### Step 2 — Decide what, if anything, to regenerate

Each content file records a **source anchor** in its frontmatter /
top-level JSON (see `init.md` Step 7 and `schemas/frontmatter.schema.json`).
Compare the recorded anchor to the current repo state; only regenerate
files whose anchors are stale.

Before regenerating any file, check whether the file has been edited by
hand since Docent last wrote it. **Two signals must BOTH agree that the
file is machine-owned** — a single spoofable signal is not enough:

**Signal 1: working tree clean + no non-Docent commits _since the last
Docent-authored commit touching the file_.**

```bash
git diff --quiet HEAD -- docs/content/{file}             # 1 = dirty

# Find the most recent Docent-authored commit touching this file.
# That commit is the reference: it wrote the current anchors, so any
# commits AFTER it must also be Docent's or the file is co-owned.
LAST_DOCENT=$(git log --pretty=format:"%H %s" -- docs/content/{file} \
  | grep -E "^[a-f0-9]+ Docent:" | head -1 | cut -d' ' -f1)

# Non-Docent commits since then (empty = machine-owned)
git log --pretty=format:"%s" "${LAST_DOCENT}..HEAD" -- docs/content/{file} \
  | grep -v "^Docent:" | head -1
```

Skip regeneration if the working tree is dirty OR there is any non-Docent
commit in the file's history *after* the last Docent-authored commit.
Commits BEFORE the last Docent commit don't matter — Docent's own write
supersedes them.

Rationale: the rule treats "what Docent most recently put in the file"
as the reference point. Pre-invariant history doesn't poison the file
forever; as soon as Docent writes with a `Docent:` prefix, the clock
resets. The attack vector (a human spoofing the prefix after a Docent
write) is caught by Signal 2 for MDX files.

**Edge case: no Docent commit in history.** If `git log | grep "^Docent:"`
returns nothing, the file has never been Docent-authored with a proper
prefix. Treat as co-owned — skip regeneration. The user should re-run
`init` (or manually prefix a future Docent commit) to establish the
reference.

**Signal 2 (MDX only): body hash matches what Docent wrote.**

Applies to MDX files with a `bodyHash` field in frontmatter. Does NOT
apply to JSON files — status.json has no separate "body" to hash, and
its `sourceSnapshot.issueSetHash` is a *freshness* signal (derived
from GitHub state) rather than a *content* signal (hash of what's on
disk). For JSON, Signal 1 alone is the ownership check.

Compute the SHA-256 of the MDX body (everything after the closing `---`
of the frontmatter) and compare to the recorded `bodyHash`:

```bash
awk 'BEGIN{n=0} /^---$/{n++; next} n>=2 {print}' docs/content/overview.mdx | \
  sha256sum | awk '{print $1}'
```

If the current body hash differs from the recorded `bodyHash`, the file
was edited — skip regardless of what the commit log says. This catches
the case a human accidentally (or deliberately) used a `Docent:` commit
subject: the hash won't match.

**For MDX files, both signals must pass** for machine-ownership. For
JSON files, Signal 1 is sufficient. Either failing → skip. Defense in
depth for MDX: commits can be subject-spoofed, but you can't spoof the
hash unless you carefully re-hash and rewrite the frontmatter, which is
well past "accidental."

### Step 2b — Regeneration writes fresh anchors

When Docent regenerates a file, it must:
1. Write the new body.
2. Compute the body's SHA-256.
3. Write the full file with the hash in frontmatter (`bodyHash` for MDX,
   inline in JSON's `sourceSnapshot`). This means writing twice or
   buffering — a minor implementation cost for a load-bearing check.

### Step 3 — Check `status.json` freshness

Read `docs/content/status.json`'s `sourceSnapshot.issueSetHash`. Fetch
the current state from GitHub:

```bash
gh issue list --state open --limit 200 \
  --json number,title,body,labels,updatedAt,url,assignees,state
```

Filter out excluded labels. Then compute the current issue-set hash:

1. For each remaining issue, produce a tuple
   `{number, updatedAt, state, labels[sorted]}`.
2. Sort the tuple list by `number`.
3. Serialize as canonical JSON (UTF-8, sorted keys, no whitespace).
4. SHA-256 of the serialization.

Compare to `sourceSnapshot.issueSetHash`. **Regenerate if and only if
the hashes differ.** Do NOT gate on `openIssueCount` and
`newestIssueUpdatedAt` alone — those can collide across different issue
sets (one issue closing while another opens with no newer timestamp
leaves both unchanged but the groupings stale).

The hash is the authoritative signal. The `openIssueCount` top-level
field is retained for the page template and humans, not for freshness
detection.

If regenerating, follow `init.md` Step 7's `status.json` procedure and
write a fresh `sourceSnapshot` with the new hash.

### Step 4 — Check `overview.mdx` freshness

Read `docs/content/overview.mdx`'s frontmatter. Extract the
`sourceFiles` array.

For each entry, run `git hash-object <path>` and compare to the
recorded `sha`. If any differ, the overview is stale and should be
regenerated.

Also refresh unconditionally if `generatedAt` is older than 90 days —
the README may not have changed but the phrasing should get a fresh
pass occasionally.

If regenerating, follow `init.md` Step 7's `overview.mdx` procedure and
write fresh `sourceFiles` entries.

### Step 5 — Check `changelog.mdx` freshness

Full tag-set reconciliation — not just the newest tag. Older tags can
be added mid-history (backported releases, annotated tag fixes), and
we must catch them all or the changelog silently drifts incomplete.

```bash
git tag --sort=-creatordate                 # all tags, newest first
```

Extract the set of tags already represented in `changelog.mdx` (each
entry opens with `## <tag> — <date>` per SPEC §4.4). Let:

- `allTags` = set from `git tag`
- `changelogTags` = set parsed from the file

**Regenerate if `allTags \ changelogTags` is non-empty** — any missing
tag triggers a run.

For each missing tag, run the `release` mode procedure (not opening a
separate PR; fold the entries into this update PR, inserted in correct
chronological position based on tag date).

Update the frontmatter `sourceCommit` to the current short HEAD after
writing. This records the commit Docent reconciled against; a future
run comparing `sourceCommit` to HEAD is a cheap first-pass check
before doing the expensive tag-set diff.

### Step 6 — Decide whether to write a journal post

Journal posts land on days when something worth reading about has
happened, not on a fixed cadence. Use judgment — the thresholds below
are suggestions, not rules.

**Gather the window.**

Find the most recent `docs/content/journal/*.mdx` post. Its frontmatter
`commitRange` ends at some SHA; the window is from there to current
`HEAD`. If no prior posts exist, use the most recent 7 days of commits.

```bash
# Newest journal post (filename is ISO-dated, so `ls -r` is chronological)
ls -r docs/content/journal/*.mdx | head -1

# Activity in the window
gh pr list --state merged --search "merged:>{start-date}" --limit 50 \
  --json number,title,body,labels,mergedAt,author

gh issue list --state closed --search "closed:>{start-date}" --limit 50 \
  --json number,title,labels,closedAt

git log {start-ref}..HEAD --no-merges --format='%h|%an|%s'

git tag --contains {start-ref} --sort=creatordate
```

**Decide whether to post.**

Rough guidance (adjust based on the shape of the project — a repo that
ships one PR a quarter vs. one a day has very different "interesting"
bars):

- **Likely post**: ≥ 3 merged PRs OR ≥ 10 non-merge commits in the
  window, AND the window spans at least ~2 days of activity.
- **Likely skip**: < 3 merged PRs AND < 10 commits, OR everything is
  purely internal (dependency bumps, whitespace, one-char typo fixes),
  OR the last journal post was less than 2 days ago.
- **Definitely post**: a new release tag appeared in the window (covered
  separately by `release` mode's announce flag, but still — a tagged
  release always deserves at least a mention).
- **Definitely skip**: nothing merged, nothing closed, no tags, fewer
  than 3 meaningful commits.

These are *suggestions*. Ultimately: if you can write a post with 2–5
coherent themes that a non-contributor would find worth reading, write
it. If the best you can manage is "some small fixes shipped," skip.
Honest silence beats padded prose.

**Also consider cadence.** If the last post was yesterday, raise the
bar significantly — an active project shouldn't produce daily journal
posts. "≥ 2 days since the last post" is a soft floor unless something
genuinely large landed in the last 24h (big release, major refactor,
significant incident).

**If the decision is skip**, skip Step 6's write — the update still
proceeds with whatever content regeneration it found. If the decision
is post, continue.

**Write the post.**

Follow `prompts/journal-system.md` for voice and structure. Filename:
`docs/content/journal/{YYYY-MM-DD}-{slug}.mdx`. Frontmatter:

```yaml
---
title: "Specific headline"
date: "{YYYY-MM-DD}"
summary: "One-sentence subtitle."
tags: ["daily"]                    # or topic-specific tags
generatedBy: "docent"
generatedAt: "{ISO timestamp}"
mode: "digest"
commitRange: "{start-sha}..{HEAD-sha}"
bodyHash: "{computed per init.md's bodyHash procedure}"
---
```

Use `mode: "digest"` for posts written this way — preserves continuity
with the (now manually-invoked) digest mode's output shape, and the
journal index template treats both identically.

Include the new post in the same update PR as any content refresh.

### Step 7 — If nothing changed, exit

If `status.json`, `overview.mdx`, and `changelog.mdx` were all left
alone AND no journal post was written, do nothing. Report "Docent:
nothing to update" and exit without opening a PR. This honors SKILL.md
invariant 5 (idempotency).

Running `update` immediately after `init` on an unchanged repo MUST be
a no-op PR-wise. The anchors are the mechanism.

### Step 8 — Otherwise, commit and open PR

Branch: `docent/update-$(date -u +%Y-%m-%d)`.

PR title: one of:
- `Docent: update {YYYY-MM-DD}` — content refresh only
- `Docent: journal post — {headline}` — journal post only
- `Docent: update + journal post — {headline}` — both

PR body: bullet list of what changed and why (which anchor drifted,
whether a journal post was added and on what basis).

```bash
git checkout -b docent/update-$(date -u +%Y-%m-%d)
git add docs/content/
git commit -m "Docent: {update|journal post|update + journal post}"
git push -u origin HEAD
gh pr create --title "..." --body "..."
```

The `Docent:` commit-subject prefix is load-bearing — Step 2's
hand-edit detection uses it to distinguish machine-authored commits
from human ones. All Docent-authored commits must start with that
prefix.

## Exit conditions

- PR opened containing any combination of content refresh + journal post, or
- No-op reported if everything was fresh AND no journal post was warranted.

## Never write to theme.json

`docs/content/theme.json` is init-only (SKILL.md invariant 7). This
mode must not read it, write it, or regenerate it. If a maintainer
wants to re-theme, they run a separate "re-analyze design" path — not
wired into update.
