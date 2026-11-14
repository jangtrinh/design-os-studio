# Spec B — Untitled type, spacing, radius and control-size scale in `theme.rs` (R5)

| Field | Value |
|---|---|
| Issue | #1143 (epic #1118, spec key B) |
| Size | L. About three days: ~40 `theme.rs` lines, 15 new constants and 2 renamed (`CONTROL_TOOL` → `CONTROL_XS`, `CONTROL_ROW` → `CONTROL_MD`), one mechanical rename across 45 references outside `theme.rs` (42 in `crates/`, 3 in `scripts/`), 7 theme tests rewritten, 38 existing tests updated (37 Rust, 1 script), 7 failing tests made green by product fixes, 2 open decisions (section 10.1), 10 token re-values, two ratchet fixes, one ratchet category added, evidence. |
| Depends on | #1164 (R12, its own small PR: re-seal the token store; at `8dfaee93` `ui ds status` and `ui ds change-token` still exit 1, re-run 2026-10-28, because #1106 hand-edited `spacing.rail-width`) and A (same files: `theme.rs`, `design/design.tokens.json`, `scripts/non_colour_token_parity.py`). One writer, B's PR opens after A merges. |
| Unblocks | C (widget anatomy uses these sizes), D–K (re-point call sites), L (height growth to allow) |
| Touches | `crates/designos-app/src/theme.rs` (type :116-139, `font_*` :141-184, radii :190-196, controls :255-260, derived sizes :287, :311, :316, :351, :367, :370, :399, :424, :590, :735, tests :777-920), 45 references renamed in `crates/` and `scripts/` (section 5.2), `crates/designos-app/src/cmdk.rs:456`, `crates/designos-app/src/dev/list.rs:60,67`, the tests and product sites of section 10.1 (`shell/tests/{layer_drag_fast,menus,dev,agents_tab_strip,agent_turn_card_scroll,canvas_label_rename}.rs`, `shell/tests.rs`, `panels/layers.rs` tests, `widgets/mod.rs` tests, `agent_start_failure.rs` tests, `agent_open_flow.rs`, `agent_picker.rs`, `panels/export.rs`, the Dev mode "Ready for dev" site, the right-panel footer and type-settings sites), `design/design.tokens.json` (radius :341-366, typography :417-450), `scripts/non_colour_token_parity.py` (:23-58), `scripts/tests/test_non_colour_token_parity.py:69-71, :81`, `scripts/rust_literal_scan.py` (:27-32 categories), `scripts/spacing_baseline.json` |

Code baseline: `8dfaee93` (`origin/main` with v0.7.0 merged, 2026-10-28), the implementation base; every cited line was re-read there (the earlier anchors were `ec47da26` and `d0555c82`). #1106 removed three rail constants and made the rail 48 px and icon-only, and #1108 added a draggable Pages/Layers separator (`panels/left_sep.rs`, 8 theme constants after `PAGES_MAX_VISIBLE_ROWS`, `LAYER_FILTER_ROW_HEIGHT = CONTROL_TOOL`, which the 5.2 rename covers). Tools run on v0.7.0 (`plans/261025-1247-untitled-ui-dark-revamp/tools/designos-latest`); code is cited at `origin/main`, not at a release branch. Re-anchor every cited line against HEAD (and against A's merged diff) before writing code.

**Drift note (`d0555c82` → `8dfaee93`, 215 files under `crates/designos-app/src` changed; checked 2026-10-28).** `theme.rs` is byte-identical, so every theme line, value and the 302-constant appendix hold (token-contract.md (g) D-1, exit 0). Counts that moved: `CONTROL_STANDARD` 47 → 49 references (15 → 16 files), `RADIUS_CONTROL` 356 → 360, legacy-name references 102 → 104, literal geometry (ratchet) offset 1675 → 1655 and radius 260 → 253; cites that moved: `panels/left.rs` +12 (`show` :27-208, title :211), `panels/properties.rs` :206-312, `panels/export.rs` -1 and +4, `shell.rs:94` → `:106`, `shell/tests.rs` +9, `widgets/mod.rs` +7, `agent_composer.rs` +34. v0.7.0 changed behaviour that section 10.1 rests on (hug text, text styles, effect caps, approval cards): 10.1 was re-measured on the A+B scratch tree at `8dfaee93`. Rules the drift breaks: none; the list of what B breaks changed (10.1).

Value tables: `specs/token-contract.md` (c) and (d). This spec says why each value is what it is, how to apply it and how to prove it.

## 1. Summary and user story

As the owner, I want Studio's chrome to use Untitled's type, spacing, radius and control sizes (R2), at the small end of the scale in editor panels and at sm/md in Home and dialogs (R5), without moving any region, panel or control (R1).

As a surface implementer (C–K), I want one constant per Untitled step (`text-xs`, `radius-md`, button `xs` …) in `theme.rs`, invariant tests that keep them on the kit's scale, and a ratchet that tells me when a site I touch becomes non-concentric.

## 2. Current state (FACT unless labelled)

**Type.** `FONT_SIZE_CAPTION` 10 (:117), `FONT_SIZE_CANVAS` 9 (:119), `FONT_SIZE_BODY` 11 (:120), `FONT_SIZE_LARGE` 13 (:121), `FONT_SIZE_TITLE` 20 (:122), `LINE_BODY` 16 (:124), `LINE_CAPTION` 14 (:125), weights `UI_FONT_WEIGHT` 450 (:131), `UI_FONT_WEIGHT_STRONG` 550 (:132), `UI_FONT_WEIGHT_MEDIUM` 500 (:133). Panel titles use `font_body_strong` 11/550 (`panels/left.rs:211`, `panels/properties.rs:213`). Usage outside `theme.rs` at `8dfaee93`: `font_body` 334, `font_caption` 222, `font_body_strong` 170, `font_caption_medium` 79, `font_caption_strong` 72, `font_mono` 67, `font_body_medium` 50.

**Kit type scale (FACT, with the command that produced it).**

| Untitled style | Size / line height | Source |
|---|---|---|
| Text xs | 12 / 18 | Styles panel, `designos "$KIT" --select none --screenshot styles.png --size 1600x7000 --no-toast`, right 240 px |
| Text sm | 14 / 20 | same; also the only typography variable (`--export-variables`: `6_ Typography` `text-sm` = 14) |
| Text md | 16 / 24 | Styles panel |
| Text lg | 18 / 28 | Styles panel |
| Text xl | 20 / 30 | Styles panel |
| Display xs … 2xl | 24/32, 30/38, 36/44, 48/60, 60/72, 72/90 | Styles panel; letter spacing −2% (`_Type scale base` label) |
| Weights | Regular 400, Medium 500, Semibold 600, Bold 700 | `designos "$KIT" --select "_Type scale base" --export-spec type-scale-base.json` ("Font weight: 400" … "700"); typeface Inter |

