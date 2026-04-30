# Mode: `suggest`

File a feedback issue against `calumjs/docent` from inside the user's
Claude Code session, with their config and plugin version attached. The
goal: make it easier to say "I noticed something" than to not say it.

## Preconditions

- `gh` CLI is authenticated (`gh auth status` succeeds). If not, prompt
  the user to run `gh auth login` before continuing.
- The user explicitly invoked suggest — this mode never runs on
  schedule and never auto-opens issues.

## What feedback goes here

Feedback about **Docent itself** — its prompts, modes, schemas,
defaults, or procedure. Things like "the inaugural post came out
weird," "init Step 5 failed on my harness," "the editorial vibe
looked wrong on my repo."

Feedback about the **user's own project** (its bugs, its features) does
NOT go here. That's what the site's `/report-bug/` and
`/request-feature/` pages are for — they file against the user's own
repo, not against Docent.

If the user's request sounds like project feedback rather than Docent
feedback, redirect them before continuing:

> That sounds like feedback for {their-project}, not for Docent. Open
> the /report-bug/ or /request-feature/ page on your Docent site — it
> opens a pre-filled issue against your repo. Want me to do that
> instead?

## Procedure

### Step 1 — Short structured intake

Ask these in ONE message. Keep it to four questions; longer forms kill
feedback loops harder than no form.

1. **Kind** — enhancement, bug, prompt issue, procedure issue, or
   other? (Pick one.)
2. **Where did you notice it?** — which mode were you running (init,
   update, digest, release, triage, suggest), which file or step was
   involved.
3. **What happened?** What did you see, what did you expect instead,
   and what makes it matter to the maintainer?
4. **One-line title** (optional). If skipped, draft one yourself from
   the answer to #3.

Accept short answers. Users who want to write essays can; users who
want to fire off a one-liner per question should be able to.

### Step 2 — Gather environment

The issue will be filed on a **public** repository (calumjs/docent).
Collect only a minimal, non-identifying whitelist of fields — never
the raw `docent.config.json` or raw repo metadata. A feedback issue
doesn't need to know which user is filing or which private project
they're filing from; the maintainer can reproduce from the mode +
plugin version + a handful of behaviour-shaping flags.

**Do this**:

```bash
# Plugin version only — from plugin.json, NOT the whole file
node -e 'console.log(require("${CLAUDE_PLUGIN_ROOT}/.claude-plugin/plugin.json").version)' 2>/dev/null

# Whether the source repo is public or private (affects publishing
# context; matters for the privacy warning in Step 4.5)
gh repo view --json visibility -q .visibility 2>/dev/null

# From docent.config.json: read it yourself, extract only:
#   tone, journal.cadence, journal.backfill, journal.backfillLimit
# Nothing else. Do NOT include project.{name,owner,repo,homepage},
# deploy.customDomain, or any other field.
```

**Do NOT do**:

- Do NOT `cat docent.config.json` into the issue body. The project
  name, owner slug, custom domain, and (if edited) excludeLabels are
  identifying or potentially sensitive for private projects.
- Do NOT run `gh repo view` with `name,owner,primaryLanguage,
  repositoryTopics`. The owner and repo name identify the user's
  project to everyone who reads calumjs/docent's issue tracker. The
  `gh issue create` author field already tells the maintainer who
  filed it; there's no need to embed owner/repo in the body.
- Do NOT include git config email, commit messages, or any string
  scraped from the user's filesystem beyond what's explicitly listed
  above.

If `docent.config.json` doesn't exist (user is filing feedback on the
`init` mode itself before it's run), record behaviour flags as
"config not yet written."

### Step 3 — Compose the issue body

Template. Note: the environment block is a small, specific whitelist —
no raw config dump, no identifying repo metadata.

```markdown
## What kind of feedback

{kind}

## Where

{where — mode, file, step}

## What happened

{what they told you, their words lightly edited for clarity but NOT
rewritten. If they wrote a one-liner, leave it short.}

---

<details>
<summary>Environment (whitelisted fields, click to expand)</summary>

- **Docent plugin version:** {e.g. 0.1.0}
- **Tone:** {neutral / formal / playful / technical}
- **Journal cadence:** {weekly / biweekly / monthly / manual}
- **Backfill:** {on with limit N / off}
- **Source repo visibility:** {public / private}

_(No project name, owner, repo, domain, or config contents are
included here — those identify the user's project and aren't needed
for Docent triage.)_

</details>

---

_Filed via Docent `suggest` mode._
```

