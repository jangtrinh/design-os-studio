# K — Surface: Assets panel, Variables, Libraries and Plugins dialogs (Untitled UI dark)

| Field | Value |
|---|---|
| Issue | #1155 (epic #1118, spec key K) |
| Size | M (about 1.5 days: 6 files with 1–18 chrome colour sites, 4 files that re-point sizes only; no behaviour change) |
| Depends on | A, B, C and L |
| Unblocks | M (docs and screenshot refresh of Assets, Variables and Libraries) |
| Touches | `crates/designos-app/src/panels/assets.rs` (`show`, drawn from `panels/left.rs:44`; constants `:64-73`); `variables/view.rs` (`show` `:246`, sidebar `:441`, main `:641`, bottom bar `:747`, table `:805`, `hscrollbar` `:950`, `header_row` `:979`, `variable_row` `:1026`); `variables/popover.rs`; `variables/widgets.rs` (picker constants `:187-191`); `libraries/modal.rs` (`show` `:159`); `libraries/review.rs` (`show_banner` `:20`, `show` `:71`); `plugins/ui.rs` (`show` `:13`); `styles.rs` (model only, no visual change); tests: new `crates/designos-app/src/shell/tests/assets_variables_libraries_chrome.rs` (new name) |

**Code baseline.** Every `file:line` is at `ec47da26` (local `main`, 2026-10-25). `origin/main` (`205c72ef`) has moved some of these files (`git diff --stat ec47da26 origin/main -- <files>`): `variables/view.rs` 33 lines changed, `styles.rs` 9, `libraries/mod.rs` 5, `variables/widgets.rs` 3, `variables/popover.rs` 2. Re-anchor every line on the branch HEAD before editing (search for the quoted code) and re-run the census; report drift in the PR. Colour and size names are the token contract's (`specs/token-contract.md`); Untitled role names are given beside them for review.

**Scope note on "styles".** The brief names styles for K. The style UI lives in files spec F owns: the Styles section of the right panel (`panels/properties.rs:454-656`), the style picker (`panels/style_picker.rs`) and the style editors (`panels/style_edit.rs`, `panels/style_edit_grid_rows.rs`); F's spec lists them as "K lists them as not mine". K yields them to F and keeps `styles.rs` (the style model: 0 `theme::` lines, no painting). K ran the four style-editor captures once (section 10) and hands them to F as before-evidence.

## 1. Summary and user story

As a designer managing components, variables and libraries, I want the Assets panel, the Variables table, the Libraries and library-update dialogs and the Plugins dialog to use Untitled UI's dark style, so these panels stop showing Figma UI3 greys (`#2C2C2C`, `#383838`, `#1E1E1E`, `#252525`), a Figma-blue search outline and white labels on orange.

Regions, widths, columns and order stay (R1). Colours come from the token contract; anatomy from spec C (`Input/Search`, `Button/*`, `Toggle/Switch`, `Checkbox/Box`, `Tag/Chip`, `Badge/StatusCount`, `Dialog/Modal`, `List/Row`, `Menu/Item`); sizes from R5 (editor panels: text-xs rows, button xs 32, row 32; dialogs: text-sm 14/20, buttons sm 36 / md 40).

## 2. Current state

FACT unless labelled. Screenshots of today in section 10 (`K-01`..`K-06`).