**Verified from variables (FACT).** The DTCG export loses the kit's local `6. Typography` collection (32 variables) behind a one-variable library copy of the same name (spec P, #1140); that is why the plan saw only `text-sm`. The REST export keeps both by id: `designos "$KIT" --export-variables all.json --format figma-rest` gives Font size text-xs 12, sm 14, md 16, lg 18, xl 20, display-xs … 2xl 24, 30, 36, 48, 60, 72; Line height 18, 20, 24, 28, 30 and 32, 38, 44, 60, 72, 90; Font weight Regular, Medium, Semibold, Bold; family Inter. Every value matches the Styles panel reading above. `2_ Radius` and `3_ Spacing` exist once each and are unaffected.

`--codegen css --select Typography` emits only the root node and `--export-spec` omits instance text overrides, so the sizes were read from the rendered Styles panel (UNKNOWN beyond what that panel shows: tracking for Text styles, assumed 0).

**Kit sizes (FACT, `--select "<set>" --export-spec`).** `Buttons/Button` heights xs 32, sm 36, md 40, lg 44, xl 48, all `radius-md`; `Input field` sm 36, md 40, lg 44, `radius-md`; `Badge` sm 22, md 24, lg 28 (pill style `radius-full`, colour/modern `radius-sm`, lg `radius-md`); `_Tab button base` sm 36 `radius-sm`; `_Nav item base` 38; dropdown list item 38 with content `radius-sm`; toggle md 24, slim 20, `radius-full`; checkbox sm 16 `radius-xs`.

**Kit radius and spacing (FACT, `--export-variables`, `2_ Radius`, `3_ Spacing`).** Radius: none 0, xxs 2, xs 4, sm 6, md 8, lg 10, xl 12, 2xl 16, 3xl 20, 4xl 24, full 9999. Spacing: none 0, xxs 2, xs 4, sm 6, md 8, lg 12, xl 16, 2xl 20, 3xl 24, 4xl 32, 5xl 40, 6xl 48, 7xl 64, 8xl 80, 9xl 96, 10xl 128, 11xl 160.

**Radii and controls in code.** `RADIUS_MARK` 2 (:190), `RADIUS_SEGMENT` 4 (:192), `RADIUS_CONTROL` 6 (:194), `RADIUS_FRAME` 14 (:196); `CONTROL_DENSE` 24 (:255), `CONTROL_STANDARD` 28 (:256), `CONTROL_TOOL` 32 (:257), `CONTROL_ROW` 40 (:258), `TARGET_MIN` 24 (:260). References outside `theme.rs` (`grep -rnow <NAME> crates/designos-app/src --exclude=theme.rs | wc -l`, counted at `8dfaee93`): `CONTROL_DENSE` 55 in 13 files, `CONTROL_STANDARD` 49 in 16, `CONTROL_TOOL` 3, `CONTROL_ROW` 39; `RADIUS_CONTROL` 360, `RADIUS_FRAME` 114. `CONTROL_DENSE` sites include chips and pills (`agent_composer.rs:974-979`, `agent_selection_chip.rs:166`, `canvas/agent_overlay.rs:150`), buttons (`agent_open_flow.rs:419`, `agent_picker.rs:381`) and header strips (`terminal.rs:5928`).

**Spacing.** The ladder (:228-243) and insets (:246-252) already hold Untitled values: 4, 4, 8, 12, 16, 24, 48 and 2, 8, 8, 12, 16, 16, 24. The test `ladder_insets_and_controls_sit_on_the_4px_grid` (:822-826) forbids 2 and 6 except `INSET_SEGMENT`.

**Tests that pin today's scale.** `control_heights_are_the_ratified_family` (:850-855), `nested_radii_are_concentric` (:858-866), `menu_pills_inset_by_the_frame_inset` (:869-872), `type_roles_keep_their_order…` (:882-889, strictly increasing, `[10, 11, 13, 20]`), `line_heights_fit_their_roles` (:891-899, slot `CONTROL_DENSE`), `only_three_ui_weights_exist` (:902-904, `[450, 500, 550]`).

**Ratchet.** `scripts/check_spacing_relationships.py` + `scripts/spacing_baseline.json` (version 1, 81 files; live counts at `8dfaee93`: offset 1655, inset 45, radius 253, font 25, nonconcentric 2; 17 entries below baseline). `nonconcentric_pairs` resolves radii through the `theme.rs` constants (`rust_literal_scan.py:265-312`), so changing a radius constant can raise a file's count without touching it.

## 3. Behaviour rules

