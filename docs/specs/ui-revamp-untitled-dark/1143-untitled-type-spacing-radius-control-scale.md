# Spec B — Untitled type, spacing, radius and control-size scale in `theme.rs` (R5)

| Field | Value |
|---|---|
| Issue | #1143 (epic #1118, spec key B) |
| Size | M. About a day and a half: ~40 `theme.rs` lines, 17 new constants, one mechanical rename across 42 references, 7 theme tests rewritten, 10 token re-values, two ratchet fixes, one ratchet category added, evidence. |
| Depends on | A (same files: `theme.rs`, `design/design.tokens.json`, `scripts/non_colour_token_parity.py`). One writer, B's PR opens after A merges. |
| Unblocks | C (widget anatomy uses these sizes), D–K (re-point call sites), L (height growth to allow) |
| Touches | `crates/designos-app/src/theme.rs` (type :116-139, `font_*` :141-184, radii :190-196, controls :255-260, derived sizes :290, :306, :311, :346, :362, :365, :394, :419, :583, :728, tests :770-913), 42 references renamed in `crates/` and `scripts/` (section 5.2), `crates/designos-app/src/cmdk.rs:446`, `crates/designos-app/src/dev/list.rs:60,67`, `design/design.tokens.json` (radius :341-366, typography :417-450), `scripts/non_colour_token_parity.py` (:23-58), `scripts/tests/test_non_colour_token_parity.py:81`, `scripts/rust_literal_scan.py` (:27-32 categories), `scripts/spacing_baseline.json` |