| Surface | Code | Today |
|---|---|---|
| Assets panel | `panels/assets.rs` (left panel, `panels/left.rs:44`): header `SEARCH_H` 40 with title "Assets" in `Color32::WHITE` `:413-418`; search field `#1E1E1E` with a 1 px `#0D99FF` outline `:435-436`; view toggle `VIEW_BTN` 24; list rows `ROW_H` 48 with `THUMB` 40; group rows `GROUP_H` 28; grid preview 72 + label 24 (`:64-73`); variants badge 16 high on `CANVAS_COMPONENT` with `WHITE` icon and count `:369-376`; section headings `WHITE` `:496`, `:546`; library cards `#2C2C2C` / hover `#383838`, outline `#383838`, icon and meta `#8C8C8C` `:542-552`; "Add more libraries" button same greys with `WHITE` label `:573-576`; drag ghost label `WHITE` on `BG_MENU` `:750-759`; thumbnail tint `WHITE` `:299` | Figma UI3 greys |
| Variables view (right side / floating) | `variables/view.rs:246` `show` (called `shell.rs:1422`, `:1536`); bar `VAR_BAR_H` 48, header `VAR_HEAD_H` = `CONTROL_ROW` 40, rows `VAR_ROW_H` = `CONTROL_ROW` 40 at pitch 41, sidebar rows `VAR_SIDE_ROW` = `CONTROL_TOOL` 32, pills `VAR_PILL_H` = `CONTROL_DENSE` 24, value fields `VAR_FIELD_H` 24, scrollbar thumb `VAR_SCROLL_THUMB #707070` (`theme.rs:704-737`); group header row `#252525` `:984`, group label `#B3B3B3` `:988`; toggle knob `WHITE` `:1389`; `hscrollbar` `:950` (gate Z12 checks its drag support) | — |
| Variable popover and picker | `variables/popover.rs` (checkbox `:73`); `variables/widgets.rs` (`:20`, `:61`, `:117`, `:163`; picker W 248, max H 320, rows `PICK_ROW` 24, groups 26, head 40 `:187-191`; computed document colour `:11`) | theme constants |
| Libraries dialog | `libraries/modal.rs:159` `show`: scrim black α120 `:52`; header 48, PAD 20, library rows 60, change rows 20 (`:13-16`); brand button label `WHITE` `:81-96`; toggle knob `WHITE` `:108-127`; search box `:129-144`; change tags `:146-152`; thumbnail tint `:427` | dialog |
| Library update review | `libraries/review.rs`: banner `show_banner` `:20` (text `WHITE` `:37`); dialog `show` `:71` (PAD 20, header 48, footer 60, group 32, rows 32 `:13-17`); checkbox 14 on `BG_BRAND` with `WHITE` tick `:57-64` | dialog |
| Plugins dialog | `plugins/ui.rs:13` `show` (called `shell.rs:1365`, `:1548`; PAD = `INSET_MODAL` 24 `:11`); scrim black α140 `:40`; paragraphs `:202` | dialog |

**Chrome colour census** (spec D's command, unchanged across D–K):

```bash
cd /Users/jangtrinh/Products/design-os-studio
PYTHONDONTWRITEBYTECODE=1 python3 - panels/assets.rs variables/mod.rs variables/view.rs variables/popover.rs \
  variables/widgets.rs variables/library.rs libraries/mod.rs libraries/modal.rs libraries/review.rs styles.rs \
  plugins/mod.rs plugins/ui.rs plugins/store.rs plugins/runtime.rs plugins/svg.rs <<'PY'
import re, sys
sys.path.insert(0, "scripts")
from rust_literal_scan import clean
HEX = re.compile(r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,"
                 r"|Color32::from_gray\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*\)|Color32::from_(?:black|white)_alpha\("
                 r"|Color32::(?:WHITE|BLACK|GRAY|LIGHT_GRAY|DARK_GRAY|RED|GREEN|BLUE|YELLOW)\b")
CONV = re.compile(r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\((?!\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,)")
for rel in sys.argv[1:]:
    src = open("crates/designos-app/src/" + rel, encoding="utf-8").read()
    code = clean(src).split("\n")
    hexs = [i + 1 for i, l in enumerate(code) for _ in HEX.finditer(l)]
    conv = [i + 1 for i, l in enumerate(code) for _ in CONV.finditer(l)]
    print(rel, "theme_lines=%d" % sum("theme::" in l for l in code), "chrome_sites=%d" % len(hexs), sorted(set(hexs)), "computed=%s" % sorted(set(conv)))
PY
```

Result at `ec47da26` (run 2026-10-25, exit 0):

| File | `theme::` lines | Chrome sites | Lines | Computed |
|---|---|---|---|---|
| `panels/assets.rs` | 63 | 18 | `:299`, `:375`, `:376`, `:418`, `:435`, `:436`, `:496`, `:542` (×2), `:544`, `:545`, `:546`, `:552`, `:573` (×2), `:575`, `:576`, `:759` | — |
| `variables/view.rs` | 177 | 3 | `:984`, `:988`, `:1389` | — |
| `variables/widgets.rs` | 29 | 0 | — | `:11` (document colour) |
| `variables/popover.rs`, `variables/library.rs`, `variables/mod.rs` | 18, 3, 0 | 0 | — | — |
| `libraries/modal.rs` | 52 | 4 | `:52`, `:96`, `:125`, `:427` | — |
| `libraries/review.rs` | 27 | 2 | `:37`, `:64` | — |
| `plugins/ui.rs` | 32 | 1 | `:40` | — |
| `libraries/mod.rs`, `styles.rs`, `plugins/mod.rs`, `store.rs`, `runtime.rs`, `svg.rs` | 0–3 | 0 | — | — |

Total today: **28** chrome sites in K's files. (plan.md's `grep -c 'Color32::from_rgb('` gives assets 10, variables/view 2; the census above is the contract.) Legacy size names in K's files: `CONTROL_STANDARD` ×2 (`panels/assets.rs`), `CONTROL_DENSE` ×2 and `CONTROL_TOOL` ×1 (`variables/view.rs`) (`grep -noE 'CONTROL_(DENSE|STANDARD|ROW|TOOL)' <K files>`).

