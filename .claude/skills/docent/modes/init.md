# Mode: `init`

First-run scaffolding. Run this when the user asks to "set up Docent" or similar
in a repo that does not yet have `docent.config.json`.

## Preconditions

- Current directory is the root of a git repository.
- Repo has a remote on GitHub (check with `git remote -v`).
- `docent.config.json` does NOT already exist. If it does, suggest running
  `update` mode instead.
- `/docs/content/` does NOT already exist. If it does, ask the user how to
  proceed before overwriting.

## Procedure

### Step 1 — Gather repo metadata

Run these commands and collect results:

```bash
git remote get-url origin              # parse owner and repo
git symbolic-ref refs/remotes/origin/HEAD  # default branch
cat README.md                          # if exists, for overview generation
ls -la                                 # detect language markers (package.json,
                                       # Cargo.toml, pyproject.toml, go.mod)
git log --oneline -20                  # recent commits
gh repo view --json name,description,licenseInfo,primaryLanguage
```

### Step 2 — Ask the user for choices

Ask these in ONE message, not sequentially. Provide defaults so the user
can say "defaults are fine":

1. **Tone** — neutral (default), formal, playful, or technical?
2. **Journal cadence** — weekly (default), biweekly, monthly, or manual only?
3. **Backfill journal from commit history?** — one of:
   - **yes, up to 12 posts** (default) — partitions git history into
     cadence-aligned buckets and writes one dated post per bucket, up
     to a cap of 12. Oldest buckets collapse into an "Early history"
     roundup so cost stays bounded on long-history repos.
   - **yes, custom cap** — user supplies an integer ≥ 2. Minimum of
     2 because the collapse algorithm needs at least one kept bucket
     plus a rollup slot.
   - **no, single welcome post** — `journal.backfill: false` in the
     generated config. Docent writes one inaugural post spanning the
     whole history.
4. **Custom domain?** — default none; the site will deploy to
   `{owner}.github.io/{repo}`.

Do NOT ask about section toggles at init; enable all sections by default.
Users can disable sections later by editing `docent.config.json`.

Record the user's backfill choice so Step 4's generated
`docent.config.json` reflects it:
- "yes, up to 12": `journal.backfill = true`, `journal.backfillLimit = 12`
- "yes, custom N": `journal.backfill = true`, `journal.backfillLimit = N`
- "no": `journal.backfill = false`, `journal.backfillLimit = 12` (default retained but unused)

### Step 3 — Create a working branch

```bash
git checkout -b docent/init-$(date -u +%Y-%m-%d)
```

### Step 4 — Write `docent.config.json`

Use values from Step 1 and Step 2. Schema in
`${CLAUDE_PLUGIN_ROOT}/skills/docent/schemas/config.schema.json`.

The generated `docent.config.json` is written into the user's repo, so
its `$schema` field cannot reference `${CLAUDE_PLUGIN_ROOT}` (that
expands at runtime inside the plugin, not in files users edit). Point
at the raw-URL on GitHub instead — stable and IDE-friendly.

Default contents:

```json
{
  "$schema": "https://raw.githubusercontent.com/calumjs/docent/master/skills/docent/schemas/config.schema.json",
  "project": {
    "name": "{{repo name, human-cased}}",
    "owner": "{{github owner}}",
    "repo": "{{github repo}}",
    "homepage": "https://{{owner}}.github.io/{{repo}}"
  },
  "tone": "{{chosen tone}}",
  "sections": {
    "overview": true,
    "journal": true,
    "status": true,
    "changelog": true,
    "bugReport": true,
    "featureRequest": true
  },
  "journal": {
    "cadence": "{{chosen cadence}}",
    "announceReleases": true,
    "minCommitsPerPost": 3,
    "backfill": {{backfill choice true/false from Step 2}},
    "backfillLimit": {{backfill cap from Step 2, default 12, minimum 2}}
  },
  "status": {
    "groupStrategy": "auto",
    "excludeLabels": ["wontfix", "duplicate", "invalid"]
  },
  "deploy": {
    "target": "github-pages",
    "customDomain": {{null or "example.com"}}
  }
}
```

### Step 5 — Copy the site template

```bash
mkdir -p docs
cp -r "${CLAUDE_PLUGIN_ROOT}/skills/docent/templates/site/." docs/
```

Then generate `docs/package-lock.json` so the deploy workflow's
`cache: npm` + `npm ci` don't fail on first push:

