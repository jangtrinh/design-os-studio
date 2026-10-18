# E15-43 Turn CSS custom properties into tokens when importing a web capture

- Issue: #633 (task E15-43, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: the web-capture module created by #276/#277 (`designos-interop/webcapture` in those issues): capture script additions (custom properties and declaration sources) and a new `token_mapping` step in the importer; `crates/designos-export/src/tokens/` (reuse of E15-42 value parsing and naming).

## 1. Summary and user story

As a designer importing a live page, I want the page's CSS custom properties to arrive as variables, and every layer whose colour, radius, spacing or font size came from a property to be bound to that variable, so the import keeps its token structure instead of hard-coded values.

## 2. Current state

- FACT: no web capture exists on `main`. T8.25 (#276) writes the capture script (computed styles, boxes, text runs, data-URL images) with Playwright; T8.27 (#277) imports captures as auto-layout frames with editable text and a screenshot diff within threshold. Both moved into this pack.
- FACT: E15-42 parses custom property values, names variables (`--primary` → `Color/primary` with WEB code syntax `var(--primary)`), and builds modes.
- FACT: E15-11 (#596) adds the token-match lint: hard-coded values that equal a variable value.
- FACT: computed styles resolve `var()` references, so the authored source of a value is not visible in `getComputedStyle` output alone.

## 3. Behaviour rules

1. **Capture additions** (in the capture script from #276):
   - `custom_properties`: for `document.documentElement` and for each element that declares custom properties, the computed value of every property starting with `--`, plus, when the page uses `.dark`, `[data-theme]` or `prefers-color-scheme`, a second capture pass with that theme active (the capture runs once per detected theme and records the theme name);
   - `declaration_sources`: for each captured element and each tracked property (`color`, `background-color`, `border-*-color`, `border-*-radius`, `padding-*`, `gap`, `row-gap`, `column-gap`, `font-size`, `font-family`, `font-weight`, `line-height`, `box-shadow`), the authored value of the winning declaration when it contains `var(`. The winner is found by walking `document.styleSheets` rules whose selector matches the element (`element.matches`), plus the inline style, ordered by importance, specificity and source order (CSS cascade level 4 rules, without `@layer` reordering beyond layer order); cross-origin sheets that cannot be read are listed in `capture.warnings`.
2. **Variables.** The importer builds variables from `custom_properties` with E15-42's naming and value rules: one collection named after the page host (`example.com`), modes from the captured themes (`Light`/`Dark`/…, or `Default`). Only properties referenced by at least one declaration source or by another imported property are imported (pages often define hundreds of unused ones); the rest are counted in the report as `unused_properties`.
3. **Binding by source.** For each node whose element has a declaration source `var(--x)` for a property, the matching node property binds to the variable for `--x` (fill → `fills/0`, stroke colour → `strokes/0`, radius → per-corner keys, padding and gap → auto-layout keys, text properties → text binding keys). Nested `var(--a, var(--b))` binds to `--a` when it is defined, else `--b`.
4. **Binding by exact value.** Remaining hard-coded values on imported nodes that exactly equal a variable's value in the node's mode (colours compared after conversion to the document profile with tolerance 0.5/255 per channel; numbers with |Δ| ≤ 0.0005) are bound when exactly one variable matches. Two or more matches are not bound.
5. **Report through lint (AC2).** After binding, the importer runs E15-11's token-match rule on the imported subtree. Values left unbound (ambiguous or no match) appear as lint findings with node names, as for any document.
6. **Scope of auto-fix.** E15-13 auto-fix follows the user's settings; the import itself never applies auto-fix beyond rules 3–4.
7. **One undo step** for the whole import, as T8.27 defines.

## 4. Data model / schema

Capture JSON additions (version bump of the #276 capture format):

```json
{ "capture_version": 2,
  "themes": ["Light", "Dark"],
  "custom_properties": { "Light": { ":root": { "--primary": "oklch(0.21 0.006 285.9)", "--radius": "0.625rem" } },
                         "Dark":  { ":root": { "--primary": "oklch(0.92 0.004 286.3)" } } },
  "elements": [ { "id": 41, "declaration_sources": { "background-color": "var(--primary)", "border-top-left-radius": "calc(var(--radius) - 2px)" } } ],
  "warnings": [ { "name": "stylesheet_unreadable", "href": "https://cdn.example/x.css" } ] }
```

`calc()` sources are not bound (the value is derived); they fall through to rule 4 and then to the lint report.

Importer report additions: `{ "variables_created": n, "bound_by_source": n, "bound_by_value": n, "unbound": n, "unused_properties": n }`.

## 5. Contracts

The web-capture import tool and CLI from #277 gain the report fields above. No new tool.

## 6. File-format mapping

| Capture | Document |
|---|---|
| `--primary` in `:root` (Light) and `.dark` (Dark) | variable `Color/primary` in collection `<host>`, modes Light and Dark |
| button `background-color: var(--primary)` | button frame `fills/0` bound to `Color/primary` |
| button `border-radius: calc(var(--radius) - 2px)` | literal radius; token-match lint if it equals a radius variable, else no finding |

## 7. UI

No new UI. The import report sheet from #277 shows the four counts; lint findings appear in the usual lint list.

## 8. Edge cases

1. Property defined on a component root rather than `:root` (scoped tokens): imported in the same collection with the scoping selector recorded in the variable description; binding uses the value the element saw.
2. Properties holding non-token data (`--tw-ring-offset-shadow` and other utility internals): imported only when referenced (rule 2); utility internal names (prefix `--tw-`) are excluded entirely and counted.
3. Theme toggle not detectable: single `Default` mode; no warning.
4. Cross-origin stylesheets: sources unknown for their rules; binding falls back to rule 4; warning listed.
5. Shadow DOM: declarations inside open shadow roots are walked; closed roots are skipped with a warning.
6. Same value from two different properties (`--primary` and `--brand` both `#111`): source binding is exact; value binding sees two matches and leaves the node unbound (lint reports it).

## 9. Non-goals

- Components from repeated subtrees (E15-44).
- Capturing pages behind authentication flows (T8.25's concern).

## 10. Test plan

Seam: capture fixture test (Playwright on local fixture pages, as #276's test seam) plus an importer unit test on a stored capture JSON, plus a screenshot.

Fixtures: `primary-button.html` (ours: `:root { --primary: #18181b; --primary-foreground: #fafafa; } .dark {…}` and three buttons using `background: var(--primary)`), `ambiguous-values.html`.

Fail-before: the importer has no token mapping.

| Test | Input | Expected |
|---|---|---|
| `primary_property_binds_buttons` (AC1) | primary-button.html | variable `Color/primary` exists; all three button frames have `fills/0` bound to it; modes Light and Dark |
| `unmatched_values_reported_as_lint` (AC2) | ambiguous-values.html with `#111111` from two properties and one literal `#123456` | token-match findings name the nodes; nothing bound for those |
| `calc_source_not_bound` | radius `calc(var(--radius) - 2px)` | literal radius; no source binding |
| `unused_properties_counted` | page with 200 unused `--x-*` | not imported; `unused_properties = 200` |
| `tw_internals_excluded` | `--tw-ring-color` referenced | excluded, counted |
| `one_undo_step` | import then undo | document restored |

Visual evidence: capture of the imported page in Studio with the Variables panel and a selected button showing its bound fill.

## 11. Performance budgets

- Declaration source resolution in the capture script ≤ 2 s for a page with 5,000 elements and 2,000 rules.
- Importer mapping ≤ 200 ms for 5,000 nodes. Import applies as one batch (CLAUDE.md §4.1).

## 12. Dependencies and build order

- Needs #276 (T8.25), #277 (T8.27), E15-42, E15-11 (#596).
- Parallel with E15-44 (same importer; merge E15-43 first, E15-44 rebases).

## 13. Owner rulings that apply

- R6.22: Studio's own web-capture importer stays in scope; this is part of it (not a Figma plugin workflow).
- Nothing vanishes silently: unmatched values are reported through lint.