**Gates today.** No gate counts colour literals; `.github/workflows/design-gates.yml` is advisory (`continue-on-error`, `:4-5`, `:21`). `scripts/zero_fake_ui_gate.py` Z12 checks that `variables::view::hscrollbar` supports dragging (`scripts/zero_fake_ui_gate.py:61`, `:779`). `scripts/check_spacing_relationships.py` passes at `ec47da26` (exit 0).

## 3. Behaviour rules

1. **Layout unchanged (R1).** The Assets panel keeps its header 40, list rows 48, grid 72 + 24; Variables keeps its bar 48, header 40, rows 40 at pitch 41, columns (`VAR_NAME_COL` 200, modes 140–280, add column 39), sidebar 240, floating window 642×380; dialogs keep widths and headers. Heights change only where R5 raises a control or row (table 3). Rows taller than R5's 32 stay as they are (Open question 1).
2. **Assets panel.** Title `TEXT` at `FONT_SIZE_LARGE` 14; search is C's `Input/Search` xs (32 inside the 40 header; focus shows `FOCUS_RING`, never the Figma-blue outline); view toggle is C's `Button/Icon` xs 32 (selected = `BG_SELECTED`); group rows 32 with `TEXT_TERTIARY` caption and `ICON_QUATERNARY` chevron; library cards `BG_SECONDARY` + `BORDER`, hover `BG_SECONDARY_HOVER`, icon `ICON_TERTIARY`, meta `TEXT_TERTIARY`; "Add more libraries" is `Button/Secondary` xs; the variants badge on a thumbnail is `Badge/StatusCount` sm (`BADGE_SM` 22) keeping its `CANVAS_COMPONENT` fill (component identity, contract (e)) with a `TEXT_ON_LIGHT_CANVAS` dark label (white on cyan fails AA); the drag ghost is a `Tag/Chip` (`BG_MENU`, `TEXT`).
3. **Variables table.** Untitled `Table` look within today's geometry: header row and group rows `BG_SECONDARY`, header text `TEXT_TERTIARY` caption, group label `TEXT_TERTIARY` (was `#B3B3B3`), row hover `BG_HOVER`, selected `BG_SELECTED`, row dividers `BORDER`; pills are `Badge` md (`VAR_PILL_H` → `BADGE_MD` 24 by the contract); value fields use `FIELD_HEIGHT` (contract: `CONTROL_XS` 32) instead of `VAR_FIELD_H` 24, inside the 40 row; toggles are C's `Toggle/Switch` (knob colour owned by C); scrollbar thumb `VAR_SCROLL_THUMB` (contract `#737373`); horizontal scrollbar drag behaviour unchanged (Z12).
4. **Variable popover and picker.** C's `Popover/Panel` and `Menu/Item`: picker rows `MENU_ITEM_HEIGHT` 32 (K's call sites use it instead of `PICK_ROW` 24), group rows 26 → 32, head 40 unchanged, width 248 unchanged; the popover checkbox (`variables/popover.rs:73`) is C's `Checkbox/Box`.
5. **Libraries dialog** is C's `Dialog/Modal`: padding `INSET_MODAL` 24 (was 20), header 48 stays, library rows 60 stay (two-line rows), search `Input/Search` sm 36, enable switches C's `Toggle/Switch`, publish and update actions `Button/Primary|Secondary` sm 36 with `TEXT_ON_BRAND` on brand, change tags C's `Tag/Chip` (height `BADGE_MD` 24, was 20); scrim from C's `modal_surface` (contract gap G-3).
6. **Library update review**: banner text `TEXT` on its banner surface (`BG_SECONDARY` + `BORDER`), banner buttons `Button/*` xs 32; dialog padding 24, header 48 and footer 60 stay, group and item rows become C's `List/Row` sm 36 (dialog rows, R5), checkbox C's `Checkbox/Box` (checked `BG_BRAND` with a `TEXT_ON_BRAND` tick).
7. **Plugins dialog**: C's `Dialog/Modal` (padding already 24), paragraphs `TEXT_TERTIARY` body text-sm, actions `Button/*` sm 36, scrim from `modal_surface`.
8. **Content stays.** Thumbnail texture tints (`panels/assets.rs:299`, `libraries/modal.rs:427`), variable colour swatches and the computed document colour (`variables/widgets.rs:11`) are content.
9. **Focus.** Every focusable control shows C's focus ring (`FOCUS_RING`) on keyboard focus; variable cells in edit mode show it around the field.
10. **No behaviour change.** Search, filtering, drag-to-canvas, variable editing, mode columns, library enable/publish/update, plugin install and run are untouched; every `widgets::record` key keeps its name (e.g. `lib-check:{key}`, `libraries/review.rs:59`).