```bash
npm install --prefix docs --ignore-scripts
```

Use `--prefix docs` rather than `cd docs && npm install && cd ..` —
harnesses vary on whether cwd persists between bash calls, and any
chain of `cd` instructions risks leaving the shell in `docs/` for the
next step. `--prefix` is a single, cwd-independent invocation.

`--ignore-scripts` avoids running postinstall hooks from template deps;
we only need the lock file, not a functional install. The resulting
`docs/node_modules/` stays gitignored.

The template is generic. It reads `docent.config.json` (site URL, project
name, repo owner, section toggles) and `docs/content/theme.json` (vibe,
accent colors, hero image) at build time — there are no `{{PLACEHOLDER}}`
values to substitute. If the user wants a distinct `package.json` name
for the site, optionally rename `"docent-site"` to `"{repo}-docs"`.

### Step 6 — Copy the deploy workflow

Docent content is regenerated by Claude Code Routines (§6.1), not CI. The
only GitHub Actions file Docent installs is the Pages deploy workflow:

```bash
mkdir -p .github/workflows
cp "${CLAUDE_PLUGIN_ROOT}/skills/docent/templates/workflows/docent-deploy.yml" .github/workflows/
```

The template's `on.push.branches` is `[main]` — the common default. If
the repo's default branch (detected in Step 1) is something else (e.g.
`master`, `trunk`, `develop`), substitute it in the copied file. Edit
`.github/workflows/docent-deploy.yml`, not the template under
`templates/workflows/`.

Do NOT copy `docent-update.yml` or `docent-digest.yml` — those are
optional CI fallbacks (§6.6) for users who can't or won't use Routines.
Users opt in by copying them manually later.

### Step 7 — Generate initial content

Create `docs/content/` and fill it:

Every generated file records one or more **source anchors** so
`update` mode can detect staleness without re-running the expensive
generation step. MDX files also record a `bodyHash` (SHA-256 of the
body, excluding frontmatter) so `update` can detect hand edits
independent of the git commit log.

| Content | Derived from | Anchor fields |
|---|---|---|
| `overview.mdx` | `README.md` | `sourceFiles: [{ path: "README.md", sha: ... }]`, `bodyHash` |
| `status.json` | GitHub Issues API | `sourceSnapshot: { issueSetHash, openIssueCount }` |
| `changelog.mdx` | git tags + HEAD | `sourceCommit: <short HEAD sha>`, `bodyHash` |
| `journal/*.mdx` | commit range | `commitRange: <first>..<last>`, `bodyHash` |

See `schemas/frontmatter.schema.json` for exact shapes.

**`docs/content/overview.mdx`**
- Read `README.md` and any obvious structural hints (top-level directories,
  language manifests).
- Load tone guidance from `prompts/tone-presets.md` based on config.
- Load the overview-writing system prompt from `prompts/overview-system.md`.
- Produce 300–800 words explaining what the project is, who it's for, and
  how someone gets started. Prioritize accessibility for non-contributors.
- **Anchors**: record `sourceFiles: [{ path: "README.md", sha: <output of `git hash-object README.md`> }]` AND `bodyHash: <SHA-256 of the MDX body>` in frontmatter. See the **Computing bodyHash** note below.

**`docs/content/status.json`**
- Fetch open issues: `gh issue list --state open --limit 100 --json number,title,labels,updatedAt,url`.
- Filter out issues with labels in `status.excludeLabels` from config.
- Load the status-summarizing system prompt from `prompts/status-system.md`.
- Group issues into sensible categories (Bugs, In progress, Feature
  requests, Other) based on labels and content. Write the JSON per the schema
  in `schemas/status.schema.json`.
- **Anchor**: compute an `issueSetHash`:
  1. For each filtered issue, produce tuple `{number, updatedAt, state, labels[sorted]}`.
  2. Sort tuple list by `number`.
  3. Serialize as canonical JSON (UTF-8, sorted keys, no whitespace).
  4. SHA-256 of the serialization.

  Include `sourceSnapshot: { issueSetHash, openIssueCount }` as a
  top-level object in `status.json`. The hash is the authoritative
  freshness signal; the count is retained for human readability.

  Empty-state case (zero issues after filtering): `issueSetHash` is
  the SHA-256 of the literal string `[]`
  (`4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945`)
  — deterministic, matches whenever the set stays empty.