1. **Type roles (R5).** `FONT_SIZE_CAPTION` and `FONT_SIZE_BODY` become 12 (`text-xs`), `FONT_SIZE_LARGE` 14 (`text-sm`), `FONT_SIZE_TITLE` stays 20 (`text-xl`); `LINE_BODY` and `LINE_CAPTION` become 18. `FONT_SIZE_CANVAS` 9 stays (canvas exception; rulers and canvas pills only), and `FONT_SIZE_LABEL_CANVAS` 11 stays; the canvas frame labels (`canvas/draw.rs:155` reads `font_body()` today) are an open decision in section 10.1. Caption and body now share size and weight: `font_caption()` and `font_body()` become the same `FontId` (12, Proportional), so colour and position tell a caption from a name; weight separates only the `_medium` and `_strong` helpers. Tests that told them apart by size are updated in 10.1.
2. **Untitled steps (new names).** `FONT_SIZE_TEXT_XS` 12, `_SM` 14, `_MD` 16, `_LG` 18, `_XL` 20 and `LINE_TEXT_XS` 18, `_SM` 20, `_MD` 24, `_LG` 28, `_XL` 30, plus `pub fn font_text(size: f32, weight: TextWeight) -> FontId` and `pub enum TextWeight { Regular, Medium, Semibold }` (new names). Home and dialogs (specs H and J) move their text to `font_text(FONT_SIZE_TEXT_SM, …)`; until they do, Home text renders at 12 (interim, sequential phases).
3. **Weights.** `UI_FONT_WEIGHT` 400 (Regular), `UI_FONT_WEIGHT_MEDIUM` 500 (Medium), `UI_FONT_WEIGHT_STRONG` 600 (Semibold). Bold 700 is Display-only and has no chrome constant. The legacy role helpers (`font_caption*`, `font_body*`) keep their names; no mass rename.
3a. **Button labels (R8.6).** Button labels at every size, xs 32 included, are `text-sm` 14 Semibold 600 (`font_text(FONT_SIZE_TEXT_SM, TextWeight::Semibold)`), as the kit's `Buttons/Button` xs. This is for buttons only: row, label and tab text stays `text-xs` 12 (R5).
4. **Radii.** `RADIUS_CONTROL` 8 (`radius-md`, R5), `RADIUS_SEGMENT` 6 (`radius-sm`), `RADIUS_FRAME` 16 (`radius-2xl`, R8.5: menus, dropdowns and popovers, items 8 at inset 8), `RADIUS_XL` 12 (`radius-xl`, new name, G-4: cards and nested panels holding controls at inset 4), `RADIUS_MARK` 2 (`radius-xxs`, unchanged).
5. **Concentric law, with numbers.** `RADIUS_FRAME` 16 − `INSET_FRAME` 8 = `RADIUS_CONTROL` 8; `RADIUS_CONTROL` 8 − `INSET_SEGMENT` 2 = `RADIUS_SEGMENT` 6; `concentric(RADIUS_FRAME, MENU_PILL_INSET 8)` = 8 = `RADIUS_CONTROL`; `RADIUS_XL` 12 − 4 = 8 = `RADIUS_CONTROL` (a card with a 4 px inset); chain 16 > 12 > 8 > 6 > 2 strictly decreasing. A pill inside a pill: `radius_full(h_outer) − inset = radius_full(h_outer − 2·inset)`, e.g. a 32 track with inset 2 holds a 28 thumb: 16 − 2 = 14 = 28 ÷ 2.
6. **Controls (R5).** `CONTROL_XS` 32 (button xs; editor buttons, rows, fields) and `CONTROL_MD` 40 (button and input md; panel title rows) are the renamed `CONTROL_TOOL` and `CONTROL_ROW` (rule 7, values kept). New names: `CONTROL_SM` 36 (button and input sm; Home, dialogs), `BADGE_SM` 22, `BADGE_MD` 24, `BADGE_LG` 28. `TARGET_MIN` 24 stays the floor. **B changes neither `PANEL_ACTION_BUTTON` nor `PAGES_SEARCH_DX`:** `PANEL_ACTION_BUTTON` stays `TARGET_MIN` 24 (two 32 px buttons at x 209 would overflow the 240 px panel) and `PAGES_SEARCH_DX` stays 28 (frozen) until spec E removes its last non-test reader, when spec A's rule 21 deletes it (no `#[allow(dead_code)]`; `RIGHT_PRESENT_FROM_RIGHT`, spec F, follows the same rule). A surface that needs another size or offset declares a local name in its own file (spec E for the left panel); C owns icon-button anatomy. This is the one place B states it.
7. **Renames.** `CONTROL_TOOL` → `CONTROL_XS` and `CONTROL_ROW` → `CONTROL_MD` (values kept; section 5.2). `CONTROL_DENSE` 24 and `CONTROL_STANDARD` 28 keep name and value as legacy: their sites mix buttons, chips and strips, so each surface spec re-points the sites it owns (button or row → `CONTROL_XS`, chip or pill → `BADGE_MD`/`BADGE_LG`, icon hit box → `TARGET_MIN`), and a ratchet stops new uses (rule 11).
8. **Re-pointed derived sizes.** `PAGE_ROW_HEIGHT`, `MENU_ITEM_HEIGHT`, `FIELD_HEIGHT` (R8.4), `FIND_FIELD_H`, `FIND_GROUP_ROW_H`, `STYLE_FOLDER_ROW_H` → `CONTROL_XS`; `SUBLABEL_HEIGHT` → `LINE_CAPTION`; `VAR_PILL_H` → `BADGE_MD`; `LABEL_BOX_H` and `CODE_LINE_HEIGHT` follow their line heights. `RIGHT_TABS_H` (:337) stays `CONTROL_DENSE` 24 (R10.1, owner: "A · Giữ như hiện tại"; R1 wins over R5 for tab heights): the Inspector segmented strip, file tabs (26 in the 32 bar) and session tabs (24) keep today's heights. Every other numeric constant keeps its value (302, token-contract.md appendix; evaluated at `8dfaee93` with A and B applied: 19 values change plus `VAR_PILL_H`'s re-point, 0 implicit changes).
9. **Spacing and control sizes.** Every ladder and inset value is a step of the Untitled spacing scale {0, 2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 96, 128, 160}; the 2 px and 6 px steps are allowed (R5, which replaces the 4px grid). Control heights are Untitled component sizes, not spacing steps: button and input {32, 36, 40, 44, 48}, badge {22, 24, 28}, plus `TARGET_MIN` 24; tab heights are the R10.1 carve-out (rule 8). Ladder and inset values do not change in B. Kit button paddings (10, 14, 18) are component anatomy (spec C).
10. **Ratchet baseline policy.** `scripts/spacing_baseline.json` is never raised. A rise caused by a re-valued theme constant is fixed in code in the same PR by deriving the inner radius from the law (`RADIUS_CONTROL` or `theme::concentric(outer, inset)`) or by moving the inset onto a theme constant. Gains may be locked with `--write-baseline`; the PR diff of the baseline shows decreases only.
11. **Legacy-name ratchet.** `scripts/rust_literal_scan.py` gains the category `legacy_control_refs` (new name, regex `\bCONTROL_(DENSE|STANDARD)\b`, code only), counted per file outside `theme.rs` into the same baseline; B writes its initial counts (104 references at `8dfaee93`: 55 + 49) once; afterwards they may only fall. When a constant reaches 0 references, the PR that removes the last one deletes the constant and its `RUST_ONLY` / `TOKEN_TO_RUST` row, except `CONTROL_DENSE`, which stays as the base of `RIGHT_TABS_H` inside `theme.rs` (R10.1).
12. **Layout unchanged (R1).** No width, x position, region or order changes. Heights grow only where rules 1, 6 and 8 raise a size (section 7). Where 12 px text no longer fits a fixed box, B fixes the site in its own PR (section 10.1); it never ships an elided label or an overflow.
13. **Staged constants.** A new constant that no call site uses yet (`CONTROL_SM`, `BADGE_*`, `FONT_SIZE_TEXT_MD/LG/XL`, `LINE_TEXT_*`, `RADIUS_XL`) carries `#[allow(dead_code)]`, the existing `theme.rs` pattern (:45, :64); the surface spec that first uses one removes it. CI Clippy has no `-D warnings`, but CONTRIBUTING.md:29 asks for it locally, so a staged constant without the attribute fails the contributor's own check.

## 4. Data model / schema changes

`theme.rs`: 15 constants added (new names: `CONTROL_SM`, 3 × `BADGE_*`, 5 × `FONT_SIZE_TEXT_*`, 5 × `LINE_TEXT_*`, `RADIUS_XL`), 2 renamed (`CONTROL_XS`, `CONTROL_MD`), 20 changed or re-pointed (token-contract.md (c); `RIGHT_TABS_H` held by R10.1); `RADIUS_*` stay `u8`. `design/design.tokens.json`: 10 re-values with `ui ds change-token` (token-contract.md (c) tsv: `font-size-caption` 12px, `font-size-body` 12px, `font-size-title` 14px, `font-size-heading` 14px, `radius-xs` 4px, `radius-sm` 6px, `radius-md` 8px, `radius-lg` 10px, `radius-xl` 12px, `radius-pill` 9999px); generation = the generation after A + 10 (scratch: 72 → 82). Store keys unchanged (no add path, kernel gap T1 in spec A).