The footer intentionally does NOT say "from {owner}/{name}" — that
would re-leak identity the whitelist was built to avoid. The `gh
issue create` author field carries the filer's identity to the
maintainer without exposing which of their projects was involved.

### Step 4 — Redact the prose

The whitelist in Step 2 keeps structured config data out of the body
by construction. But the user's **prose answers** (to "what happened",
"where did you notice it") are free-form and can still contain
secrets — a pasted error message with a token in it, a copied URL with
credentials. Run a redaction pass over the prose fields only. Replace
matches with `[REDACTED]`:

- GitHub tokens: `gh[oprs]_[A-Za-z0-9]{20,}`
- AWS access keys: `AKIA[0-9A-Z]{16}`
- GCP service-account keys: `-----BEGIN PRIVATE KEY-----` through the
  matching `-----END PRIVATE KEY-----`
- JWTs: `eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+`
- URLs containing `:` + `@` (basic-auth in URL): e.g. `https://user:pass@host`
- Generic high-entropy 32+ char hex or base64 runs that appear next to
  keywords like `token`, `key`, `secret`, `password`, `api_key`, `bearer`
- Email addresses (unless they're clearly public — GitHub noreply,
  e.g. `@users.noreply.github.com` stays)

The regex list is defense-in-depth, not a promise of completeness.
The primary protection is the whitelist from Step 2: because the
body never contains raw config or identity fields, there's a much
smaller surface for a missed pattern to do harm.

SKILL.md invariant 3 forbids writing secrets into generated *site
content*; this extends the same rule to generated *issue content*.

### Step 4.5 — Private-repo warning

If the source repo's visibility (collected in Step 2) is `private`,
show an extra explicit warning before the confirmation prompt:

> **Heads up**: you're filing feedback from a **private** repository
> into a **public** issue tracker (calumjs/docent). The environment
> block only includes Docent behaviour flags (tone, cadence, plugin
> version, visibility) — no project name, owner, or config content.
> But your prose description will be public. Review it for any
> internal details before confirming.

The Step 5 confirmation still waits for explicit user approval.

### Step 5 — Confirm with the user

Show the full composed body and the target:

> I'm about to file this against `calumjs/docent`:
>
> **Title:** {title}
>
> {body}
>
> Confirm to file, or say what to change.

Wait for explicit confirmation. Never call `gh issue create` without
an affirmative response from the user in this turn — no silent
external writes.

If the user requests changes, apply them and re-confirm before filing.

### Step 6 — File

```bash
gh issue create \
  --repo calumjs/docent \
  --title "{title}" \
  --body "$(cat <<'EOF'
{redacted, confirmed body}
EOF
)" \
  --label "{kind-derived-label}"
```

Labels by kind:
- `enhancement` or other → `enhancement`
- `bug` → `bug`
- `prompt issue`, `procedure issue` → `documentation`
- `other` → no label (maintainer triages)

### Step 7 — Report back

Report the issue URL to the user. Thank them briefly and mention that
the maintainer reviews feedback when iterating on Docent's prompts.

## Exit conditions

- Issue filed, URL reported, or
- User declined to confirm — exit without filing. Do not retry
  automatically.

## Error handling

- **`gh auth status` fails**: prompt the user to run `gh auth login`.
- **`gh issue create` fails** (rate limit, network, permissions): show
  the full composed body and the title so the user can file manually
  at https://github.com/calumjs/docent/issues/new.

## Non-goals

- Not a general-purpose bug tracker. Feedback goes to `calumjs/docent`,
  not to the user's own repo.
- Not anonymous. Issues are filed under the authenticated `gh` user,
  same as any other `gh issue create`.
- Not a proxy for iterating on the user's project — use the site's
  `/report-bug/` or `/request-feature/` forms for that.
