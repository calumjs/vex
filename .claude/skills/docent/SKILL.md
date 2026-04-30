---
name: docent
description: |
  Generates and maintains a public-facing static website for a GitHub repo.
  Use this skill when the user wants to set up Docent, update the Docent site,
  write a journal post or digest, generate release notes, triage new issues,
  or file feedback about Docent itself (the "suggest" mode).
  Also triggers on mentions of "docent site", "docs site update", operations
  on /docs/content/ in a repo that has docent.config.json, or phrases like
  "I want to suggest something about Docent" / "report a Docent bug".
---

# Docent

Docent is a skill that turns a GitHub repo into a public-facing static website.
The site lives in `/docs`, deploys to GitHub Pages, and is maintained by running
this skill periodically or on demand.

## How this skill works

Docent has multiple **modes**. Each mode is a self-contained procedure defined
in a file under `modes/`. You dispatch to a mode based on what the user asks
for and the current state of the repo.

| User intent signals | Mode | File |
|---|---|---|
| "set up Docent", "initialize Docent", no existing `docent.config.json` | `init` | `modes/init.md` |
| "update Docent", "refresh the site", scheduled run | `update` | `modes/update.md` |
| "write a journal post", "write a digest", "summarize recent work" | `digest` | `modes/digest.md` |
| "release notes for {tag}", "generate release notes", new tag pushed | `release` | `modes/release.md` |
| "triage issues", "look at new issues" | `triage` | `modes/triage.md` |
| "suggest", "file feedback", "I noticed something about Docent" | `suggest` | `modes/suggest.md` |

When invoked, follow this procedure:

1. **Identify the mode.** Read the user's request. If ambiguous, ask a single
   clarifying question. If the repo has no `docent.config.json` and the user
   didn't explicitly ask for `init`, ask whether they want to initialize.
2. **Read `docent.config.json`** (if it exists) to get project settings.
3. **Open the corresponding mode file** from `modes/`.
4. **Follow the mode's procedure exactly.** Each mode tells you what to read,
   what to write, and when to stop.

## Invariants (apply to every mode)

These rules apply regardless of which mode is running:

1. **Content boundary.** Only write to `/docs/content/`. The only exception is
   `init` mode, which also scaffolds `/docs/src/`, `/docs/public/`,
   `/docs/astro.config.mjs`, `/docs/package.json`, `/docent.config.json`, and
   `.github/workflows/docent-*.yml`. After `init`, those files are
   human-owned and must not be modified by the skill.

2. **PR-gated output.** Never push directly to the default branch. Always:
   - create a new branch named `docent/{mode}-{ISO-date}`
   - commit changes to it; **every commit subject must start with
     `Docent:`** — update mode's hand-edit detection uses this prefix
     to tell machine-authored commits from human edits
   - push the branch
   - open a pull request using `gh pr create`
   - report the PR URL back to the user

3. **No secrets in content.** If you encounter tokens, keys, or private email
   addresses in commits, issues, or PRs, redact them from generated content.

4. **Frontmatter attribution.** Every generated MDX file includes frontmatter
   fields `generatedBy: "docent"`, `generatedAt: "{ISO timestamp}"`, and
   `mode: "{mode name}"`.

5. **Idempotency.** If running the mode would produce no meaningful change
   (e.g., `update` with no new issues and no stale overview), exit without
   opening a PR. Report "nothing to update" to the user.

   The mechanism for this is **source anchors** recorded in generated
   files' frontmatter — `sourceCommit`, `sourceFiles`, `sourceSnapshot`.
   See `schemas/frontmatter.schema.json`. Modes compare anchors to
   current repo/external state and skip regeneration when they match.

6. **Deterministic ordering.** When listing things in generated content
   (issues, commits, releases), use stable ordering — chronological for
   time-based lists, alphabetical for label-based grouping.

7. **Theme is init-only.** `docs/content/theme.json` is written once,
   during `init` mode, and never modified by `update`, `digest`,
   `release`, or `triage`. Visual identity should not drift on scheduled
   runs. A maintainer who wants to re-theme invokes the analysis step
   explicitly ("Docent, re-analyze the design").

8. **Journal posts are immutable once written.** Posts under
   `docs/content/journal/*.mdx` are append-only regardless of their
   `mode` (`init`, `backfill`, or `digest`). Scheduled runs never
   rewrite existing posts — `digest` only adds new ones, `update`
   doesn't touch journal at all. A maintainer who wants to correct a
   post edits it by hand; Docent will respect the edit.

## Configuration

Read `docent.config.json` at the repo root. Schema is defined in
`schemas/config.schema.json`. Key fields:

- `project.name`, `project.owner`, `project.repo` — identity
- `tone` — one of `neutral`, `formal`, `playful`, `technical`; see
  `prompts/tone-presets.md`
- `sections` — booleans controlling which sections the site includes
- `journal.cadence` — `weekly`, `biweekly`, `monthly`, or `manual`
- `journal.minCommitsPerPost` — minimum activity threshold for auto-digests
- `status.excludeLabels` — issue labels to filter out of the status page

If `docent.config.json` is missing and the mode is not `init`, stop and ask
the user to run init first.

## Tools you will use

This skill relies entirely on tools Claude Code already has:

- `bash` for git operations and file manipulation
- `gh` CLI for GitHub API access (issues, PRs, releases)
- File read/write tools for editing content

You do not need any special runtime, SDK, or credentials beyond what Claude
Code and `gh` already require.

## When in doubt

- Ask the user one question, not five.
- Prefer opening a PR over pushing directly, always.
- Prefer structured content (JSON, MDX frontmatter) over prose-only output.
- If a mode's procedure doesn't cover a situation you encounter, describe
  what you found and ask the user how to proceed rather than improvising.