Code baseline: `ec47da26` (local `main`). Drift: `origin/main@35a65796` adds `TOAST_REVEAL_SECONDS` after `theme.rs:463` (+2 lines; every cited line after :463 moves by 2). Re-anchor every cited line against HEAD (and against A's merged diff) before writing code.

Value tables: `specs/token-contract.md` (c) and (d). This spec says why each value is what it is, how to apply it and how to prove it.

## 1. Summary and user story

As the owner, I want Studio's chrome to use Untitled's type, spacing, radius and control sizes (R2), at the small end of the scale in editor panels and at sm/md in Home and dialogs (R5), without moving any region, panel or control (R1).

As a surface implementer (C–K), I want one constant per Untitled step (`text-xs`, `radius-md`, button `xs` …) in `theme.rs`, invariant tests that keep them on the kit's scale, and a ratchet that tells me when a site I touch becomes non-concentric.

## 2. Current state (FACT unless labelled)

**Type.** `FONT_SIZE_CAPTION` 10 (:117), `FONT_SIZE_CANVAS` 9 (:119), `FONT_SIZE_BODY` 11 (:120), `FONT_SIZE_LARGE` 13 (:121), `FONT_SIZE_TITLE` 20 (:122), `LINE_BODY` 16 (:124), `LINE_CAPTION` 14 (:125), weights `UI_FONT_WEIGHT` 450 (:131), `UI_FONT_WEIGHT_STRONG` 550 (:132), `UI_FONT_WEIGHT_MEDIUM` 500 (:133). Panel titles use `font_body_strong` 11/550 (`panels/left.rs:193`, `panels/properties.rs:213`). Usage outside `theme.rs`: `font_body` 329, `font_caption` 228, `font_body_strong` 172, `font_caption_medium` 84, `font_caption_strong` 73, `font_mono` 69, `font_body_medium` 49.

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

**Radii and controls in code.** `RADIUS_MARK` 2 (:190), `RADIUS_SEGMENT` 4 (:192), `RADIUS_CONTROL` 6 (:194), `RADIUS_FRAME` 14 (:196); `CONTROL_DENSE` 24 (:255), `CONTROL_STANDARD` 28 (:256), `CONTROL_TOOL` 32 (:257), `CONTROL_ROW` 40 (:258), `TARGET_MIN` 24 (:260). References outside `theme.rs` (`grep -rnow <NAME> crates/designos-app/src --exclude=theme.rs | wc -l`): `CONTROL_DENSE` 55 in 13 files, `CONTROL_STANDARD` 47 in 15, `CONTROL_TOOL` 3, `CONTROL_ROW` 39; `RADIUS_CONTROL` 354, `RADIUS_FRAME` 114. `CONTROL_DENSE` sites include chips and pills (`agent_composer.rs:940-945`, `agent_selection_chip.rs:166`, `canvas/agent_overlay.rs:150`), buttons (`agent_open_flow.rs:397`, `agent_picker.rs:354`) and header strips (`terminal.rs:5217`).

**Spacing.** The ladder (:228-243) and insets (:246-252) already hold Untitled values: 4, 4, 8, 12, 16, 24, 48 and 2, 8, 8, 12, 16, 16, 24. The test `ladder_insets_and_controls_sit_on_the_4px_grid` (:815-819) forbids 2 and 6 except `INSET_SEGMENT`.

**Tests that pin today's scale.** `control_heights_are_the_ratified_family` (:843-848), `nested_radii_are_concentric` (:851-859), `menu_pills_inset_by_the_frame_inset` (:862-865), `type_roles_keep_their_order…` (:875-882, strictly increasing, `[10, 11, 13, 20]`), `line_heights_fit_their_roles` (:884-892, slot `CONTROL_DENSE`), `only_three_ui_weights_exist` (:895-897, `[450, 500, 550]`).

**Ratchet.** `scripts/check_spacing_relationships.py` + `scripts/spacing_baseline.json` (version 1, 81 files; HEAD live counts offset 1713, inset 45, radius 270, font 25, nonconcentric 2). `nonconcentric_pairs` resolves radii through the `theme.rs` constants (`rust_literal_scan.py:265-312`), so changing a radius constant can raise a file's count without touching it.

## 3. Behaviour rules

1. **Type roles (R5).** `FONT_SIZE_CAPTION` and `FONT_SIZE_BODY` become 12 (`text-xs`), `FONT_SIZE_LARGE` 14 (`text-sm`), `FONT_SIZE_TITLE` stays 20 (`text-xl`); `LINE_BODY` and `LINE_CAPTION` become 18. `FONT_SIZE_CANVAS` 9 stays (canvas exception; rulers and canvas pills only). Caption and body now share a size; weight tells them apart.
2. **Untitled steps (new names).** `FONT_SIZE_TEXT_XS` 12, `_SM` 14, `_MD` 16, `_LG` 18, `_XL` 20 and `LINE_TEXT_XS` 18, `_SM` 20, `_MD` 24, `_LG` 28, `_XL` 30, plus `pub fn font_text(size: f32, weight: TextWeight) -> FontId` and `pub enum TextWeight { Regular, Medium, Semibold }` (new names). Home and dialogs (specs H and J) move their text to `font_text(FONT_SIZE_TEXT_SM, …)`; until they do, Home text renders at 12 (interim, sequential phases).
3. **Weights.** `UI_FONT_WEIGHT` 400 (Regular), `UI_FONT_WEIGHT_MEDIUM` 500 (Medium), `UI_FONT_WEIGHT_STRONG` 600 (Semibold). Bold 700 is Display-only and has no chrome constant. The legacy role helpers (`font_caption*`, `font_body*`) keep their names; no mass rename.
3a. **Button labels (R8.6).** Button labels at every size, xs 32 included, are `text-sm` 14 Semibold 600 (`font_text(FONT_SIZE_TEXT_SM, TextWeight::Semibold)`), as the kit's `Buttons/Button` xs. This is for buttons only: row, label and tab text stays `text-xs` 12 (R5).
4. **Radii.** `RADIUS_CONTROL` 8 (`radius-md`, R5), `RADIUS_SEGMENT` 6 (`radius-sm`), `RADIUS_FRAME` 16 (`radius-2xl`, R8.5: menus, dropdowns and popovers, items 8 at inset 8), `RADIUS_XL` 12 (`radius-xl`, new name, G-4: cards and nested panels holding controls at inset 4), `RADIUS_MARK` 2 (`radius-xxs`, unchanged).
5. **Concentric law, with numbers.** `RADIUS_FRAME` 16 − `INSET_FRAME` 8 = `RADIUS_CONTROL` 8; `RADIUS_CONTROL` 8 − `INSET_SEGMENT` 2 = `RADIUS_SEGMENT` 6; `concentric(RADIUS_FRAME, MENU_PILL_INSET 8)` = 8 = `RADIUS_CONTROL`; `RADIUS_XL` 12 − 4 = 8 = `RADIUS_CONTROL` (a card with a 4 px inset); chain 16 > 12 > 8 > 6 > 2 strictly decreasing. A pill inside a pill: `radius_full(h_outer) − inset = radius_full(h_outer − 2·inset)`, e.g. a 32 track with inset 2 holds a 28 thumb: 16 − 2 = 14 = 28 ÷ 2.
6. **Controls (R5).** New `CONTROL_XS` 32 (button xs; editor buttons, rows, fields), `CONTROL_SM` 36 (button and input sm; Home, dialogs), `CONTROL_MD` 40 (button and input md; panel title rows), `BADGE_SM` 22, `BADGE_MD` 24, `BADGE_LG` 28 (new names). `TARGET_MIN` 24 stays the floor; `PANEL_ACTION_BUTTON` stays `TARGET_MIN` (two 32 px buttons at x 209 would overflow the 240 px panel; spec C/D decide icon-button anatomy).
7. **Renames.** `CONTROL_TOOL` → `CONTROL_XS` and `CONTROL_ROW` → `CONTROL_MD` (values kept; section 5.2). `CONTROL_DENSE` 24 and `CONTROL_STANDARD` 28 keep name and value as legacy: their sites mix buttons, chips and strips, so each surface spec re-points the sites it owns (button or row → `CONTROL_XS`, chip or pill → `BADGE_MD`/`BADGE_LG`, icon hit box → `TARGET_MIN`), and a ratchet stops new uses (rule 11).
8. **Re-pointed derived sizes.** `PAGE_ROW_HEIGHT`, `MENU_ITEM_HEIGHT`, `FIELD_HEIGHT` (R8.4), `FIND_FIELD_H`, `FIND_GROUP_ROW_H`, `STYLE_FOLDER_ROW_H` → `CONTROL_XS`; `SUBLABEL_HEIGHT` → `LINE_CAPTION`; `VAR_PILL_H` → `BADGE_MD`; `LABEL_BOX_H` and `CODE_LINE_HEIGHT` follow their line heights. `RIGHT_TABS_H` (:332) stays `CONTROL_DENSE` 24 (R10.1, owner: "A · Giữ như hiện tại"; R1 wins over R5 for tab heights): the Inspector segmented strip, file tabs (26 in the 32 bar) and session tabs (24) keep today's heights. Every other numeric constant keeps its value (296, token-contract.md appendix; evaluated, 0 implicit changes).
9. **Spacing.** Every ladder, inset and control value must be a step of the Untitled spacing scale {0, 2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 96, 128, 160}; the 2 px and 6 px steps are allowed (R5). Values do not change in B. Badge heights 22 and 28 and kit button paddings (10, 14, 18) are component anatomy sizes, not spacing steps (spec C).
10. **Ratchet baseline policy.** `scripts/spacing_baseline.json` is never raised. A rise caused by a re-valued theme constant is fixed in code in the same PR by deriving the inner radius from the law (`RADIUS_CONTROL` or `theme::concentric(outer, inset)`) or by moving the inset onto a theme constant. Gains may be locked with `--write-baseline`; the PR diff of the baseline shows decreases only.
11. **Legacy-name ratchet.** `scripts/rust_literal_scan.py` gains the category `legacy_control_refs` (new name, regex `\bCONTROL_(DENSE|STANDARD)\b`, code only), counted per file outside `theme.rs` into the same baseline; B writes its initial counts (102 references) once; afterwards they may only fall. When a constant reaches 0 references, the PR that removes the last one deletes the constant and its `RUST_ONLY` / `TOKEN_TO_RUST` row, except `CONTROL_DENSE`, which stays as the base of `RIGHT_TABS_H` inside `theme.rs` (R10.1).
12. **Layout unchanged (R1).** No width, x position, region or order changes. Heights grow only where rules 1, 6 and 8 raise a size (section 7).

## 4. Data model / schema changes

`theme.rs`: 17 constants added (new names, rules 2, 4 and 6), 2 renamed, 20 changed or re-pointed (token-contract.md (c); `RIGHT_TABS_H` held by R10.1); `RADIUS_*` stay `u8`. `design/design.tokens.json`: 10 re-values with `ui ds change-token` (token-contract.md (c) tsv: `font-size-caption` 12px, `font-size-body` 12px, `font-size-title` 14px, `font-size-heading` 14px, `radius-xs` 4px, `radius-sm` 6px, `radius-md` 8px, `radius-lg` 10px, `radius-xl` 12px, `radius-pill` 9999px); generation 69 → 79. Store keys unchanged (no add path, kernel gap T1 in spec A).

## 5. Contracts

### 5.1 Token re-values (scratch run, pasted)

```bash
while IFS=$'\t' read -r path value reason; do
  ui ds change-token "$path" --value "$value" --reason "Untitled UI v8: $reason" --dir . || exit 1
done < scale.tsv          # = the tsv block in token-contract.md (c)
```

Scratch result on the A-re-valued copy: 10/10 succeeded, generation 69 → 79, roles kept, `ui ds status` "tokens: 77 … components: 12".

### 5.2 Mechanical rename

```bash
grep -rlwE 'CONTROL_(TOOL|ROW)' crates scripts | xargs perl -pi -e 's/\bCONTROL_TOOL\b/CONTROL_XS/g; s/\bCONTROL_ROW\b/CONTROL_MD/g'
grep -rnwE 'CONTROL_(TOOL|ROW)' crates scripts      # prints nothing
```

Scratch copy of `crates/designos-app/src`: 42 references renamed outside `theme.rs`, 0 left. Shipped specs in `docs/specs/v0.6/` keep the old names (records).

### 5.3 `scripts/non_colour_token_parity.py`

`TOKEN_TO_RUST` and `RUST_ONLY` exactly as token-contract.md (d). Scratch harness (repo G01 functions + A and B edits, scratch theme and store): `G01 ok = True`, "35/35 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 30 non-colour constants match" (with `radius.radius-xl` → `RADIUS_XL`); unmapped (information only): `radius-lg`, `radius-pill`, `radius-xs`, `ai-panel-width`, `font-size-title`.

### 5.4 Ratchet on the scratch tree (pasted)

A scratch copy of `crates/designos-app/src` with B's theme and the 5.2 rename:

```text
$ python3 scripts/check_spacing_relationships.py --src <scratch>/src --baseline scripts/spacing_baseline.json --verbose
literal geometry outside theme.rs: offset_literals=1713, inset_literals=45, radius_literals=270, font_literals=25, nonconcentric_pairs=4
FAIL cmdk.rs: nonconcentric_pairs rose 0 -> 1 (lines 438)
FAIL dev/list.rs: nonconcentric_pairs rose 0 -> 1 (lines 67)
```

Causes (FACT): `cmdk.rs:438` row inset 8 inside `RADIUS_FRAME` (14 → 16) keeps a literal `CornerRadius::same(6)` (:446): 16 − 8 = 8 ≠ 6. `dev/list.rs:67` badge offset `r.left() + 7.0` inside a `RADIUS_CONTROL` tab (6 → 8): inset 7 was clear of the 6 arc and now sits inside the 8 arc. Fixes in B's PR: `cmdk.rs:446` → `theme::cr(theme::RADIUS_CONTROL)`; `dev/list.rs:60,67` `r.left() + 7.0` → `r.left() + theme::INSET_CONTROL` (8, on the arc's edge). After both:

