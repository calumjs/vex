# Overview system prompt

Use this guidance when writing `docs/content/overview.mdx`.

## Audience

A curious visitor who found the project via search, a link, or word of
mouth. They are NOT already a contributor. They may or may not be technical.
They want to know, in this order:

1. What does this project do?
2. Who is it for?
3. Why might I care?
4. How do I get started or learn more?

## Structure

Aim for 300–800 words total. Roughly:

- **Opening paragraph** (50–100 words): the one-sentence pitch, expanded.
  Lead with the concrete thing the project does, not with abstractions like
  "a powerful toolkit for..."
- **What it's for** (100–200 words): the problem it solves, with at least
  one specific example of a situation where you'd use it.
- **How it works at a high level** (100–200 words): the shape of the thing
  — is it a CLI, a library, a web app, a service? What does using it look
  like? Include one small code or usage example if relevant.
- **Getting started** (50–150 words): the shortest path from "I'm curious"
  to "I've tried it." Link to the README or install docs rather than
  duplicating them in full.
- **Closing** (optional, 20–50 words): a pointer to the journal, the status
  page, or the repo itself.

## Rules

- **Concrete over abstract.** "It converts Markdown to PDF" beats "a
  flexible document transformation platform."
- **Short sentences.** Average 15–20 words. Split anything longer unless
  the rhythm demands it.
- **No marketing filler.** Avoid "seamlessly," "robust," "powerful,"
  "cutting-edge," "elegant," "beautiful," "simply," "just." If a sentence
  survives removing these words, use that version.
- **No installation commands unless essential.** The README has those. Link
  instead: `[See the README for install instructions](...)`.
- **No claims you can't verify from the repo.** If the README doesn't say
  it's fast, don't say it's fast.
- **Imports OK.** The MDX file may use Astro components. Don't invent
  components — only use ones that exist in `docs/src/components/`.
- **Internal links must respect the site's base path.** The site
  deploys to `{owner}.github.io/{repo}` by default, so internal routes
  are served from `/repo/…`, not `/`. Markdown links like
  `[report a bug](/report-bug/)` render verbatim — they miss the base
  and break on GitHub Pages. Use inline HTML with the base
  interpolated instead:

  ```mdx
  <a href={`${import.meta.env.BASE_URL}report-bug/`}>report a bug</a>
  ```

  External links (GitHub issues, PRs, anything `https://…`) are
  absolute and fine as normal markdown. This only matters for links
  *to other pages on this same site*.

## Tone adjustments

Apply the tone preset from config (see `tone-presets.md`).

## Frontmatter

Required fields:

```yaml
---
title: "Project Name"
tagline: "one-line description"
generatedBy: "docent"
generatedAt: "{ISO timestamp}"
mode: "{mode}"
---
```

`tagline` should be under 70 characters and read well as a subtitle under
the project name.
