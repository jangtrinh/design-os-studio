# E15-11 Lint spacing scale, type scale and hard-coded values that match tokens

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | M / P0 |
| Depends on | E15-9, #154 (E3-10, number variables in the matcher), #155 (E3-11, library suggestions) |
| Unblocks | E15-13 (safe exact binds; snaps are suggestions), E15-3 acceptance 4 |
| Touches | new `crates/designos-doc/src/lint/rules/scale.rs`, new `crates/designos-doc/src/lint/token_match.rs` (the Check designs matcher moved here as a shared library function), `crates/designos-app/src/check_designs/hardcoded.rs` (becomes a caller) |

## 1. Summary and user story

As a designer or agent, I see every gap, padding, radius and font size that is off the file's own
scale, with the nearest step, and every raw value that exactly equals a variable or style, with a
one-step bind. The linter and Check designs agree because they call the same matcher.

## 2. Current state (FACT)

- Legacy linter scales are hard-coded constants: spacing `[0,2,4,6,8,12,16,20,24,32,40,48,64,80,96]`,
  font `[10..96]` (22 values), radius `[0..32, 999, 1000, 9999]` (lines 671-674 of the legacy module in
  `crates/designos-doc/src/`); "on scale" means within 0.05 (line 676-678); the nearest step is the
  minimum absolute distance (line 680-682). It ignores the document's own variables and styles.
- Check designs matcher (`crates/designos-app/src/check_designs/hardcoded.rs`): an index of colour
  variables by 8-bit RGB with a per-variable "constant across modes" flag
  (`hardcoded.rs:18-60`), alpha tolerance half an 8-bit step (`ALPHA_EPS = 0.002`, `hardcoded.rs:16`),
  outcomes `None | One | Ambiguous` (`hardcoded.rs:25-31`); whole-list style matching; variable wins
  over style for paints (`hardcoded.rs:1-5`). It lives in designos-app, so MCP cannot call it.
- #154 adds number variables (gap, padding, radius, stroke, font size, line height) to that matcher;
  #155 adds enabled-library suggestions. The #154 amendment (2026-10-18) requires exposing the
  matcher as a shared library function for E15.
- Bindable keys and scope checks: `property_kind`, `scope_allows`
  (`crates/designos-doc/src/variables_apply.rs:21-60`).

## 3. Behaviour rules

### 3.1 Scale sources (first that yields ≥ 3 distinct positive values wins)

| Scale | 1. Active style archetype | 2. Document | 3. Default |
|---|---|---|---|
| Spacing (gap, counter gap, padding) | archetype spacing parameter (part 2) | number variables whose scopes allow `GAP` (or all scopes), every mode's value | legacy spacing list (4 px base) |
| Radius | archetype roundness scale | number variables allowing `CORNER_RADIUS` | legacy radius list; values ≥ 999 are "pill" and always on scale |
| Type | archetype type scale | font sizes of local text styles plus number variables allowing `FONT_SIZE` | legacy font list |

The chosen source is reported in the finding (`scale_source: "archetype" | "variables" | "styles" |
"default"`). Zero is always on scale.

### 3.2 Off-scale rule (`off-scale`)

- Checked properties: `itemSpacing`, `counterAxisSpacing` (wrap only), the four paddings,
  corner radii (each corner), font size of each text style run.
- A property already bound to a variable is never off-scale (the binding is the source of truth).
- Off when `|v − nearest| > 0.05`. `nearest` = the step with minimum distance; when two steps are
  equidistant, both are listed and there is no single suggestion.
- Finding: `property`, `value`, `nearest` (number or two numbers), `scale_source`, and
  `suggested_fix: {kind:"set", property, from, to}` when unique. When the nearest step is itself a
  variable value with a unique variable, the fix is a bind (`to: "var:Spacing/4"`) instead of a raw
  set.
- Off-scale findings are never `safe` (`safe: false`): snapping to the nearest step changes a gap,
  padding, radius or font size, so E15-13 keeps `off-scale` in the suggest tier (owner rule: automatic
  fixes leave the rendered result unchanged). The unique `suggested_fix` above is offered as a Fix
  action only.
- Legacy ids `inconsistent-spacing`, `non-scale-font-size`, `inconsistent-radius` become aliases.

Example: scale `[4, 8, 12, 16, 24]`, gap 15 → off, nearest 16, suggested fix to 16; gap 14 → ties 12
and 16 → no fix; gap 30 → nearest 24, suggested fix to 24. None is applied automatically.

### 3.3 Hard-coded value rule (`hardcoded-value`)

