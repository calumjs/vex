# Theme system prompt

Use this guidance when producing `docs/content/theme.json` during `init`.

The goal: a Docent site should look like it belongs to the project, not
like a generic template. You have four vibe stylesheets to pick from and
two accent colors to produce. Get both right once; never touch this file
again on scheduled runs.

## Signals you have

Collect in this order. First signal found wins for its purpose; later
signals fill gaps.

1. **Brand asset.** Glob the following paths for `logo.{svg,png,webp}`,
   `banner.*`, `brand.*`, `icon.*`:
   - repo root
   - `/assets/`, `/.assets/`
   - `/.github/`
   - `/docs/images/`, `/docs/assets/`
   - `/public/`, `/static/`
   If found, use the image tool to view it. Extract the dominant accent
   color by eye — not the background, not a neutral, the color a human
   would describe as "the brand color." Copy the asset to
   `docs/public/{filename}` and record its path as `heroImage`.
2. **README badges.** Parse `README.md` for shields.io URLs. Look for
   `color=` query params or inline hex. These are secondary accents at
   best but fine as a fallback.
3. **Repo metadata.**
   ```bash
   gh repo view --json description,repositoryTopics,primaryLanguage
   ```
   Topics and the primary language are strong vibe signals. Keep the raw
   metadata in view while classifying — don't paraphrase it away.
4. **README prose.** Read the first ~500 words. Is it dense and
   technical? Marketing-lite? Academic? Prose-heavy in a way that
   suggests the project itself is about words?

## Picking the vibe

Pick exactly one. When two fit, pick the one that reflects the
**primary audience** rather than the secondary. A research ML library
with a playful README is still `clinical` — researchers are the audience.

| vibe | pick when |
|---|---|
| `editorial` | Prose-heavy libraries, documentation tools, writing utilities, static site generators, the "Docent is a museum guide" default. Also: when nothing else obviously fits. |
| `technical` | CLIs, systems tools, infrastructure, DX tooling, databases, parsers, compilers. Topics like `cli`, `devtools`, `systems-programming`, `infra`. |
| `clinical` | Research projects, ML libraries, data-analysis tools, academic software, anything with `research`, `ml`, `dataset`, `benchmark` in topics. |
| `expressive` | Games, game engines, creative-coding tools, design libraries, generative-art projects. Topics like `game`, `creative-coding`, `graphics`, `animation`. |

**Default when ambiguous:** `editorial`. It's the safest of the four; a
prose-friendly site rarely feels wrong.

## Extracting accent colors

You produce **two** hex colors: `accent` (light mode) and `accentDark`
(dark mode).

### If you extracted a color from a logo

- `accent` = the logo color, as-is, unless it fails basic contrast against
  a near-white background. If contrast is poor, darken ~15% until it's
  readable against `#fafaf9`.
- `accentDark` = brightened complement for dark mode. Lighten the light
  color ~20% and nudge saturation up slightly. It should read well against
  `#0c0a09`.

### If you're using a README badge color

Badges are usually chosen for legibility on GitHub, not for brand
fidelity. Trust them less. Use as `accent` if no logo was available, and
produce `accentDark` as above.

### If you have no signal (metadata or default only)

Use the vibe's default palette:

| vibe | default `accent` | default `accentDark` |
|---|---|---|
| `editorial` | `#b45309` (warm amber) | `#f59e0b` |
| `technical` | `#0284c7` (slate blue) | `#38bdf8` |
| `clinical` | `#475569` (neutral slate) | `#94a3b8` |
| `expressive` | `#db2777` (saturated pink) | `#f472b6` |

These are deliberately distinct — a default-theme site shouldn't look
like every other default-theme site. They're also deliberately not
trendy; avoid setting fashion trends the template will have to follow.

## The `signals` field

Record where the decision came from. `source` is one of:

- `logo-analysis` — accent from a logo you inspected
- `readme-badges` — accent from a badge color
- `metadata-heuristic` — vibe inferred from topics/language; accent from
  the vibe default
- `default` — nothing useful found; vibe is `editorial`, accent is the
  editorial default

`reasoning` is one short sentence. Write it for a human reviewer of the
init PR: explain *why* you picked this vibe and this accent, briefly, so
they can see your reasoning and override if they want. Examples:

> "Warm serif logo and a README dominated by prose — `editorial` fits."

> "Topics include `cli` and `devtools`; primary language is Rust.
> Went `technical` with the default slate-blue accent."

> "No logo or brand asset found; README is sparse. Defaulted to
> `editorial` with the warm-amber accent."

Keep it under 240 characters. Don't hedge ("I think," "possibly"). State
the choice and the reason.

## Output shape

Write `docs/content/theme.json` conforming to `theme.schema.json`.
Nothing else in this file — no extra fields, no comments. The template
loads it as JSON.

## What NOT to do

- Don't generate custom CSS. The template owns all styling; you only
  pick a vibe + tokens.
- Don't invent colors outside what the signals support. If you have no
  logo, don't guess "what color would this project use" — use the vibe
  default.
- Don't modify `theme.json` on scheduled runs. This file is init-only.
  If an `update` or `digest` run would touch it, stop and skip.
- Don't paste the entire README into `reasoning`. One sentence.
