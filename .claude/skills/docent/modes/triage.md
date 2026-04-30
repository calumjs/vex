# Mode: `triage`

Process untriaged issues: suggest labels, flag potential duplicates, draft
clarifying comments. Does NOT automatically modify issues — produces a report
for the maintainer to act on.

## Preconditions

- `docent.config.json` exists.
- User explicitly invoked triage (this mode does not run on schedule by
  default — too opinionated for automatic behavior).

## Procedure

### Step 1 — Fetch candidate issues

Untriaged = no labels, OR has label `needs-triage`, OR opened in the last 7
days with no maintainer comment.

```bash
gh issue list --state open --limit 30 \
  --json number,title,body,labels,createdAt,author,comments
```

Filter to candidates in code. Skip issues the user has already interacted
with if possible.

### Step 2 — For each issue, analyze

For each candidate:

1. **Suggest labels.** Based on content, suggest 1–3 labels from the repo's
   existing label set. Get the label set with
   `gh label list --limit 100 --json name,description`.

2. **Check for duplicates.** Search existing issues for semantic duplicates:
   ```bash
   gh issue list --state all --search "{keywords from title}" --limit 10
   ```
   If any titles look related, flag them with a similarity score
   (your judgment, not a real metric).

3. **Check for completeness.** Does the issue include reproduction steps,
   environment info, expected vs. actual behavior? If missing, draft a
   short, friendly clarifying comment. Do NOT post it.

### Step 3 — Produce the report

Write a report to stdout (NOT to a file — this is not site content). Format:

```
Docent triage report — {date}
{N} issues analyzed

─────────────────────────────────────────────
Issue #142: "Login fails on Safari"
  Suggested labels: bug, browser-safari
  Possible duplicate: #98 "Safari login broken" (similarity: high)
  Completeness: missing Safari version
  Draft comment:
    > Thanks for reporting! Could you share which version of Safari
    > you're seeing this on? It looks like it might be related to #98.
─────────────────────────────────────────────
Issue #143: ...
```

### Step 4 — Do NOT modify issues

The report is the output. The maintainer decides what to do with it. This
is a hard rule: Docent must not post comments, add labels, or close issues.

If the user asks Docent to apply the changes, respond: "I've drafted these
actions but I'm designed not to modify issues automatically. You can copy
the suggested labels and comments from the report, or I can help you apply
them one by one if you confirm each."

## Exit conditions

- Report printed to user. No files written, no PRs opened.