```text
literal geometry outside theme.rs: offset_literals=1711, inset_literals=45, radius_literals=269, font_literals=25, nonconcentric_pairs=2
note: 10 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

## 6. File-format mapping

None. Document text keeps document sizes; only chrome changes.

## 7. UI

- UI implementers load the `es-designer` skill first, then follow `design/principles.md` (P3, P4, P5, P9, P10 as rewritten in `specs/A-principles-draft.md`), `design/soul.md` and this spec; the project's own design system wins over generic defaults.
- Height growth, layout unchanged (R1). INFERENCE from the code at `ec47da26`, assuming the left panel is 1056 px tall at the default 1600×1088 window (1088 − tab bar 32):
  - Left panel stack (`panels/left.rs:60-193`): header 65 + Pages title 40 + Pages list `min(n,5) × PAGE_ROW_HEIGHT + 8` + Layers header 40. With ≥ 5 pages: 65 + 40 + 128 + 40 = 273 before, 65 + 40 + 168 + 40 = 313 after; visible Layers rows (32 each) ⌊(1056 − 273)/32⌋ = 24 before, ⌊(1056 − 313)/32⌋ = 23 after. With 1 page: 27 rows before and after.
  - Inspector (`panels/properties.rs:207-297`): a labelled group is sublabel gap (12 − 4) + `LABEL_BOX_H` + 4 + `FIELD_HEIGHT` + 4 = 54 before, 66 after (+12); the first group after a title 46 → 58. A section with g groups grows 12g; Position (title 40 + 3 groups) 194 → 230.
  - Menus: `MENU_ITEM_HEIGHT` 24 → 32, a menu of n items grows 8n. Inspector tab strip stays 24 (`RIGHT_TABS_H` = `CONTROL_DENSE`, R10.1), so nothing below `RIGHT_TABS_Y` 48 moves.
  - Text widths: body 11 → 12 (+9%), caption 10 → 12 (+20%). Rail labels in the 56 px rail (e.g. "Variables") are UNKNOWN at 12 px; spec D measures them.
- Evidence to the PR, not Figma (outputs under `evidence/B/`):

```bash
cargo run -q --release -p designos-app --bin designos -- --home --screenshot evidence/B/home.png --size 1600x1088 --no-toast
cargo run -q --release -p designos-app --bin designos -- "$KIT_COPY" --screenshot evidence/B/editor-kit.png --size 1600x1088 --no-toast
for p in cmdk variables assets picker; do cargo run -q --release -p designos-app --bin designos -- "$KIT_COPY" --panel "$p" --screenshot "evidence/B/panel-$p.png" --size 1600x1088 --no-toast; done
```

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

Rewritten tests (red against HEAD values, green after):

1. `ladder_insets_and_controls_are_untitled_spacing_steps` (replaces `…sit_on_the_4px_grid`): every `LADDER`, `INSETS` and control value ∈ the Untitled spacing set (rule 9); `CONTROLS` = `[CONTROL_XS, CONTROL_SM, CONTROL_MD, TARGET_MIN]`.
2. `control_heights_are_the_untitled_button_sizes` (replaces `…ratified_family`): `[CONTROL_XS, CONTROL_SM, CONTROL_MD] == [32, 36, 40]`, `[BADGE_SM, BADGE_MD, BADGE_LG] == [22, 24, 28]`, `TARGET_MIN == 24`, `PAGE_ROW_HEIGHT == MENU_ITEM_HEIGHT == CONTROL_XS`; `RIGHT_TABS_H == CONTROL_DENSE == 24` stays as today (`theme.rs:847`, "inspector tab strips are 24px, not 28px"; R10.1).
3. `nested_radii_are_concentric`: unchanged asserts; add `[RADIUS_FRAME, RADIUS_XL, RADIUS_CONTROL, RADIUS_SEGMENT, RADIUS_MARK] == [16, 12, 8, 6, 2]`, `concentric(RADIUS_XL, 4.0) == RADIUS_CONTROL` and membership in the Untitled radius set {0, 2, 4, 6, 8, 10, 12, 16, 20, 24}.
4. `type_roles_are_untitled_text_styles` (replaces `type_roles_keep_their_order…`): non-decreasing chain, `[CAPTION, BODY, LARGE, TITLE] == [12, 12, 14, 20]`, `FONT_SIZE_CANVAS == 9` the only size below 12, `(FONT_SIZE_TEXT_*, LINE_TEXT_*)` == the five kit pairs.
5. `line_heights_fit_their_roles`: `LINE_CAPTION == LINE_BODY == 18`; slot `FIELD_HEIGHT` (32): `LABEL_BOX_H + SPACE_FACT <= FIELD_HEIGHT`.
6. `only_three_ui_weights_exist`: `[400, 500, 600]`.
7. `menu_pills_inset_by_the_frame_inset`, `pill_radius_is_half_the_height`, ladder/inset order tests: unchanged and green.
8. `scripts/tests/test_non_colour_token_parity.py:81`: `CONTROL_ROW` → `CONTROL_MD`; run `python3 -m unittest discover scripts/tests`.
9. Ratchet: `legacy_control_refs` fixture test (a temp file with one new `CONTROL_DENSE` fails); the two nonconcentric fixes of 5.4 (red without them).
10. G01 with B's `TOKEN_TO_RUST`/`RUST_ONLY`: green; with `radius.radius-xs` still mapped to `RADIUS_MARK` it fails (4 ≠ 2).

Acceptance criteria:

1. `theme.rs` scale constants equal token-contract.md (c); `grep -rnwE 'CONTROL_(TOOL|ROW)' crates scripts` prints nothing.
2. `cargo test -p designos-app` green with tests 1–7.
3. `python3 scripts/design_os_self_evolve.py` green (G01 "… 30 non-colour constants match" or more); `python3 scripts/check_spacing_relationships.py` PASS with the baseline diff showing decreases only plus the new `legacy_control_refs` entries.
4. `ui ds status --dir .` generation 79; `design.tokens.json` radius and typography equal token-contract.md (c).
5. `python3 scripts/zero_fake_ui_gate.py`, `ui ds soul check --dir .`, `ui agents check --dir .`, `design-os evolution` green.
6. Evidence PNGs of section 7 attached; L's layout gate (once it exists) shows no region, width or order change; "responsive: SKIPPED (owner default)" stated.

## 11. Performance budgets

Constants only. Taller rows draw fewer Layers rows per frame (24 → 23 at the default window), so no frame-time growth; idle CPU stays `0.00%`. Font atlas: one more weight instance is not added (400/500/600 replace 450/500/550).

## 12. Dependencies and build order

A merges first; B rebases on it and runs 5.1–5.4 on top (one writer, sequential PRs, same files). Then C (anatomy), then D–K re-point sites and retire `CONTROL_STANDARD` and every `CONTROL_DENSE` call site (`CONTROL_DENSE` itself stays as the base of `RIGHT_TABS_H`, R10.1). L's baseline capture must be taken at A+B before D–K start, so their diffs are measured against B's sizes.

## 13. Owner rulings that apply

R1 (layout unchanged; heights may grow only where a size rises), R2 (take type and spacing from Untitled), R5 (editor: text-xs 12/18 rows and labels, text-sm 14/20 panel titles, button xs 32, row 32, radius md 8; Home and dialogs: text-sm 14/20, button sm 36 / md 40; spacing scale incl. 2 and 6, ban lifted), R7 (spec-driven).

## Open questions for the owner

All decided by the owner on 2026-10-25 (plan.md R8); none open.

1. **OQ-4 Inspector field height: decided (R8.4).** 32 (`CONTROL_XS`); Position section 194 → 230. Owner: "A · 32px".
2. **OQ-5 Floating-chrome radius: decided (R8.5).** 16 (`radius-2xl`) for menus, dropdowns and popovers, items radius 8 at inset 8 (concentric). Owner: "A · 16, item 8, inset 8".
3. **Button xs label: decided (R8.6).** `text-sm` 14 Semibold for button labels only; rows, labels and tabs stay `text-xs` 12. Owner: "A · 14px semibold".

## Decisions on the open questions (designer, 2026-10-25)

- Caption and body both take `text-xs` 12 (R5 names one size for rows and labels); weight separates them.
- Only value-preserving renames (`CONTROL_TOOL`, `CONTROL_ROW`); `CONTROL_DENSE`/`CONTROL_STANDARD` are legacy with a falling reference ratchet, because a blanket rename would silently grow chips 24 → 32.
- No mass rename of `font_*` helpers; new Untitled steps sit beside them.
- `PANEL_ACTION_BUTTON` stays 24; icon-button anatomy is spec C's.
- Button labels follow R8.6 (`text-sm` 14 Semibold at every button size); B provides `FONT_SIZE_TEXT_SM` and `TextWeight::Semibold`, spec C applies them.
- `RADIUS_XL` 12 is added for the contract gap G-4 and mapped to `radius.radius-xl` (12 after the re-value), so that token is guarded instead of unmapped.
- Contrast is not B's acceptance: B changes no colour; specs cite contrast check C-1 in token-contract.md (f).
- Ratchet rises from re-valued radii are fixed in code, never by raising the baseline.
