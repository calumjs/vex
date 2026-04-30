# Mode: `release`

Generate changelog entry (and optionally an announcement journal post) for a
specific release tag.

## Preconditions

- `docent.config.json` exists.
- The tag specified by the user (or inferred from context) exists in the repo.

## Procedure

### Step 1 — Identify the tag

If user specified a tag, use it. Otherwise, use the most recent tag:

```bash
git tag --sort=-creatordate | head -1
```

### Step 2 — Find the previous tag

```bash
git describe --tags --abbrev=0 {tag}^   # previous tag reachable from {tag}^
```

If no previous tag, the range is from the repo's first commit.

### Step 3 — Collect changes in range

```bash
# Commits
git log {prev}..{tag} --no-merges --format='%h|%s|%an'

# Merged PRs in range (by merge commit)
gh pr list --state merged --search "merged:{prev-date}..{tag-date}" \
  --limit 100 --json number,title,labels,body

# GitHub release body if it exists
gh release view {tag} --json body 2>/dev/null
```

### Step 4 — Group by change type

Use conventional-commit prefixes where present (`feat:`, `fix:`, `chore:`,
`docs:`, etc.). For commits without prefixes, classify by content:

- **Added**: new features, new commands, new config options
- **Changed**: behavior changes, API changes, defaults
- **Deprecated**: features marked for removal
- **Removed**: features removed
- **Fixed**: bug fixes
- **Security**: security-relevant changes

Skip `chore:` and purely internal refactors unless they're user-visible.

### Step 5 — Write the changelog entry

Prepend to `docs/content/changelog.mdx`. Format:

```mdx
## {tag} — {YYYY-MM-DD}

_One-paragraph human summary of the release._

### Added
- Description (#PR or commit hash)

### Changed
- Description (#PR or commit hash)

### Fixed
- Description (#PR or commit hash)
```

If there's a GitHub release body, weave its key points into the summary
paragraph but do not copy it verbatim.

### Step 6 — Optional announcement post

If `docent.config.json` has `journal.announceReleases: true`, also create a
journal post `docs/content/journal/{YYYY-MM-DD}-release-{tag}.mdx` using
`digest` mode's post structure, but scoped to this release.

### Step 7 — Open PR

Branch: `docent/release-{tag}`.
PR title: `Docent: release notes for {tag}`.
PR body: include the changelog entry verbatim so reviewers see it without
navigating.

## Exit conditions

- PR opened with changelog entry and (optionally) announcement post.