## 5. Contracts

### 5.1 Token re-values (scratch run, pasted)

```bash
while IFS=$'\t' read -r tok value reason; do      # not `path`: zsh ties `path` to `PATH`
  ui ds change-token "$tok" --value "$value" --reason "Studio scale: $reason" --dir . || exit 1   # Studio's wording only (R14)
done < scale.tsv          # = the tsv block in token-contract.md (c)
```

Scratch result under zsh on the A-re-valued copy (`design/` of `8dfaee93`, #1164 re-seal emulated, A's 5.1 applied; 2026-10-28): 10/10 succeeded, generation 72 → 82, roles kept, `ui ds status` "tokens: 77 … components: 12". The reasons are Studio's wording (R14); the Untitled step names stay in token-contract.md (c) only.

### 5.2 Mechanical rename

```bash
grep -rlwE 'CONTROL_(TOOL|ROW)' crates scripts | xargs perl -pi -e 's/\bCONTROL_TOOL\b/CONTROL_XS/g; s/\bCONTROL_ROW\b/CONTROL_MD/g'
grep -rnwE 'CONTROL_(TOOL|ROW)' crates scripts      # prints nothing
```

Counted at `8dfaee93` (identical at `d0555c82`; `grep -rhwoE 'CONTROL_(TOOL|ROW)'`): 42 references in `crates/` outside `theme.rs` (3 `CONTROL_TOOL`, 39 `CONTROL_ROW`), 3 in `scripts/` (`non_colour_token_parity.py` 2, `tests/test_non_colour_token_parity.py:81` 1) and 16 inside `theme.rs` (the two definitions and their uses, including `LAYER_ROW_HEIGHT` and #1108's `LAYER_FILTER_ROW_HEIGHT`). On a scratch copy of the full tree the perl renames all of them, 0 left. Shipped specs in `docs/specs/v0.6/` keep the old names (records).

### 5.3 `scripts/non_colour_token_parity.py`

`TOKEN_TO_RUST` and `RUST_ONLY` exactly as token-contract.md (d). Scratch run 2026-10-28 (the repo's G01 with A's and B's edits applied to a scratch copy of the scripts, `theme.rs` at `8dfaee93` with A and B applied, scratch store after A's and B's re-values): `G01 ok = True`, "35/35 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 30 non-colour constants match" (with `radius.radius-xl` → `RADIUS_XL`); unmapped (information only): `radius-lg`, `radius-pill`, `radius-xs`, `ai-panel-width`, `font-size-title`. `python3 -m unittest tests.test_non_colour_token_parity` (scratch, with test 8's two edits): 11 tests, OK.

### 5.4 Ratchet on the scratch tree (pasted)

A scratch copy of the full tree at `8dfaee93` with A's and B's `theme.rs` and the 5.2 rename, against `scripts/spacing_baseline.json` of `8dfaee93` (2026-10-28):

```text
$ python3 scripts/check_spacing_relationships.py --src <HEAD src> --baseline scripts/spacing_baseline.json      # HEAD, no change
literal geometry outside theme.rs: offset_literals=1655, inset_literals=45, radius_literals=253, font_literals=25, nonconcentric_pairs=2
note: 17 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
$ … --src <scratch with A+B>                                                                                    # before the fixes
literal geometry outside theme.rs: offset_literals=1655, inset_literals=45, radius_literals=253, font_literals=25, nonconcentric_pairs=4
FAIL cmdk.rs: nonconcentric_pairs rose 0 -> 1 (lines 448)
FAIL dev/list.rs: nonconcentric_pairs rose 0 -> 1 (lines 67)
FAIL: ratchet violated
```

Causes (FACT): `cmdk.rs:448` row inset 8 inside `RADIUS_FRAME` (14 → 16) keeps a literal `CornerRadius::same(6)` (:456): 16 − 8 = 8 ≠ 6. `dev/list.rs:67` badge offset `r.left() + 7.0` inside a `RADIUS_CONTROL` tab (6 → 8): inset 7 was clear of the 6 arc and now sits inside the 8 arc. Fixes in B's PR: `cmdk.rs:456` → `theme::cr(theme::RADIUS_CONTROL)`; `dev/list.rs:60,67` `r.left() + 7.0` → `r.left() + theme::INSET_CONTROL` (8, on the arc's edge). After both:

```text
literal geometry outside theme.rs: offset_literals=1653, inset_literals=45, radius_literals=252, font_literals=25, nonconcentric_pairs=2
note: 19 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

## 6. File-format mapping

None. Document text keeps document sizes; only chrome changes.

## 7. UI

- UI implementers load the `es-designer` skill first, then follow `design/principles.md` (P3, P4, P5, P9, P10 as rewritten in `specs/A-principles-draft.md`), `design/soul.md` and this spec; the project's own design system wins over generic defaults.
- Height growth, layout unchanged (R1). INFERENCE from the code at `8dfaee93`, assuming the left panel is 1056 px tall at the default 1600×1088 window (1088 − tab bar 32):
  - Left panel stack (`panels/left.rs:27-208`, `show`; Pages list height from `panels/left_sep.rs` `compute_bounds`, #1108): header 65 + Pages title 40 + Pages list + 8 + Layers header 40. The list's default height is still `min(n,5) × PAGE_ROW_HEIGHT`. Since #1108 the user can drag it (at least one row; Layers keep at least `LAYERS_MIN_HEIGHT` 120), and a dragged height persists in pixels, so after B it keeps its pixels and shows fewer 32 px rows. At the default height with ≥ 5 pages: 65 + 40 + 120 + 8 + 40 = 273 before, 65 + 40 + 160 + 8 + 40 = 313 after; visible Layers rows (32 each) ⌊(1056 − 273)/32⌋ = 24 before, ⌊(1056 − 313)/32⌋ = 23 after. With 1 page: 27 rows before and after.
  - Inspector (`panels/properties.rs:206-312`: `title`, `sublabel`, `row_end`): a labelled group is sublabel gap (12 − 4) + `LABEL_BOX_H` + 4 + `FIELD_HEIGHT` + 4 = 54 before, 66 after (+12); the first group after a title 46 → 58. A section with g groups grows 12g; Position (title 40 + 3 groups) 194 → 230.
  - Menus: `MENU_ITEM_HEIGHT` 24 → 32, a menu of n items grows 8n. Inspector tab strip stays 24 (`RIGHT_TABS_H` = `CONTROL_DENSE`, R10.1), so nothing below `RIGHT_TABS_Y` 48 moves.
  - Text widths: body 11 → 12 (+9%), caption 10 → 12 (+20%). The rail has no text labels to measure: since #1106 it is 48 px and icon-only (`RAIL_WIDTH` 48, `panels/rail.rs`, names in tooltips, which take body 12). Where 12 px text no longer fits, section 10.1 lists the sites and their fixes.
- Evidence to the PR, not Figma (outputs under `evidence/B/`). Home is captured on a seeded sample library, never the owner's own; the editor captures open no library (`shell.rs:106`: with `--screenshot` and a file, no library is opened):

```bash
LIB="$(mktemp -d)/lib"   # seeded sample library; never the owner's own library
cargo run -q --release -p designos-app --bin designos -- --seed-library "$LIB"
cargo run -q --release -p designos-app --bin designos -- --library "$LIB" --home --screenshot evidence/B/home.png --size 1600x1088 --no-toast
cargo run -q --release -p designos-app --bin designos -- "$KIT_COPY" --screenshot evidence/B/editor-kit.png --size 1600x1088 --no-toast
for p in cmdk variables assets picker; do cargo run -q --release -p designos-app --bin designos -- "$KIT_COPY" --panel "$p" --screenshot "evidence/B/panel-$p.png" --size 1600x1088 --no-toast; done
```

Compare Home against spec L's seeded-library baselines (`scripts/layout_baseline_capture.py --compare ~/designos-layout-baseline/<bless sha>`, case `h-home`) and the editor against `evidence/baseline-editor-with-kit.png` (kit content only). No capture made from the owner's library is used or attached.

- responsive: SKIPPED (owner default).

## 8. Edge cases

- `FONT_SIZE_CAPTION == FONT_SIZE_BODY`: any code that picks a font by comparing sizes (none outside `theme.rs`: `grep -rnE "FONT_SIZE_[A-Z]+ *(==|<|>)" crates/designos-app/src` prints only `theme.rs` lines) would change branch; re-check at re-anchor.
- `RADIUS_*` are `u8`: 16, 8, 6, 2 fit.
- A 32 px row in a list that also uses `PAGES_MAX_VISIBLE_ROWS` 5: the Pages list grows 40 px (left panel stack above).
- `CONTROL_DENSE` stays equal to `TARGET_MIN`; do not use either for a new button (rule 7, ratchet rule 11).
- Text truncation: wider text at 12 px may ellipsize labels in 240 px panels; surface specs own truncation checks (L gate).
- The 9 px canvas size must not be used in chrome; the type test keeps it the only size below 12.

## 9. Non-goals

- Colours (A). Widget anatomy, paddings, states, icon-button sizes (C). Re-pointing `CONTROL_DENSE`/`CONTROL_STANDARD` call sites and moving Home/dialog text to `text-sm` (D–K). Display styles in chrome. Changing layout widths or positions (R1). Renaming the `font_*` role helpers.

## 10. Test plan (red first)

Rewritten tests in `theme.rs` (red against HEAD values, green after; lines at `8dfaee93`, identical at `d0555c82`):

1. `ladder_insets_and_controls_are_untitled_spacing_steps` (new name, replaces `ladder_insets_and_controls_sit_on_the_4px_grid` :821), two checks, one per kind (rule 9):
   - every `LADDER` and `INSETS` value ∈ the Untitled spacing set {0, 2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 96, 128, 160};
   - every control height in `CONTROLS` = `[CONTROL_XS, CONTROL_SM, CONTROL_MD, BADGE_SM, BADGE_MD, BADGE_LG, TARGET_MIN]` ∈ the button and input sizes {32, 36, 40, 44, 48} ∪ the badge sizes {22, 24, 28} ∪ {`TARGET_MIN`}. A single spacing-set check cannot hold: 36 and 44 are button sizes, not spacing steps.
2. `control_heights_are_the_untitled_button_sizes` (new name, replaces `control_heights_are_the_ratified_family` :850): `[CONTROL_XS, CONTROL_SM, CONTROL_MD] == [32, 36, 40]`, `[BADGE_SM, BADGE_MD, BADGE_LG] == [22, 24, 28]`, `TARGET_MIN == 24`, `PAGE_ROW_HEIGHT == MENU_ITEM_HEIGHT == FIELD_HEIGHT == CONTROL_XS`; the assert `RIGHT_TABS_H == CONTROL_DENSE` stays as today (`theme.rs:854`, "inspector tab strips are 24px, not 28px"; R10.1).
3. `nested_radii_are_concentric` (:858): unchanged asserts; add `[RADIUS_FRAME, RADIUS_XL, RADIUS_CONTROL, RADIUS_SEGMENT, RADIUS_MARK] == [16, 12, 8, 6, 2]`, `concentric(RADIUS_XL, 4.0) == RADIUS_CONTROL` and membership in the Untitled radius set {0, 2, 4, 6, 8, 10, 12, 16, 20, 24}.
4. `type_roles_are_untitled_text_styles` (new name, replaces `type_roles_keep_their_order_and_the_canvas_size_is_the_only_one_below_the_chrome_floor` :882). The set it scans is named: `[FONT_SIZE_CAPTION, FONT_SIZE_BODY, FONT_SIZE_LARGE, FONT_SIZE_TITLE, FONT_SIZE_TEXT_XS, FONT_SIZE_TEXT_SM, FONT_SIZE_TEXT_MD, FONT_SIZE_TEXT_LG, FONT_SIZE_TEXT_XL]` plus `FONT_SIZE_CANVAS`. Asserts: the role chain is non-decreasing; `[CAPTION, BODY, LARGE, TITLE] == [12, 12, 14, 20]`; `FONT_SIZE_CANVAS == 9` is the only size in the set below 12; `(FONT_SIZE_TEXT_*, LINE_TEXT_*)` == the five kit pairs (12/18, 14/20, 16/24, 18/28, 20/30). Sizes outside this set (for example `FONT_SIZE_MONO`) are out of scope.
5. `line_heights_fit_their_roles` (:891): `LINE_CAPTION == LINE_BODY == 18`; slot `FIELD_HEIGHT` (32): `LABEL_BOX_H + SPACE_FACT <= FIELD_HEIGHT`.
6. `only_three_ui_weights_exist` (:902): `[400, 500, 600]`.
7. `menu_pills_inset_by_the_frame_inset` (:869), `pill_radius_is_half_the_height` (:875), ladder and inset order tests: unchanged and green.
8. `scripts/tests/test_non_colour_token_parity.py`: `:69-71` expects `TOKEN_TO_RUST["typography.font-size-hero"] == ["FONT_SIZE_TITLE", "FONT_SIZE_TEXT_XL"]` (both 20; B maps the token to both); `:81` `CONTROL_ROW` → `CONTROL_MD` (5.2 rename). Run `python3 -m unittest discover scripts/tests`.
9. Ratchet: `legacy_control_refs` fixture test (a temp file with one new `CONTROL_DENSE` fails); the two nonconcentric fixes of 5.4 (red without them).
10. G01 with B's `TOKEN_TO_RUST`/`RUST_ONLY`: green; with `radius.radius-xs` still mapped to `RADIUS_MARK` it fails (4 ≠ 2).

### 10.1 Existing tests that B breaks, and what each needs

Measured on 2026-10-28, not estimated: `cargo test -p designos-app --bin designos --no-fail-fast` on a scratch copy of the full repo at `8dfaee93` with A and B applied (contract (a) and (c), the rule-3 `apply()` lines, the 5.2 rename; the old theme tests left as they are) gave 3444 passed, 50 failed, 12 ignored (`evidence/AB-fix-round-261028/test-breaks-at-8dfaee93.txt`). With A alone the same tree gave 3493 passed and 1 failed, which is A's (`terminal::tests::theme_has_info_success_warning_in_token_parity`, the pins at `terminal.rs:10728-10730`). Of the 49 other failures, 1 passes when run alone (`agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one`: its dot depends on the stub agent's exit timing, and the full run was under load), so B owns 48 cargo failures: 37 expected updates, the 2 theme tests (tests 4 and 6), 7 real regressions and 2 open decisions. One script test outside the cargo run is also an expected update of B's (49 items in all). v0.7.0 changed the list: the 4 `effect_grid_styles` tests of the earlier measurement now pass (`pick()` derives the click from `MENU_ITEM_HEIGHT` and `SEP_H`), and two new failures appeared (`agent_turn_card_scroll::ac05…`, `canvas_label_rename::frame_label_r2…`, the second an open decision).

Rule for every row: no assert is loosened, deleted or `#[ignore]`d. An expected update replaces a literal that encoded the old size with the constant or a measured rect, so the test checks the same behaviour at the new size. A real regression keeps its test unchanged and fixes the product.

**Expected updates (37 Rust tests + 1 script test; the 2 theme tests are tests 4 and 6 above):**

| Tests | n | Cause (FACT unless labelled) | Update and new value |
|---|---|---|---|
| `shell::tests::layer_drag_fast::*` | 22 | Literal Layers row centres 241, 273, 305 (`shell/tests/layer_drag_fast.rs:10-13`) assume the 120 px Pages list | Derive each centre from the recorded row rects (or `LEFT_HEADER_HEIGHT + PANEL_TITLE_ROW_HEIGHT + default list + SPACE_INLINE + LAYER_HEADER_HEIGHT + (i + ½) × LAYER_ROW_HEIGHT`); new values 281, 313, 345 (+40, the list grows 120 → 160) |
| `shell::tests::{dragging_a_page_row_reorders_it, layers_panel_click_and_visibility_toggle, layers_drag_reparents_into_frame, page_context_menu_duplicates_and_deletes}` | 4 | Literal y positions for 24 px page rows and menu items (`shell/tests.rs:462-470`, `:477-483`, `:572-593`, `:604-609`) | Derive from `PAGE_ROW_HEIGHT` and `MENU_ITEM_HEIGHT` (32) |
| `panels::layers::tests::{only_rows_inside_the_viewport_paint_a_type_label, type_label_never_overlaps_the_name_or_the_layer_icon_at_any_depth, type_label_is_clipped_to_the_text_column, type_label_stays_on_wide_rows_and_gives_way_to_the_name_on_narrow_ones, hidden_layers_grey_the_type_label_with_the_name}` | 5 | The tests tell the name from the type label by font size (body 11 vs caption 10); after B both are 12 (rule 1), so the name is counted twice (40 vs 20) or taken for the label (`panels/layers.rs:938-939`, `:1038-1039`, `:1090-1091`, `:1138-1139`) | Identify name and label by position in the row (the label is the right-hand text), not by size. Re-verify that a wide top-level row still shows "Auto layout" (the run reads `Primary …` today); if it no longer fits at 12 px, that is a real regression fixed in `panels/layers.rs` |
| `shell::tests::menus::hovering_opens_a_submenu_and_clicking_a_leaf_runs_it` | 1 | Literal 24 px item steps (`shell/tests/menus.rs:14-17` `main_row_center`, `:38-39`) now hit "Open…", which spawns an `rfd` file dialog (the run prints "no file dialog backend available") | Use `MENU_ITEM_HEIGHT` and `widgets::menu::SEP_H` |
| `shell::tests::dev::language_dropdown_switches_the_code_and_copy_reports` | 1 | Literal item step (`shell/tests/dev.rs:85-89`): option 0 reads `SwiftUi` instead of `Css` | Use `MENU_ITEM_HEIGHT` |
| `shell::tests::dev::dev_mode_component_playground_toggles_props_without_mutating_doc_and_updates_codegen_live` | 1 | A click at a fixed x (`shell/tests/dev.rs:561-562`) puts the caret inside the wider 12 px label, so typing gives "Get starte nowd" (assert at `:567`) | Press End (caret to the end) before typing |
| `widgets::tests::field_text_baseline_is_16px_below_the_top` | 1 | Pins a 24 px field and 11 px text (`widgets/mod.rs:301-307`; assert at `:321`, "baseline 116.5 at ppp 2") | Use `FIELD_HEIGHT` (32); expected baseline = `centered_baseline` = field centre + 0.3638 × 12 (120.37 in the test's frame); rename to `field_text_baseline_is_centred_in_the_field` (new name) |
| `agent_start_failure::tests::panel_paints_cause_output_and_one_working_fix_button` | 1 | `agent_start_failure.rs:497` matches the stderr tail as one painted string; at 12 px it wraps onto two rows ("flags provided but not \ndefined: -mcp-config") | Join the wrapped rows before matching |
| `shell::tests::agent_turn_card_scroll::ac05_three_row_card_has_no_scrollbar_and_preserves_base_layout` (new at 0.7.0) | 1 | Pins the 3-row agent card at 174 and its terminal at 710 (`shell/tests/agent_turn_card_scroll.rs:278-294`); the formula is `2·pad 16 + LINE_BODY + SPACE_FACT 4 + LINE_CAPTION + SPACE_INLINE 8 + 80 + SPACE_INLINE 8 + CONTROL_STANDARD 28`, and `LINE_BODY` 16 → 18 and `LINE_CAPTION` 14 → 18 make the card 180 (the run reads "got 180") | Build the expected height from `theme::LINE_BODY`, `LINE_CAPTION`, `SPACE_*` and `CONTROL_STANDARD` instead of the literals 174 and 710 (new values 180 and 704: 180 is what the run reads, 704 = 900 − 180 − 16 follows from the old 710 = 900 − 174 − 16 and was not run); spec I owns the card, and spec L's gate gets one allow entry (+6 px, R5 raises the row text; R1: width and order unchanged) |
| `scripts/tests/test_non_colour_token_parity.py::test_title_token_is_not_matched_by_name` | 1 | `:69-71` pins `["FONT_SIZE_TITLE"]` | `["FONT_SIZE_TITLE", "FONT_SIZE_TEXT_XL"]` (test 8) |

**Real regressions, fixed in B's PR (7 tests stay as they are; the surface spec named owns the site afterwards; the fixes are the writer's proposals, each unverified until B's PR runs the test, except where a row says it ran):**

| Test | Failure at 12 px | Product fix in B's PR | Surface |
|---|---|---|---|
| `agent_open_flow::tests::resume_notice_copy_fits_the_narrowest_menus_without_eliding` (`agent_open_flow.rs:1365`) | "No earlier session. Start a new one." elides in the narrowest popover | The notice wraps onto a second line instead of eliding | I |
| `agent_open_flow::tests::resume_notice_does_not_outlive_the_menu_that_showed_it` (`agent_open_flow.rs:1396`) | The Sessions button label elides to "Sess…", so the test cannot find it | The button hugs its label (R11, whole pixels) | I |
| `agent_picker::tests::a_long_other_cli_error_keeps_8px_from_the_buttons_and_stays_in_the_card`, `…the_other_cli_caption_keeps_8px_from_the_buttons_and_is_not_cut` (`agent_picker.rs:1323`, `:1313`; the assert is in their shared helper at `:1289`) | The two-line caption ends at the buttons' top (0 px gap, 8 px required) | Button row y = caption bottom + 8, measured from the laid-out caption | I |
| `panels::export::tests::format_labels_fit_their_dropdown` (`panels/export.rs:593-605`) | "512w" is 29.125 px; `SCALE_W` 56 − chevron 20 − 8 = 28 | Chevron slot 20 → 16 (`panels/export.rs:226-227`): 56 − 16 − 8 = 32 | F, anatomy C |
| `shell::tests::lint::design_os_gates_audit_all_studio_screens_zero_overflow_zero_purple_and_valid_contrast` (`shell/tests/lint.rs:185`) | Dev mode "Ready for dev" text [373, 452.7] overflows its container [354, 439.8] | The status control hugs its label (R11) | G |
| `shell::tests::lint::nothing_overflows_and_icons_are_centered` (`shell/tests/lint.rs:58`, 4 findings) | A right-panel value elided to a bare "…" at (1480, 724) in two scenes (values, type settings); "Shader" and "Soon" sit 5.5 px above the 36 px footer [1052, 1088] of the effects menu | INFERENCE: the value field keeps at least one glyph plus the ellipsis (or the column widens within R1); the effect footer labels centre in the footer row from `LINE_CAPTION` 18, not from the old 14 | F |

**Open decisions (2 tests):**

1. `shell::tests::agents_tab_strip::an_exited_tab_title_stays_readable_whether_the_tab_is_active_or_not` (`shell/tests/agents_tab_strip.rs:192`) finds the title by its "Claude" prefix; at 12 px within the 82 px session-tab cap the painted title no longer starts with it. Option 1 (recommended, R1: widths do not change): keep the cap and find the title by the tab's rect; the full title stays in the accessible name (`:165` asserts it). Option 2: raise the cap to 90, a width change that R1 forbids without a ruling. The controller decides; B implements the choice. B's AC-2 cannot pass until it is ruled.
2. `shell::tests::canvas_label_rename::frame_label_r2_four_labels_match_figma_at_zoom_068_and_full_names_at_zoom_2` (`canvas_label_rename.rs:923`; new at 0.7.0). `measure_label` (`canvas/draw.rs:155`) sets the canvas frame labels in `theme::font_body()`: 11 px today, 12 px after B (rule 1), so the label elides at "A frame with a very l…" instead of "…very lon…" and the Figma-parity test fails (1 test). Frame labels are canvas overlay text, not chrome, so they should not follow R5, but the two fixes tried in the scratch tree both fail: (a) a canvas-label font at `FONT_SIZE_LABEL_CANVAS` 11 in the new Regular weight 400 (was 450) makes the text narrower, and 5 label tests fail instead of 1; (b) one extra font instance at weight 450 for the label (`FAMILY_CANVAS_LABEL`) makes the r2 test pass but 4 others fail (one panics because the named family is missing from the test's plain egui context, three assert on elision points). So parity needs either a font instance and test-context setup nobody has designed, or the owner's acceptance that the labels elide at the new weight and size, with the 1 expected string re-measured. This is a ruling for the controller (Figma parity of canvas labels against the R5 weights), not a mechanical fix.

**Coverage of the measurement.** The 3506 tests of the run include the ones added since the first measurement (`shell/tests/pages_layers_separator.rs`, `panel_export.rs`, `text_styles.rs`, `dev_code_panel.rs`, `agent_turn_card_scroll.rs`, `canvas_label_rename.rs`, the rewritten `effect_grid_styles.rs`); each failure is in the tables above and every other test passes. B re-runs the same command at implementation and classifies anything new with the rule above before AC-2 can pass.

Acceptance criteria (generation numbers are relative to the store at B's start, which follows #1164 and A):

1. `theme.rs` scale constants equal token-contract.md (c); `grep -rnwE 'CONTROL_(TOOL|ROW)' crates scripts` prints nothing.
2. `cargo test -p designos-app` green with tests 1–7 and every row of 10.1 resolved as listed (the open decision as the controller rules); no assert loosened, deleted or ignored; `python3 -m unittest discover scripts/tests` green.
3. `python3 scripts/design_os_self_evolve.py` green (G01 "… 30 non-colour constants match" or more); `python3 scripts/check_spacing_relationships.py` PASS with the baseline diff showing decreases only plus the new `legacy_control_refs` entries.
4. `ui ds status --dir .` generation = the generation at B's start + 10 (scratch: 72 → 82); `design.tokens.json` radius and typography equal token-contract.md (c).
5. `python3 scripts/zero_fake_ui_gate.py`, `ui ds soul check --dir .`, `ui agents check --dir .`, `design-os evolution` green.
6. Evidence PNGs of section 7 attached (Home on a seeded library only); L's layout gate (once it exists) shows no region, width or order change; "responsive: SKIPPED (owner default)" stated.

## 11. Performance budgets

Constants only. Taller rows draw fewer Layers rows per frame (24 → 23 at the default window), so no frame-time growth; idle CPU stays `0.00%`. Font atlas: no weight instance is added (400/500/600 replace 450/500/550). Staged constants (rule 13) carry `#[allow(dead_code)]` and cost nothing at run time.

## 12. Dependencies and build order

#1164 (R12, its own small PR) lands first: it re-seals the token store that #1106 left unsealed (at `8dfaee93`, `ui ds status` and `ui ds change-token` still exit 1). Then A merges; B rebases on it and runs 5.1–5.4 on top (one writer, sequential PRs, same files). Then C (anatomy), then D–K re-point sites and retire `CONTROL_STANDARD` and every `CONTROL_DENSE` call site (`CONTROL_DENSE` itself stays as the base of `RIGHT_TABS_H`, R10.1). L's baseline capture must be taken at A+B before D–K start, so their diffs are measured against B's sizes.

## 13. Owner rulings that apply

R1 (layout unchanged; heights may grow only where a size rises), R2 (take type and spacing from Untitled), R5 (editor: text-xs 12/18 rows and labels, text-sm 14/20 panel titles, button xs 32, row 32, radius md 8; Home and dialogs: text-sm 14/20, button sm 36 / md 40; spacing scale incl. 2 and 6, ban lifted), R7 (spec-driven), R8.4 (Inspector field 32), R8.5 (floating chrome radius 16, items 8 at inset 8), R8.6 (button labels text-sm 14 Semibold), R10.1 (editor tab heights stay: file tabs 26, session tabs 24, Inspector strip 24), R11 (hug text width in whole pixels; the 10.1 fixes that hug a label follow it).

## Open questions for the owner

All decided by the owner on 2026-10-25 (plan.md R8); none open.

1. **OQ-4 Inspector field height: decided (R8.4).** 32 (`CONTROL_XS`); Position section 194 → 230. Owner: "A · 32px".
2. **OQ-5 Floating-chrome radius: decided (R8.5).** 16 (`radius-2xl`) for menus, dropdowns and popovers, items radius 8 at inset 8 (concentric). Owner: "A · 16, item 8, inset 8".
3. **Button xs label: decided (R8.6).** `text-sm` 14 Semibold for button labels only; rows, labels and tabs stay `text-xs` 12. Owner: "A · 14px semibold".

## Decisions on the open questions (designer, 2026-10-25)

- Caption and body both take `text-xs` 12 (R5 names one size for rows and labels); they become the same `FontId`, so colour and position separate them (rule 1).
- Only value-preserving renames (`CONTROL_TOOL`, `CONTROL_ROW`); `CONTROL_DENSE`/`CONTROL_STANDARD` are legacy with a falling reference ratchet, because a blanket rename would silently grow chips 24 → 32.
- No mass rename of `font_*` helpers; new Untitled steps sit beside them.
- `PANEL_ACTION_BUTTON` and `PAGES_SEARCH_DX`: rule 6 (stated once there).
- Button labels follow R8.6 (`text-sm` 14 Semibold at every button size); B provides `FONT_SIZE_TEXT_SM` and `TextWeight::Semibold`, spec C applies them.
- `RADIUS_XL` 12 is added for the contract gap G-4 and mapped to `radius.radius-xl` (12 after the re-value), so that token is guarded instead of unmapped.
- Contrast is not B's acceptance: B changes no colour; specs cite contrast check C-1 in token-contract.md (f).
- Ratchet rises from re-valued radii are fixed in code, never by raising the baseline.

## Changelog (curator fix round, review `reviews/AB-contract-curator-review.md`; 2026-10-25, re-anchored and extended 2026-10-28)

One line per finding that touches B; spec A's changelog carries the rest, so every finding has a line in A or B.

- F1 (blocker): #1164 (R12, its own small PR) is a hard predecessor (header "Depends on", section 12); re-run at `8dfaee93` on 2026-10-28 it still fails; AC-4 states the generation relative to B's start (start + 10; scratch 72 → 82 after the re-seal and A, re-run 2026-10-28); cites re-anchored to `origin/main@8dfaee93`.
- F2 (resolved in live): `RIGHT_TABS_H` stays `CONTROL_DENSE` 24 (rule 8, test 2, section 7); section 13 lists R8.4, R8.5, R8.6, R10.1 and R11.
- F3: rule 9 separates spacing steps (ladder, insets) from control heights (button and input {32, 36, 40, 44, 48}, badge {22, 24, 28}, `TARGET_MIN`); test 1 is two checks, one per kind.
- F11: the 5.1 loop variable is `tok`, not `path` (zsh ties `path` to `PATH`); the scratch run is under zsh (re-run 2026-10-28: 10/10, 72 → 82).
- F12: section 10.1 lists every break of the A+B tree, re-measured at `8dfaee93` on 2026-10-28 (the list changed with v0.7.0, see 10.1); no test is loosened; tests the measurement could not classify are named for the PR.
- F15: drift: #1106 (icon-only 48 px rail; the "rail labels" risk in section 7 is moot) and #1108 (draggable Pages/Layers separator; section 7 uses `left_sep.rs` `compute_bounds`) are in the header; cites re-anchored to `8dfaee93` with a drift note (counts, moved lines, what 0.7.0 changed).
- F17: "15 constants added (new names) and 2 renamed" (header, section 4, rule 6).
- F18: test 4 names the set it scans (`CAPTION`, `BODY`, `LARGE`, `TITLE`, `TEXT_*`, plus `CANVAS`); sizes outside it are out of scope.
- F19: rule 6 states once that B changes neither `PANEL_ACTION_BUTTON` (24) nor `PAGES_SEARCH_DX` (28, frozen); the frozen constant is deleted by spec A's rule 21 once its last reader goes (D–G amendment). Specs D and E cite rule 6.
- F20: rule 13: staged constants carry `#[allow(dead_code)]`; CI Clippy has no `-D warnings`, but CONTRIBUTING.md:29 asks for it locally; section 11 notes it.
- F22: P3's "4px grid becomes the spacing scale" sentence is in `A-principles-draft.md` (A changelog); the `CLAUDE.md` line is spec M's.
- R14: the store reasons of the 5.1 loop are Studio wording (`Studio scale: <reason>`), and token-contract.md (c) carries no Untitled role name in them.
- D–G review c: `PANEL_ACTION_BUTTON` 24 and `PAGES_SEARCH_DX` frozen, stated once (rule 6).
- Self-found 1: rule 1 said weight separates caption and body; false, because `font_caption()` and `font_body()` become the same `FontId` (12, Proportional). Now colour and position separate them; the decisions list says the same.
- Self-found 2: section 7's Home capture would also have read the owner's library; it now seeds a sample library (`--seed-library`, `--library`), and the editor captures open no library (`shell.rs:106`).
- Rename counts: 45 references outside `theme.rs` (42 in `crates/`, 3 in `scripts/`) plus 16 inside; verified 61 → 0 on a scratch copy of the full tree at `8dfaee93`.
- 5.4 ratchet re-pasted at `8dfaee93` with the two fixes (`cmdk.rs`, `dev/list.rs`): PASS, offset 1653, radius 252, 19 entries below baseline.
- Tools and anchors: Studio CLI and MCP runs use v0.7.0 (`tools/designos-latest`, `tools/designos-mcp-latest.sh`); code is cited at `origin/main` (`8dfaee93`).