**`docs/content/changelog.mdx`**
- Fetch tags: `git tag --sort=-creatordate`.
- For each tag (newest first, up to 10), collect commits between it and the
  previous tag: `git log {prev}..{tag} --oneline --no-merges`.
- Write release entries using the template in SPEC §4.4.
- If there are no tags, write a placeholder: "No tagged releases yet."
- **Anchors**: record `sourceCommit: <output of `git rev-parse --short HEAD`>` AND `bodyHash: <SHA-256 of the MDX body>` in frontmatter.

**`docs/content/journal/*.mdx` — backfill posts**

Journal generation branches on `journal.backfill` in config.

**Runtime fallback rules (important).** JSON Schema `default` fields
are annotations — most validators do NOT substitute them at parse
time. So `init` and `update` MUST apply these defaults explicitly
when reading config:

- If `journal.backfill` is missing → treat as `true`.
- If `journal.backfillLimit` is missing → treat as `12`.
- If `journal.backfillLimit` is present and less than 2 → reject
  the config with a specific error pointing the user at the schema
  constraint. Do NOT silently clamp; that hides the misconfiguration.

The `backfillLimit: 1` case is rejected specifically because the
collapse algorithm (step 4 below) keeps `backfillLimit - 1` recent
buckets and dates the rollup relative to the oldest kept one — with
zero kept buckets, the rollup has no reference point. Users who want
a single post should set `backfill: false` (which writes one welcome
post) rather than `backfillLimit: 1`.

**If `journal.backfill === true`** (default):

1. **Walk history chronologically**:
   ```bash
   git log --reverse --no-merges --format='%H|%aI|%s'
   ```
2. **Bucket commits by cadence** (from `journal.cadence`):
   - `weekly`: week starts Monday 00:00 in the author's timezone (use
     commit's `%aI` date). Group commits by ISO week.
   - `biweekly`: every other Monday; pair adjacent weeks.
   - `monthly`: first of month.
   - `manual`: fall back to `weekly` for bucketing; the cadence field
     only governs scheduled digests, not backfill.
3. **Drop buckets with fewer than `journal.minCommitsPerPost` commits**
   — no "quiet week" filler for dead periods.
4. **Apply the cap**. If more than `journal.backfillLimit` buckets
   remain, keep the most recent `backfillLimit - 1` buckets as
   individual posts and collapse all older buckets into one "Early
   history" roundup post (date = last commit of the oldest kept
   bucket minus one second, so it sorts first chronologically).
5. **For each kept bucket**, generate one post:
   - Filename: `{YYYY-MM-DD}-{slug}.mdx` where date is the bucket's
     last commit date and slug is kebab-case derived from the post's
     headline (the model picks a headline; see
     `prompts/journal-system.md` §Inaugural-and-backfill).
   - Frontmatter:
     ```yaml
     date: "{last-commit-date}"
     mode: "backfill"
     commitRange: "{first-sha}..{last-sha}"
     generatedBy: "docent"
     generatedAt: "{ISO timestamp}"
     bodyHash: "{SHA-256 of body, computed per the procedure below}"
     ```
   - Apply the **Inaugural and backfill posts** section of
     `prompts/journal-system.md`. These posts are retrospective
     reconstructions, not contemporaneous reporting — frame them
     honestly ("Looking at the commits from this period, …").
   - The oldest backfill post implicitly opens the archive; do NOT
     also write a separate welcome/inaugural post.

**If `journal.backfill === false`**:

Write a single `docs/content/journal/{YYYY-MM-DD}-inaugural.mdx` using
the "project so far" approach — whole-repo-history scope, welcoming
first-visitor voice, `mode: "init"`, `commitRange: {first-sha}..{HEAD-sha}`,
plus `bodyHash`. Follow the **Inaugural and backfill posts** section
of `prompts/journal-system.md` — retrospective, honest framing.

**Why a cap matters**: cost scales linearly with bucket count. A
5-year repo on weekly cadence is 260 Claude calls uncapped. With
`backfillLimit: 12` it's 13 calls regardless of repo age.

**Honesty with history**: force-pushed or rebased commits produce
timestamps that don't reflect when work actually happened. Don't try
to work around this; just note it in the init PR body so the
maintainer knows the backfilled dates are git-authoritative, not
memory-authoritative.

---

**Computing `bodyHash` for MDX files**

