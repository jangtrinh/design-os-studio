# #154 Check designs: flag hard-coded numbers that exactly match number variables

- Issue: https://github.com/jangtrinh/design-os-studio/issues/154 (task E3-10, epic #16, M / P1)
- Spec base: `main` at `767acf9`.
- Open PR: none. Claimed by mac-worker-1 on 2026-10-18 14:41Z.
- Files: `crates/designos-app/src/check_designs/hardcoded.rs` (or a new `check_designs/numbers.rs` to keep files under 200 lines), `fix.rs`, `mod.rs`, `tests/numbers.rs` (new), `fixture.rs`.

## 1. Summary and user story

As a design-system maintainer, I want Check designs to list layers whose spacing, padding, radius, stroke, font size or line height is typed by hand but exactly equals a number variable. A one-click Bind fix should link them, so later token changes reach those layers.

## 2. Current state

- FACT: `hardcoded::Index` (`hardcoded.rs:33-66`) indexes colour variables only (`VariableKind::Color`, `hardcoded.rs:42`).
- FACT: variable kinds are `Color`, `Number`, `String` and `Boolean` (`designos-doc/src/variables.rs:44-49`).
- FACT: number binding keys and their scopes are listed in `variables_apply.rs` (`scope_allows`, `variables_apply.rs:36-62`):
  - `topLeftRadius`, `topRightRadius`, `bottomLeftRadius`, `bottomRightRadius` → `CORNER_RADIUS`;
  - `itemSpacing`, `counterAxisSpacing`, `paddingLeft`, `paddingRight`, `paddingTop`, `paddingBottom` → `GAP`;
  - `strokeWeight`, `strokeTopWeight`, `strokeRightWeight`, `strokeBottomWeight`, `strokeLeftWeight` → `STROKE_FLOAT`;
  - `fontSize` → `FONT_SIZE`;
  - `lineHeight` → `LINE_HEIGHT`.
- FACT: applying a number binding writes the node value (`variables_apply.rs:207-270`). For example `lineHeight` writes `LineHeight::Pixels(x)`, and padding writes `al.padding[i]`.
- FACT: the colour rule's tie-breaking: a variable constant across modes wins, and several equal candidates make the finding ambiguous with no fix (`hardcoded.rs:68-82`). That is PR #45 finding M2.

## 3. Behaviour rules

1. **Properties checked**, each a binding key from §2:
   - **Corner radii.** When all four corner keys are unbound and equal, report one grouped finding labelled `Corner radius`; its fix binds all four keys. Otherwise report one finding per unbound corner key. There is no single `cornerRadius` binding key: keys are per corner (`variables_apply.rs:53`).
   - **Gaps.** `itemSpacing` (`Gap`) and `counterAxisSpacing` (`Row gap`), one finding each.
   - **Padding.** When all four padding keys are unbound and equal, report one grouped finding labelled `Padding`; its fix binds all four. Otherwise report one finding per unbound side key (`Padding top`, `Padding right`, `Padding bottom`, `Padding left`). No pair grouping.
   - **Stroke weight.** `strokeWeight` when `side_weights` is `None`. Otherwise one finding per per-side key.
   - **Font size.** `fontSize`.
   - **Line height.** `lineHeight`, only when the node's line height is `LineHeight::Pixels(x)`. Percent and auto are skipped.
