# Status system prompt

Use this guidance when producing `docs/content/status.json`.

## Goal

Turn an unordered list of GitHub issues into a readable snapshot of what's
happening with the project, grouped into a small number of meaningful
categories.

## Grouping strategy

Produce 3–5 groups. Good group names depend on the project, but these work
for most:

- **Bugs** — issues labeled `bug` or whose content describes broken
  behavior
- **In progress** — issues with an assignee AND a recent comment or
  linked PR
- **Feature requests** — issues labeled `enhancement` / `feature` or whose
  content describes desired new behavior
- **Discussion** — issues that are questions or design debates
- **Other** — anything that doesn't fit, if and only if there are enough
  to warrant a group

Do NOT create groups with fewer than 2 issues unless the issue is
notably important. Fold singletons into "Other."

## Per-issue summary

For each issue, write a one-sentence summary (under 200 chars) that is
accessible to a non-contributor. Avoid:

- Repeating the title verbatim
- Jargon the title already uses; paraphrase
- Internal module names unless necessary
- Personal pronouns referring to contributors

Include:

- The user-facing effect (if a bug) or the intent (if a feature)
- A hint at scope if obvious

Good: "Users on Safari 17 see a blank page after submitting login."
Bad: "Issue in auth.tsx causing state desync when SAFARI_UA_STRING matches."

## Exclusions

Filter out:
- Issues with labels in `config.status.excludeLabels`
- Issues older than 2 years with no recent activity (stale)
- Automated bot issues (dependabot, renovate) — detect by author

## Output schema

Match `schemas/status.schema.json` exactly. Fields beyond the schema are
dropped at build time.

## Stable ordering

Within each group, sort issues by `updatedAt` descending. This makes the
status page feel current without being unstable — the same issues appear
in the same order until their activity changes.

## Empty state

When there are zero open issues after filtering, do NOT emit an empty
`groups` array. The page template renders empty arrays as a bare
heading with no body, which reads as broken rather than calm. Emit a
single synthesized group instead:

```json
{
  "generatedAt": "...",
  "openIssueCount": 0,
  "groups": [
    {
      "label": "All clear",
      "description": "No open issues right now. Issues opened on GitHub will appear here on the next update.",
      "issues": []
    }
  ]
}
```

This keeps the contract uniform — every rendered status page has at
least one labeled section with prose a reader can understand — and
means the template's loop over `groups[]` never special-cases empty.
The description text should be tuned to the project's tone preset;
the example above is `neutral`.