## 4. Data model / schema changes

None. `styles.rs` (style model) is not changed.

## 5. Contracts

- **Colour constants consumed** (token contract (a)): `BG`, `BG_SECONDARY`, `BG_SECONDARY_HOVER`, `BG_TERTIARY`, `BG_HOVER`, `BG_PRESSED`, `BG_MENU`, `BG_MENU_HOVER`, `BG_SELECTED`, `BG_BRAND`, `BG_BRAND_HOVER`, `BG_BRAND_PRESSED`, `BORDER`, `BORDER_STRONG`, `BORDER_MENU`, `BORDER_TRANSLUCENT` (swatch and thumbnail outlines), `TEXT`, `TEXT_SECONDARY`, `TEXT_SECONDARY_HOVER`, `TEXT_TERTIARY`, `TEXT_PLACEHOLDER`, `TEXT_ON_BRAND`, `TEXT_ON_LIGHT_CANVAS`, `TEXT_ERROR`, `TEXT_SUCCESS`, `TEXT_WARNING`, `ICON_SECONDARY`, `ICON_TERTIARY`, `ICON_QUATERNARY`, `CANVAS_COMPONENT`, `VAR_SCROLL_THUMB`, `FOCUS_RING`.
- **Contract gaps** (shared with I and J; requested from spec A): G-3 modal scrim (today α120 / α140 in K's files; C's `modal_surface` owns it). Change tags (`libraries/modal.rs:146-152`: added / changed / removed) need the status badge roles of gap G-1 (spec I) if the owner picks tinted badges; with the contract alone they render as gray tags with `TEXT_SUCCESS` / `TEXT_WARNING` / `TEXT_ERROR` labels.
- **Size constants consumed** (contract (c)): `CONTROL_XS` 32 (was `CONTROL_TOOL`, mechanical rename by B), `CONTROL_SM` 36, `CONTROL_MD` 40 (was `CONTROL_ROW`; `VAR_HEAD_H` and `VAR_ROW_H` keep 40 through the rename), `BADGE_SM` 22, `BADGE_MD` 24, `MENU_ITEM_HEIGHT` 32, `FIELD_HEIGHT` 32, `INSET_MODAL` 24, `RADIUS_CONTROL` 8, `RADIUS_FRAME` 16, `FONT_SIZE_*` / `LINE_*` per B. Theme constants K stops reading (values stay per the contract appendix; A/B may delete them when no reader is left): `VAR_FIELD_H` 24. Local constants K changes in its own files: `GROUP_H` 28 → `CONTROL_XS`, `VIEW_BTN` 24 → `CONTROL_XS` (`panels/assets.rs:65`, `:73`); `PICK_ROW` 24 → `MENU_ITEM_HEIGHT`, `PICK_GROUP` 26 → `CONTROL_XS` (`variables/widgets.rs:189-190`); `PAD` 20 → `theme::INSET_MODAL`, `CHANGE_ROW_H` 20 → `BADGE_MD` (`libraries/modal.rs:14`, `:16`); `PAD` 20 → `INSET_MODAL`, `GROUP_H` / `ROW_H` 32 → `CONTROL_SM` (`libraries/review.rs:13`, `:16-17`). `CONTROL_DENSE` / `CONTROL_STANDARD` sites in K's files: re-pointed to 0.
- **Widgets consumed** (spec C, `widgets/kit/`, new names there): `Input/Search`, `Input/Field`, `Button/Primary|Secondary|Icon|Close`, `Toggle/Switch`, `Checkbox/Box`, `Tag/Chip`, `Badge/StatusCount`, `List/Row`, `Menu/Item`, `Popover/Panel`, `Dialog/Modal` (with `modal_surface`). C names these K duplicates to retire: `libraries/modal.rs:43-79` (modal frame), `:81-101` (buttons), `:108-127` (toggle), `:129-144` (search), `:146-152` (change tag) (C section 3.3 and 5.1).
- **Rust surface.** No public signature changes (`assets::show`, `variables::view::show`, `libraries::modal::show`, `libraries::review::{show_banner, show}`, `plugins::ui::show`).

## 6. File-format mapping

None.

## 7. UI

UI implementers load the `es-designer` skill first and complete its checklist; the project's design system and the token contract win over the skill's defaults. Evidence goes in the PR (not Figma, R3). Responsive: SKIPPED (owner default); capture at 1600×1088.