2. **Candidates.** Number variables whose value equals the layer value **exactly**, and whose scopes allow the key for the node kind (`scope_allows`).
   - Exact means `|a − b| ≤ 0.0005`, which absorbs f32 storage noise only and is not a design tolerance (ruling R2.7).
   - A variable "holds" the value when `doc.resolve_variable(node.id, var.id)` (the node's effective mode) equals it.
   - Variables with the same value in every mode of their collection are preferred, as in the colour rule.
3. **Outcome.**
   - Exactly one preferred candidate: a finding `"{Label} {value} matches variable \"{name}\""`, with `Fix::BindVariable { key, variable }`.
   - Two or more: a finding `"{Label} {value} matches {n} variables: {names}"`, with no fix (AC3).
   - None: no finding.
4. **Already handled.** A key already in `n.variables.bindings`, or a value of 0 for padding, spacing, radius or stroke, is skipped. A zero gap matching `space/0` is not useful noise.
5. **Labels:** `Corner radius`, `Gap`, `Row gap`, `Padding`, `Padding top/right/bottom/left`, `Stroke weight`, `Font size` and `Line height`.
6. **The fix** binds through the existing `Fix::BindVariable` path. For grouped findings (all four corner keys, all four padding keys), the fix binds each member key in one `Tx` with `cmds_all_or_none`. It is one undo step.
7. **Stale guard.** At apply time each key must still be unbound and still hold the variable's value (a `number_bind_is_current` sibling of `bind_is_current`, `hardcoded.rs:236-247`). Otherwise the fix is refused, the same way the colour fix is.
8. **Rule group.** These findings are listed under the existing `Hard-coded values` group (`Rule::HardCodedValue`).

## 4. Data model / schema changes

`Fix::BindVariable { key, variable }` gets a sibling `Fix::BindVariables { keys: Vec<String>, variable: String }` for grouped findings. It is in memory only.

## 5. Contracts

```rust
pub(super) struct NumberIndex<'a> { /* value-bits -> Vec<Entry> */ }
impl<'a> NumberIndex<'a> { pub(super) fn new(doc: &'a Document) -> Self; fn find(&self, doc: &Document, node: &Node, key: &str, x: f32) -> Match<'a>; }
pub(super) fn check_numbers(doc: &Document, idx: &NumberIndex, n: &Node, out: &mut Vec<Finding>);
pub(super) fn number_bind_is_current(doc: &Document, id: NodeId, key: &str, variable: &str) -> bool;
```

The index key is the value rounded to 1/1000 (`(x * 1000.0).round() as i64`), so `find` is O(1) per property.

## 6. File-format mapping

None in this task. RESOLVED.

REA (E3-d), from the Figma 126.9.13 editor bundle: Figma's own number rules are:
- `REQUIRE_CORNER_RADIUS_VARIABLES`, covering `rectangleTopLeftCornerRadius`, `rectangleTopRightCornerRadius`, `rectangleBottomLeftCornerRadius` and `rectangleBottomRightCornerRadius` ("Detects missing number variables for corner radius fields, and applies a fix to use the most appropriate variable");
- `REQUIRE_PADDING_VARIABLES` (dependent fields `stack-padding-top`, `-right`, `-bottom`, `-left`);
- `REQUIRE_HORIZONTAL_SPACING_VARIABLES` (`stack-spacing`);
- `REQUIRE_VERTICAL_SPACING_VARIABLES` (`stack-counter-spacing`, `stack-wrap`).

Figma has no stroke-weight, font-size or line-height number rule in this build. Studio covers them because the issue AC requires them.

## 7. UI

- **Rows.** Findings appear as rows in the existing Hard-coded values group, with the existing `Fix` button (≥24 px).
- **Ambiguous findings.** No Fix button, and the message lists the names.
- **Evidence.** A 1440 capture of the panel with a gap finding and an ambiguous radius finding.

## 8. Edge cases

1. A vertical auto-layout frame with gap 16 and variables `space/4 = 16` (all modes): finding `Gap 16 matches variable "space/4"`, with Bind (AC1).
2. `radius/md = 8` and `radius/card = 8`, both constant: radius 8 is ambiguous and has no fix (AC3).
3. `space/4 = 16` in Light and `20` in Dense; the node is in Light; no other candidate. Finding with Bind: the variable is the only holder, though not constant.
4. Gap 15.9996 against a variable of 16: matches (storage noise). Gap 15.9 does not.
5. Line height 150% (percent): skipped.
6. A variable scoped only to `FONT_SIZE`: never suggested for a gap.
7. Padding `[16, 16, 16, 16]` with `space/4`: one finding binds all four in one undo step.
8. Already bound `itemSpacing`: no finding.

## 9. Non-goals

- Nearest-value suggestions (ruling R2.7 forbids them).
- Library variables (#155).
- Effects, opacity, width and height.

## 10. Test plan

Seam: `crates/designos-app/src/check_designs/tests/numbers.rs`. Literal fixtures: a collection `Spacing` with modes `Light` and `Dense`.

| Test | Expected | Fails before |
|---|---|---|
| `gap_16_matching_space_4_is_flagged_with_bind_fix` | 1 finding; after Fix `bindings["itemSpacing"] == space/4`; one undo | yes |
| `each_number_property_is_covered` (radius, gap, row gap, padding, stroke, font size, line height px) | 7 findings | yes |
| `ambiguous_number_match_has_no_fix` | `fix == None`, message lists both names | yes |
| `percent_line_height_is_skipped` | 0 | guard |
| `scope_excludes_wrong_property` | 0 | guard |
| `stale_number_fix_is_refused` | change gap to 17 before apply: `check_designs_fix` returns false | yes |
| `grouped_padding_fix_binds_four_keys_in_one_undo` | | yes |

## 11. Performance budgets

- `CLAUDE.md` §4.1: the audit runs per `rev`, never per frame.
- The `NumberIndex` is built once per audit. It is O(variables × modes), and each property check is O(1).

## 12. Dependencies and build order

- Depends on #35 (merged).
- Blocks #155.
- Do it before #157 so the off-thread scan carries all rules.

## 13. Owner rulings that apply

- **R2.7 / D-E3-3** (`owner-decisions-and-cross-check.md` round 2, item 7): exact value matches only. Ambiguous matches are reported without a fix.
- Owner default: 1440 capture only.