The body is everything *after* the closing `---` of the frontmatter —
i.e. the MDX content itself, not the YAML header. Compute the hash
over the body ONLY (so frontmatter fields like `generatedAt` or
`bodyHash` itself don't affect it). Procedure:

1. Write the file with frontmatter + body (no `bodyHash` field yet).
2. Extract the body: everything after the second `---` line.
3. Compute `sha256(body)`.
4. Rewrite the file with `bodyHash: <hash>` added to the frontmatter.

Shell helper (POSIX awk):
```bash
awk 'BEGIN{n=0} /^---$/{n++; next} n>=2 {print}' <file> | sha256sum | awk '{print $1}'
```

The same procedure is used in `update` mode Step 2's hand-edit
check. Matching implementations on both sides is the whole point.

### Step 7.5 — Analyze the repo's design and write `theme.json`

Before committing, pick the site's visual identity once. This step runs
only during `init`; scheduled modes must not touch `theme.json` (see
invariant 7 in SKILL.md).

Follow `prompts/theme-system.md`. Summary:

1. **Find a brand asset.** Glob these paths for `logo.*`, `banner.*`,
   `brand.*`, `icon.*`:
   - repo root, `/assets/`, `/.assets/`, `/.github/`,
     `/docs/images/`, `/docs/assets/`, `/public/`, `/static/`
2. **If found, view the image** and extract its dominant brand color.
   Copy the asset to `docs/public/{filename}`.
3. **If no asset**, check `README.md` for shields.io badge colors.
4. **If no badge color**, collect repo metadata:
   ```bash
   gh repo view --json description,repositoryTopics,primaryLanguage
   ```
   and classify using the vibe table in `prompts/theme-system.md`.
5. **Pick exactly one vibe** from: `editorial`, `technical`, `clinical`,
   `expressive`. Default to `editorial` if signals are ambiguous.
6. **Produce `accent` and `accentDark`** per the color-extraction rules.
7. **Write `docs/content/theme.json`** conforming to
   `schemas/theme.schema.json`. Include a one-sentence `signals.reasoning`
   so the maintainer reviewing the PR can see your rationale.

Do NOT generate custom CSS or edit any template stylesheet. The template
ships one stylesheet per vibe; `theme.json` just picks which one loads.

### Step 8 — Commit and push

```bash
git add docs/ docs/package-lock.json docent.config.json .github/workflows/docent-deploy.yml
git commit -m "Docent: initial site scaffold"
git push -u origin HEAD
```

The lock file must be committed — the deploy workflow caches npm based on
it, and `npm ci` requires it.

### Step 9 — Open the PR

```bash
gh pr create \
  --title "Docent: initial site scaffold" \
  --body "$(cat <<'EOF'
This PR scaffolds a Docent-maintained site at `/docs`.

## What's included

- `/docs/` — Astro site template and initial content
- `docent.config.json` — project settings
- `.github/workflows/docent-deploy.yml` — GitHub Pages deploy

## Generated content

- Overview page based on this repo's README and structure
- Status page summarizing {{N}} open issues
- Changelog with {{N}} release entries
- **{{N}} backfill journal posts** covering the repo's history
  ({{bucket description}}):
  {{each filename on its own line, oldest first}}

> **About the backfill posts**: these are **retrospective
> reconstructions** from the commit log, not contemporaneous reporting.
> Please scan-check each post — Docent is doing archaeology, not
> remembering what you were actually thinking at the time. Dates are
> git-authoritative; any force-pushed or rebased history will produce
> dates that don't reflect when work really happened.

## Next steps

1. **Enable GitHub Pages with Actions as the source** (one-time). Either:
   - **Settings → Pages → Source → GitHub Actions** in the web UI, or
   - ```bash
     gh api -X POST repos/{{owner}}/{{repo}}/pages -f build_type=workflow
     ```
2. **Merge this PR.** The deploy workflow runs automatically.
3. **Set up the Routine** that keeps content fresh. In any Claude Code
   session, run one command. The routine spawns a remote environment
   that starts with *nothing installed* — the prompt must bootstrap
   the Docent plugin before dispatching to update mode:

   ```
   /schedule Docent update daily at 08:00 — If the Docent skill is not available in this environment, run `/plugin marketplace add calumjs/docent` and `/plugin install docent@docent` first. Then follow skills/docent/modes/update.md against the current repo.
   ```

   One routine, adaptive output: update mode decides for itself whether
   to regenerate content, write a journal post, both, or nothing,
   based on what happened since the last run (`modes/update.md` Step 6).
   The digest mode is still available for manual invocation ("Docent,
   write a post about the auth refactor") but has no scheduled
   presence.

   The bootstrap clause is there because Claude Code Routines run in a
   clean remote CCR environment that does NOT inherit plugins from the
   maintainer's local session — without it, the first scheduled run
   fails silently and the site quietly goes stale.

   Or configure the routines at claude.ai/code/routines directly,
   including the same bootstrap text in each prompt. Routines use your
   Claude Code subscription — no `ANTHROPIC_API_KEY` required.

   **Trust / pinning note.** `docent@docent` tracks the marketplace's
   default ref (the Docent repo's `master`). Every routine run pulls
   the latest version, which is the same trust posture as any
   auto-updating tool. Users who want stricter reproducibility can pin
   to a specific commit by replacing the install line with
   `/plugin install docent@docent@<commit-sha>` (e.g. a tagged
   release). Pinning trades update freshness for supply-chain
   tightness — if the Docent repo is ever compromised, pinned
   installs keep running the known-good version.

   **DST caveat.** Claude Code Routines take UTC cron only; they do
   not currently support IANA-timezone-aware scheduling. A routine
   scheduled for "08:00 Sydney" becomes a fixed UTC cron that drifts
   by an hour when Australia enters or leaves AEDT. For most content
   workflows this doesn't matter (who cares if the daily update runs
   at 008:00 vs 08:00?), but users who need wall-clock-stable timing
   should plan to re-adjust the cron twice yearly at DST transitions,
   or pick a local time far from midnight where an hour's drift is
   inconsequential.

The site will be live at {{homepage}} shortly after the first deploy.

---

_Did Docent get something wrong during scaffolding — a prompt that
missed the mark, a step that failed, a default that doesn't fit this
project? Open Claude Code, say **"Docent, suggest"**, and it'll help
you file feedback against [calumjs/docent](https://github.com/calumjs/docent/issues)._
EOF
)"
```

### Step 10 — Report back to the user

Report the PR URL and echo the three-step checklist from the PR body.
Make the routine-creation command the most prominent part of the
reply — it's the step users most often forget. Call out that Routines
run on their subscription plan, not via a separate API key.

**Offer to create the routine directly.** At the end of the report,
ask whether the user wants you to set up the update routine now. If
they agree, invoke the `schedule` skill (via the Skill tool) with the
routine definition below. This saves the user from pasting the long
bootstrap prompt manually.

The routine definition to pass:

- Name: `Docent update`. Cron: daily at 08:00 local (convert to UTC
  for the cron expression based on the user's timezone). Prompt:
  ```
  If the Docent skill is not available in this environment, run:
    /plugin marketplace add calumjs/docent
    /plugin install docent@docent
  Then follow skills/docent/modes/update.md against this repo. Update
  mode will decide for itself whether the day's activity is worth a
  journal post; if it is, write one in the same PR. Open a PR against
  the default branch if anything changed, otherwise exit silently.
  ```

The routine targets the user's GitHub repo (owner/repo already
collected in Step 1). If the user declines the offer or the schedule
skill isn't available, fall back to printing the paste-this command
from Step 9's PR body.

**No separate digest routine.** Earlier versions of this skill shipped
two Routines (daily update + weekly digest). That's consolidated now:
update mode decides journal-worthiness adaptively. Manual digest
invocation ("Docent, write a post about X") still works via
`modes/digest.md` for topic-scoped posts.

End the reply with a short invitation to file feedback:

> Noticed something that felt off during scaffolding — a prompt that
> missed the mark, a step that failed, a default that doesn't fit your
> project? Say **"Docent, suggest"** anytime and I'll help you file it
> against calumjs/docent. Every report sharpens the next run.

This nudge is important because init is the moment when the user has
freshest context on what Docent just did and didn't do well. Waiting
until they notice again is a longer feedback loop than just asking
now.

## Exit conditions

- PR has been opened successfully. Report URL.
- PR creation failed. Report the error and the current branch state so the
  user can push manually.

## Error handling

- **No `gh` auth**: prompt the user to run `gh auth login`.
- **No write access to repo**: explain and offer to generate the files
  locally for the user to push manually.
- **Existing `/docs` directory with unrelated content**: stop. Ask the user
  whether to use a different directory or abort.
- **No git remote**: stop. Explain that Docent needs a GitHub remote.
