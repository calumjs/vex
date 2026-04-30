# Tone presets

Docent supports four tones, configurable via `tone` in `docent.config.json`.
Apply the matching guidance when writing any content.

Each tone has two example openings, because **overview and journal
voices differ**. The overview introduces *what the project is*; the
journal reports *what happened this week*. Same tone, different
structure. When picking a voice for a given content type, match the
example for that type.

## `neutral` (default)

Plain, descriptive, past tense for retrospective content. Minimal
adjectives. No exclamation marks. Reads like competent documentation.

- **Overview opening:** "Docent turns a GitHub repository into a
  public-facing static website. It reads the repo's commits, issues,
  and releases, and writes the site's content."
- **Journal opening:** "The auth module was rewritten to support
  passkeys. Three Safari bugs were closed alongside it."

## `formal`

Third-person, publication-style. Slightly more elevated vocabulary.
Longer sentences are OK if they're well-constructed. Suitable for
projects that want to read like a company release or an academic tool.

- **Overview opening:** "Docent is a documentation system that produces
  a public-facing static website from a project's own source history,
  issue tracker, and release metadata."
- **Journal opening:** "The authentication module has been substantially
  restructured to introduce native passkey support."

## `playful`

First-person plural ("we shipped"), light wordplay allowed, willing to
admit mistakes openly ("we broke login for a full day on Tuesday and
everyone was very annoyed"). Avoid forced jokes. Humor should come from
specifics, not from the voice straining to be funny.

- **Overview opening:** "Docent is the little robot that reads your
  repo and writes the website your repo wishes it had. We built it
  because README.md is doing too much."
- **Journal opening:** "We finally got passkeys working, only 11 months
  after saying we would."

## `technical`

Assume the reader is a developer. Include file paths, PR numbers inline
(not just as links), module names. Short, dense sentences. Still
accessible — the overview isn't a README and the journal isn't a
release-notes dump, but both assume technical fluency.

- **Overview opening:** "Docent is a Claude Code skill (`skills/docent/`)
  that writes MDX and JSON into `/docs/content/` and ships an Astro
  template that renders it. Deploys via GitHub Pages."
- **Journal opening:** "`src/auth/` was rewritten around the WebAuthn
  API. Session cookies are gone; tokens now live in IndexedDB."

## Common rules (all tones)

- No marketing filler in any tone.
- No "simply," "just," or "easily" describing features.
- No superlatives without support. "Fastest" requires a benchmark link.
- No fictional testimonials or user quotes.

## How to apply

When the overview prompt asks for the opening paragraph, consult the
**Overview opening** for the active tone. When the journal prompt asks
for the opening, consult the **Journal opening**. Don't cross the
streams — a journal that opens like an overview feels stale, and an
overview that opens like a journal confuses first-time visitors.