**Table 1: colours** (contract constant first; hex is the contract's dark value):

| Element | Today | Contract constant (Untitled role) | Hex |
|---|---|---|---|
| Assets title, section headings | `Color32::WHITE` | `TEXT` | `#FAFAFA` |
| Assets search field | `#1E1E1E` + `#0D99FF` 1 px | `BG` + `BORDER_STRONG`, placeholder `TEXT_PLACEHOLDER`, focus `FOCUS_RING` | `#0A0A0A` + `#404040`, focus `#F97316` |
| Library card / hover / outline | `#2C2C2C` / `#383838` / `#383838` | `BG_SECONDARY` / `BG_SECONDARY_HOVER` / `BORDER` | `#171717` / `#262626` / `#262626` |
| Card icon, meta text | `#8C8C8C` | `ICON_TERTIARY`, `TEXT_TERTIARY` | `#A3A3A3` |
| "Add more libraries" | `#2C2C2C` / `#383838` + `WHITE` | `Button/Secondary`: `BG` + `BORDER_STRONG`, label `TEXT_SECONDARY`; hover `BG_HOVER`, `TEXT_SECONDARY_HOVER` | `#0A0A0A` + `#404040`, `#D4D4D4`; `#171717`, `#E5E5E5` |
| Variants badge | `CANVAS_COMPONENT` + `WHITE` | `CANVAS_COMPONENT` (kept) + `TEXT_ON_LIGHT_CANVAS` | cyan + `#0A0A0A` |
| Drag ghost | `BG_MENU` + `WHITE` | `BG_MENU` + `BORDER_MENU`, label `TEXT` | `#0A0A0A` + `#262626`, `#FAFAFA` |
| Variables header and group rows | theme / `#252525` | `BG_SECONDARY` | `#171717` |
| Variables group label | `#B3B3B3` | `TEXT_TERTIARY` | `#A3A3A3` |
| Variables row hover / selected / divider | theme | `BG_HOVER` / `BG_SELECTED` / `BORDER` | `#171717` / `#262626` / `#262626` |
| Scrollbar thumb | `VAR_SCROLL_THUMB #707070` | `VAR_SCROLL_THUMB` | `#737373` |
| Toggle knob | `Color32::WHITE` | C's `Toggle/Switch` (knob colour in C) | per C |
| Dialog surfaces (libraries, review, plugins) | theme / α scrims | `BG_MENU` + `BORDER_MENU`; scrim from `modal_surface` (G-3) | `#0A0A0A` + `#262626` |
| Brand button label | `WHITE` | `TEXT_ON_BRAND` | `#0A0A0A` |
| Review checkbox | `BG_BRAND` + `WHITE` tick | `Checkbox/Box`: `BG_BRAND` + `TEXT_ON_BRAND` tick; unchecked `BG` + `BORDER_STRONG` | `#E36323` + `#0A0A0A` |
| Update banner text | `WHITE` | `TEXT` | `#FAFAFA` |
| Change tags (added / changed / removed) | theme colours | `Tag/Chip` + `TEXT_SUCCESS` / `TEXT_WARNING` / `TEXT_ERROR` (G-1 fills if accepted) | `#4ADE80` / `#FACC15` / `#F87171` |
| Thumbnail tints, swatches, document colours | `WHITE` tint, computed | unchanged (content, allowlist) | — |

**Table 2: type** (B's constants): panel rows, buttons, table cells `FONT_SIZE_BODY` 12 / `LINE_BODY` 18; captions, group labels, table header `FONT_SIZE_CAPTION` 12 / 18; panel title "Assets" and dialog section titles `FONT_SIZE_LARGE` 14; dialog titles text-md 16 semibold, dialog body text-sm 14 / 20; weights 400 / 500 / 600. Hex values and numbers in the Variables table stay in `UI_MONO_FAMILY`.

**Table 3: sizes** ("Δ" at 1600×1088):

| Element | Today | After | Δ |
|---|---|---|---|
| Assets header / search | 40 / field drawn in it | 40 / `Input/Search` xs 32 | 0 |
| Assets view toggle | 24 | `CONTROL_XS` 32 | +8 (inside the 40 header) |
| Assets group rows | 28 | 32 | +4 per group |
| Assets list rows, grid cell | 48, 72 + 24 | same | 0 |
| Variants badge | 16 | `BADGE_SM` 22 | +6 (on the 72 preview) |
| Variables bar / header / rows / pitch | 48 / 40 / 40 / 41 | same (Open question 1) | 0 |
| Variables sidebar rows | 32 (`CONTROL_TOOL`) | 32 (`CONTROL_XS`) | 0 |
| Variables pills | 24 | `BADGE_MD` 24 | 0 |
| Variables value fields | 24 | `FIELD_HEIGHT` 32 | +8 (inside the 40 row) |
| Variable picker rows / groups | 24 / 26 | 32 / 32 | +8 / +6 per row |
| Libraries dialog padding / rows / change tags | 20 / 60 / 20 | 24 / 60 / 24 | +8 total height, 0, +4 per tag row |
| Libraries search, buttons | theme sizes | `CONTROL_SM` 36 | measured in the PR |
| Review rows (group, item) | 32 | `CONTROL_SM` 36 | +4 per row |
| Review and plugins padding | 20 / 24 | 24 / 24 | +8 / 0 |
| Radii | 6 / 14 | `RADIUS_CONTROL` 8 / `RADIUS_FRAME` 16 | — |

**States**: default, hover, pressed (`BG_PRESSED`), focused (`FOCUS_RING`), disabled (50 % opacity, C rule S5), selected (`BG_SELECTED`). Empty states: "No assets" and "No variables" use C's `EmptyState/Default` text roles (`TEXT`, `TEXT_TERTIARY`), no new illustration.

## 8. Edge cases

1. **Many libraries** (dialog scroll): rows stay 60; padding 24 leaves 8 px less width for row text; names elide as today.
2. **Variables with many modes** (horizontal scroll): column widths unchanged; the 32 value field fits in a 40 row with 4 px above and below; Z12's drag support must stay.
3. **Variables floating window** (642×380, `K-03`): the header 40 and rows 40 are unchanged, so the same number of rows is visible.
4. **Long asset names** in grid mode: label row stays 24 with text-xs 18 line; names elide as today.
5. **Variants badge on a light thumbnail**: `CANVAS_COMPONENT` fill with dark label stays readable on both light and dark thumbnails (the badge carries its own fill).
6. **Assets search focus**: the old blue outline was always drawn; after the change the outline is `BORDER_STRONG` and the orange ring appears only on keyboard focus (C rule), which is a visible change to call out in the PR.
7. **Plugin install dialog** (`--panel plugin-install`, `K-06`) and the plugin list (`K-05`) share `plugins/ui.rs`; both follow rule 7.
8. **Library update banner** appears only when updates exist; capture it from a seeded library with an update, or say it was not captured.

## 9. Non-goals

- Style UI (Styles section, style picker, style editors): spec F.
- Canvas component colours (`CANVAS_COMPONENT`), variable colour swatches, document colours, thumbnails.
- Variables table geometry (row heights, columns), Assets list geometry, any behaviour change.
- `panels/check_designs.rs` (unassigned; recommended owner K in a follow-up issue, see section 12).
- Light mode, icon set (R6), Figma (R3).

## 10. Test plan (red first)

New file `crates/designos-app/src/shell/tests/assets_variables_libraries_chrome.rs` (new name), registered with one `mod` line in `crates/designos-app/src/shell/tests.rs` (`:1017-1048`); it uses the shared `chrome_colour_sites` helper from `shell/tests/chrome_census.rs` (new name, spec H section 10) and the recorded-rect hook (`widgets::record`, `widgets/mod.rs:245`) through `Sim::rect_of` (`shell/tests.rs:159`).

| Test | Asserts | Red at `ec47da26` |
|---|---|---|
| `asset_variable_library_sources_paint_no_chrome_colour_literals` | census over K's 15 files returns only the allowlist (thumbnail tints `panels/assets.rs:299`, `libraries/modal.rs:427`, matched by code text) | yes (28) |
| `assets_search_has_no_blue_outline` | the Assets search field's look is C's `field_look(State::Default, false)` (border `BORDER_STRONG`); no `0x0D, 0x99, 0xFF` in `panels/assets.rs` | yes |
| `variables_rows_keep_geometry` | `VAR_HEAD_H == 40`, `VAR_ROW_H == 40`, `VAR_ROW_PITCH == 41` (guard, R1); recorded value-field rects are `FIELD_HEIGHT` high | yes (fields 24) |
| `variable_picker_rows_are_menu_rows` | picker rows `MENU_ITEM_HEIGHT` (32) | yes (24) |
| `libraries_dialog_follows_dialog_sizes` | dialog padding 24; search and buttons `CONTROL_SM`; change tags `BADGE_MD` | yes (20 / … / 20) |
| `review_checkbox_uses_dark_tick` | `kit::choice` checkbox look checked = `BG_BRAND` fill and `TEXT_ON_BRAND` tick; review rows `CONTROL_SM` | yes |
| `variables_hscrollbar_still_drags` | existing Z12 behaviour: dragging the thumb scrolls the table | no (guard) |

Keep green: `shell/tests/variables.rs`, `shell/tests/libraries.rs`, `shell/tests/plugins.rs`, `shell/tests/assets_thumbs.rs`, `shell/tests/components.rs`.

Commands (repo root): `cargo test -p designos-app --bin designos assets_variables_libraries_chrome`; `cargo test -p designos-app --bin designos variables libraries plugins assets`; `cargo test --workspace --lib`; `python3 scripts/zero_fake_ui_gate.py` (Z12 OK); `python3 scripts/check_spacing_relationships.py` (exit 0); `python3 scripts/design_os_self_evolve.py`.

**Screenshot evidence** (before = at `ec47da26`, run 2026-10-25; after = same commands on the PR branch; both in the PR). Setup as spec I: `--seed-library --library "$LIB"` (exit 0), `FILE=$(ls "$LIB"/projects/*/files/*.designos | sed -n 3p)` (the "Design system" file; in this session it resolved to `…/files/18e1b21f9d7260280003.designos`, the seed creates one project with 4 files); each run `cargo run -q --release -p designos-app --bin designos -- "$FILE" <args> --screenshot <out>.png --size 1600x1088 --no-toast`.

| Id | `<args>` | Exit code (this session) |
|---|---|---|
| K-01 | `--panel assets` | 0 |
| K-02 | `--panel variables` | 0 |
| K-03 | `--panel variables-floating` | 0 |
| K-04 | `--panel libraries` | 0 |
| K-05 | `--panel plugins` | 1 ("screenshot was not captured"), retry 0 |
| K-06 | `--panel plugin-install` | 0 |
| (F) | `--panel edit-style-color`, `edit-style-text`, `edit-style-effect`, `edit-style-grid` (F's surfaces, handed to F) | 0, 0, 0, 0 |

The exit-1 run happened while other agents were building in the same checkout; the retry passed. Not captured from the CLI: the library update review dialog and its banner, the variable picker popover, the variable popover; capture them from a live session and say so in the PR.

**Acceptance criteria**

1. The census prints `chrome_sites=0` for every K file except `panels/assets.rs` and `libraries/modal.rs`, which print only their one allowlisted thumbnail tint each.
2. The tests above pass and the listed existing tests stay green.
3. `grep -rnwE 'CONTROL_(DENSE|STANDARD|TOOL|ROW)|VAR_FIELD_H' <K files>` prints nothing (B's mechanical rename handles `CONTROL_TOOL` / `CONTROL_ROW`; K re-points the rest).
4. `K-01`..`K-06` after-shots show contract colours, no Figma-blue outline, no white label on orange, and unchanged Assets and Variables geometry (L's layout gate passes for these captures).
5. Z12 stays OK; `check_spacing_relationships.py` exits 0.
6. The PR lists re-anchored lines, the before/after census and which surfaces were captured by hand.

## 11. Performance budgets

The Variables table paints up to hundreds of rows per frame: the change replaces literals with constants and adds no allocation; per-row `Look` lookups are pure. Frame time with the floating Variables window open on 500 variables must not rise by more than 0.5 ms. Idle CPU unchanged (0.00 %).

## 12. Dependencies, ownership and build order

**Build order.** L captures baselines (`K-01`..`K-06`) → A (contract constants; G-1, G-3 if accepted) → B (sizes; `CONTROL_TOOL` → `CONTROL_XS`, `CONTROL_ROW` → `CONTROL_MD` renames, `VAR_PILL_H` → `BADGE_MD`, `VAR_SCROLL_THUMB` revalue) → C (`widgets/kit/`, including `Toggle/Switch`, `Checkbox/Box`, `modal_surface`) → this spec.

**Technical decisions recorded here** (the owner may reverse): (1) Variables header and rows stay 40 and Assets list rows 48 (R1; R5's 32 would shrink them); (2) the variants badge keeps the component cyan with a dark label (identity colour, AA); (3) K re-points `VAR_FIELD_H` call sites instead of asking B to change its value, so the contract stays as written; (4) K yields the style UI files to F (scope note).

**File ownership across H–K** (same table in H, I, J and K):

| File | Owner |
|---|---|
| `home/mod.rs`, `home/thumb.rs`, `home/time.rs`, `home/session.rs`, `home/leave.rs` and `home/unsaved.rs` (new on `origin/main`, no painting), `home/sidebar.rs` and `home/main_view.rs` (deleted), `shell/tests/home.rs` | H |
| `terminal.rs`, `agent_*.rs`, `canvas/agent_banner.rs`, `canvas/agent_changes_chip.rs`, `canvas/agent_overlay.rs`, `skills/*`, `image_jobs.rs`, `shell/tests/agent_chrome.rs` (new) | I |
| `cmdk.rs`, `updater.rs`, `present.rs`, `panels/context.rs`, `home/dialog.rs`, `history/*`, `find/mod.rs`, `find/panel.rs`, `find/select.rs`, `comments/*`, `shell/tests/overlays_chrome.rs` (new) | J |
| `panels/assets.rs`, `variables/*`, `libraries/*`, `styles.rs`, `plugins/*`, `shell/tests/assets_variables_libraries_chrome.rs` (new) | K |
| `shell/tests/chrome_census.rs` (new; the `chrome_colour_sites` helper of spec H section 10; whichever of H–K lands first adds it exactly as H defines it) | H |

Not mine: `panels/properties.rs` (Styles section `:454-656`), `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs`, `widgets/picker.rs`, `panels/right.rs`, `panels/inspect.rs` (F); `panels/left.rs` (E: hosts the Assets panel at `:44`), `panels/layers.rs`, `find/filter.rs` (E); `tabs.rs`, `shell.rs` (calls the Variables view `:1422`, `:1536` and the Plugins dialog `:1365`, `:1548`; no edit needed), `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); `dev/*`, `specs.rs` (G); `theme.rs` (including `VAR_*` constants), `design/*`, `scripts/*` (A, B); `widgets/*`, `widgets/kit/*` (C). `shell/tests.rs` is L's (`Sim`, `:53-125`; `Sim::rect_of` `:159`): each H–K PR only appends one `mod` line to its module list (`:1017-1048`). `scripts/spacing_baseline.json` is a shared ratchet: each surface PR may only lower its own files' counts with `--write-baseline`.

Unassigned (flag to the controller): `panels/check_designs.rs` [recommended K, as a follow-up issue: it paints a scan progress bar and empty state that C lists as duplicates (`:330-337`, `:511`, `:518-528`) and reads I's terminal palette `:7`, `:81`, `:83`, `:84`, `:511`], `lint.rs` [K], `canvas/draw.rs` [D or L], `decks/ui.rs` [D], `board/*` [D], `icons.rs` [C].

**Conflict with the brief, recorded for the controller.** The brief names styles for K; spec F claims the style UI files and K yields (scope note). If the controller wants the style editors in K instead, move `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` from F's table to this one; nothing else in K changes.

**Open PRs touching K files** (`gh pr list --state open`, 2026-10-25; file lists from `gh api repos/{owner}/{repo}/pulls/<n>/files`): #1112 (`styles.rs`, plus F's `panels/style_edit.rs`, `panels/style_picker.rs`); stale #765 (`libraries/mod.rs`, `styles.rs`, `variables/library.rs`, `variables/mod.rs`, `variables/view.rs`), #768 and #770 (`libraries/mod.rs`), all `mergeable: CONFLICTING`. Land or rebase #1112 first; re-run the census after each rebase.

## 13. Owner rulings that apply

- R1: "sử dụng 100% style kiểu của Untitled UI nhưng vẫn giữ cấu trúc layout và bố cục hiện tại" (all of Untitled's style; keep layout and arrangement).
- R2: take Untitled spacing and typography as they are, adapted to Studio's layout.
- R3: Studio improves itself; no Figma. Evidence goes in the PR.
- R4: brand = Untitled anatomy with Studio orange `#E36323`, dark label `#0A0A0A`; focus ring `#F97316`; Purple Ban stays.
- R5: editor panels text-xs 12/18, button xs 32, row 32, radius md 8; dialogs text-sm 14/20, buttons sm 36 / md 40.
- R6: keep the icon set; restyle colour to Untitled fg roles.
- R7: spec-driven; the PM coordinates delivery.
- Controller defaults: canvas and component identity colours keep their values; document content unchanged.
- Standing: `main` is protected (PR, green macOS test, Clippy, Rustfmt, squash); visual evidence in every PR (2026-09-22).

## Open questions for the owner

1. **Variables rows and header (40) and Assets list rows (48).** Recommended: keep them (R1: R5 raises sizes, it does not shrink rows; Untitled's own table rows are taller than 32). Alternative: shrink Variables rows to R5's row 32 so they match Layers; the floating window's 259 px of rows (380 − bar 40 − header 40 − bottom 41) then shows 7 rows instead of 6, but the table's geometry changes.
2. **Assets search outline.** Recommended: Untitled's neutral field border with the orange focus ring only on keyboard focus (rule 2). Alternative: keep an always-visible accent outline, drawn in `BORDER_SELECTED` orange instead of Figma blue.