- Uses `token_match::Index` (moved verbatim from `hardcoded.rs`, extended by #154/#155): colour
  variables, number variables, fill/text/effect styles, and enabled-library variables and styles.
- Exact match only: colours equal in 8-bit RGB with alpha within 0.002; numbers equal after rounding
  to 0.01; styles equal on the whole value list. Candidates are filtered by `scope_allows` for the
  property and node kind.
- `One` → finding with `suggested_fix: {kind:"bind", property, to:"var:<collection>/<name>"}` (or
  `"style:<name>"`), library origin named when it comes from a library (the fix imports and binds in
  one step, as #155).
- A `One` finding is `safe: true` (E15-13 safe tier) only when binding leaves the rendered result
  unchanged in every mode: the variable is constant across modes, or the style matches on the whole
  value list. Otherwise `safe: false`.
- `Ambiguous` → finding listing the candidates, no fix (R2.7).
- Legacy `hardcoded-color` (any unbound solid) remains only in the `strict` preset.

### 3.4 One matcher

`token_match` is the only matcher. `check_designs/hardcoded.rs` keeps only UI glue and calls it; the
designos-app copy of the index is deleted. Check designs and `designos_lint` therefore produce
identical `hardcoded-value` findings for the same scope.

## 4. Data model / schema changes

- `designos_doc::lint::token_match::{Index, Match, MatchKind}` (public).
- `designos_doc::lint::scale::{Scale, ScaleSource, scales_for(doc, archetype) -> Scales}`.
- Finding extension fields: `property`, `value`, `nearest`, `scale_source`, `candidates`, `safe`.
- No persisted model change.

## 5. Contracts

Findings appear in `designos_lint` (E15-9), the issues block (E15-12) and Check designs. Examples:

```json
{"rule":"off-scale","severity":"warning","node_id":77,"property":"itemSpacing","value":15,
 "nearest":16,"scale_source":"variables","safe":false,
 "suggested_fix":{"kind":"bind","property":"itemSpacing","from":15,"to":"var:Spacing/4"},
 "message":"Gap 15 is off the spacing scale; nearest step 16 (Spacing/4)."}
{"rule":"hardcoded-value","severity":"warning","node_id":78,"property":"fills/0","value":"#0D99FF",
 "candidates":["Color/Primary","Brand/Blue"],"suggested_fix":null,
 "message":"#0D99FF matches 2 variables; choose one."}
```

## 6. File-format mapping

None.

## 7. UI

Check designs shows `hardcoded-value` rows exactly as its hard-coded-values group does today, now
including numbers and library suggestions (from #154/#155). Off-scale rows appear when the panel's
preset includes them (preset selection is #158/E15-14).

## 8. Edge cases

1. Spacing variables that differ per mode (compact/comfortable): every mode's values join the scale;
   a value on either mode's scale is on scale.
2. Fewer than 3 distinct document values: falls back to the next source; the finding says why.
3. Negative item spacing: not checked (intentional overlap).
4. Auto-layout `space-between` with a gap value: the gap is ignored by layout; not checked.
5. Font size 15 with a text style "Body" at 15 that the node does not use: 15 is on the type scale
   (style sizes are scale steps); the separate `detached-text-style` rule may still apply.
6. A colour equal to a variable that exists only in a disabled library: not a candidate (#155).
7. Instance children: checked on the main component only (instances inherit), as Check designs does.

## 9. Non-goals

- Inferring a scale from frequency of raw values in the document.
- Line-height and letter-spacing scales (line height is matched for tokens only, via #154).

## 10. Test plan

Seam: designos-doc unit tests.

| Case | Fixture | Expected |
|---|---|---|
| `gap_15_on_4_step_scale_suggests_16` | no variables (default scale), auto layout gap 15 | `off-scale`, nearest 16, suggested fix present, `safe:false` |
| `variables_define_the_scale` | GAP variables 4, 8, 12, 16, 24; gap 20 | off-scale, nearest tie 16/24 → no fix; `scale_source:"variables"` |
| `bound_gap_never_off_scale` | gap bound to `Spacing/odd` = 15 | no finding |
| `colour_equal_to_primary_gets_bind` | fill #0D99FF; one variable `Color/Primary` | `hardcoded-value`, fix `var:Color/Primary` |
| `ambiguous_colour_no_fix` | two variables with #0D99FF | candidates listed, `suggested_fix: null` |
| `check_designs_and_linter_identical` | the Check designs fixture (`crates/designos-app/src/check_designs/fixture.rs`) | `audit()` hard-coded findings == `designos_lint` `hardcoded-value` findings (node, property, fix) |
| `type_scale_from_text_styles` | styles at 12, 14, 16, 20; text at 13 | tie 12/14 → no fix |

Fail-before: `check_designs_and_linter_identical` cannot compile today (matcher private to designos-app).

## 11. Performance budgets

Index build once per lint run: O(variables × modes + styles), under 5 ms for 5,000 variables. Per node
O(properties) hash lookups. The off-thread full scan budget is E15-9's; these rules add under 10% to
it (measured and recorded).

## 12. Dependencies and build order

After E15-9 and after #154/#155 have landed their matcher extensions (this task moves the extended
matcher). Parallel with E15-10. The PR that registers `hardcoded-value` and `off-scale` also appends
them to E15-9's built-in `agent` preset and extends `presets_name_only_registered_rules`.

## 13. Owner rulings

- R2.7 (D-E3-3): exact value matches only; ambiguous matches are reported without a fix.
- #154 amendment (2026-10-18): the matcher is a shared library function; E15-11 and E15-13 call it
  instead of re-implementing it.
- Auto-fix (2026-10-18, tiers revised 2026-10-18 by the owner): only unique exact binds are safe-tier;
  off-scale snaps change spacing and are suggest-only (E15-13).
