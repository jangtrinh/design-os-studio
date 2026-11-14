---
id: detail-1124-surface-right-panel-inspector-desig
type: note
title: "F Right panel: inspector header, Design and Prototype tabs, Dev Inspect panel, colour picker and inspector popovers in Untitled UI dark"
updated: "2026-10-25"
status: draft
---

## Nguồn
Bản sao từ nhánh `docs/specs-untitled-ui-dark` (`docs/specs/ui-revamp-untitled-dark/1124-surface-right-panel-inspector-design-prototype.md`), lấy ngày 25/10/2026 để pack tự đủ. Nguồn chuẩn chi tiết cho US tương ứng.


| Field | Value |
|---|---|
| Issue | #1124 (epic #1118, spec key F) |
| Size | L. Four to five days: 17 owned files, 84 literal colour sites (60 chrome choices, 16 canvas overlays that move to theme constants, 7 placeholders, 1 allowlisted image tint), every inspector field and section on C's anatomy, R5/R8.4 field and label sizes in a 240 px panel. Splittable into F1 (header, tabs, footer, `properties.rs` sections) and F2 (`inspect.rs`, `prototype.rs`, picker and popovers) if the PM wants two PRs. |
| Depends on | A (colour constants, `specs/token-contract.md`, contrast check C-1, colour-literal gate), B (type, spacing, radius and control-size scale), C (widget anatomy and states, `specs/C-widget-kit-untitled-anatomy.md`) and L (layout-unchanged gate and baselines). G for the Dev accent role (its value `DEV_READY` `#14AE5C` is R10.4) and for the Dev sections drawn inside the Inspect panel. |
| Unblocks | M (docs and screenshot refresh). |
| Touches | `crates/designos-app/src/panels/right.rs` (`:11-181` header, tabs, footer; `:183-315` Dev plugins; `:317-381` frame presets); `panels/properties.rs` (`:177-353` pane, titles, sublabels, segmented, checkbox; `:389-710` page, styles and selection-colour sections; `:711-1427` selection, dimensions, resizing, auto layout, grid; `:1438-1680` paints; `:1819-2258` picker popup and gradient handles); `panels/inspect.rs` (`:17-21`, `:313-823`); `panels/prototype.rs`; `panels/proto_actions.rs`; `panels/components.rs`; `panels/export.rs`; `panels/stroke.rs`; `panels/effects.rs`; `panels/typography.rs`; `panels/layoutguide.rs`; `panels/vertex.rs`; `panels/var_bind.rs`; `panels/style_picker.rs`; `panels/style_edit.rs`; `panels/style_edit_grid_rows.rs`; `crates/designos-app/src/widgets/picker.rs`. Reads only: the right-panel constants in `theme.rs` (`:315-393`, A and B). |

Code baseline: `origin/main` `205c72ef` (checked with `git show 205c72ef:<path>`, no checkout). Between `35a65796` and `205c72ef` F's files changed only where line numbers do not move for the cites here: `panels/properties.rs` (2 lines replaced at `:262`), `panels/style_edit.rs` (2 lines replaced at `:31`) and `panels/style_picker.rs` (+46 lines, #1112 "delete style", now merged); `theme.rs` lost 3 lines after `:263` (#1106), and every `theme.rs` cite here was re-anchored by that offset. The section 2 census gives the same output at `35a65796` and `205c72ef` (exit 0 at both).

Drift after `205c72ef` (measured 2026-10-25 against `origin/main` `649da124`; `649da124` #1151 touches only `crates/designos-mcp`): `panels/export.rs` gained 2 lines after `:208` and 1 after `:314` (#1148, `7e15ddae`), so the allowlisted tint `export.rs:308` sits at `:310`; `theme.rs` gained 8 lines after `:290` (#1108), so every `theme.rs` cite here (all after `:290`) sits 8 lines lower; no other F file changed. Re-anchor against the HEAD you branch from and report drift before writing code. Open PR #1113 still changes `panels/typography.rs` (section 12).

Naming follows the plan's contract: a colour is named by the token-contract Rust constant, its Untitled token name and the dark hex. A name the contract does not have is marked "new name" and is a request to A. Widgets are named by spec C's anatomy names.

## 1. Summary and user story

As a designer, I read and edit properties in the right panel in Untitled UI dark: Untitled property fields, selects, segmented controls, checkboxes and section headers at the small end of the scale, neutral fills, the Studio orange only on the one primary action (Share) and on live state, and a clear focus ring. In Dev mode the same panel shows the Inspect tab in the same style. The panel stays 240 px wide with its header, tab strip, sections and footer where they are (R1). Sections get longer where R8.4 raises fields to 32 and R5 raises labels; the inspector already scrolls.

## 2. Current state (FACT unless labelled)

- **Panel shell** (`right.rs:11-181`). Version history and comment mode replace the whole panel (`:13-20`, specs J/history and J/comments). Fill `BG`, left hairline `BORDER` (`:22-23`). Header: avatar pill 38×24 with a literal amber `#F59E0B` circle and a white initial (`:28-46`); Present button 24 + chevron 16 (`:49-61`; the chevron is `RIGHT_PRESENT_CHEVRON_W` 16, `theme.rs:327`, below the 24 px pointer floor; 6 px gap before Share); Share 48×24 filled literal `#0D99FF` (or `#1BC47D` in Dev mode) with a white label (`:64-70`; white on `#0D99FF` is 2.99:1); every header button is `RIGHT_BTN_SIZE.min(24.0)` = 24 (`:28`); Present starts at `right − 102` and the avatar ends at x 46, so about 92 px of the header row are free. Tab strip at y 48, height `RIGHT_TABS_H` 24 (`theme.rs:328-329`): "Design | Prototype" or, in Dev mode, "Inspect | Plugins" (`:104-129`), selected fill `BG_SECONDARY`, hover `BG_HOVER`, labels `TEXT`/`TEXT_SECONDARY`. Zoom dropdown 55×24 (`:132-141`, menu drawn by spec D). Content hairline at `RIGHT_CONTENT_TOP` 81 (`theme.rs:334`, `:143-145`). Footer 36 px with a help circle in literals `#2C2C2C`/`#383838`/`#B3B3B3` (`:161-180`). Dev plugins inspector (`:183-315`, literals `#1BC47D` at `:203`, `:274` and a white label at `:276`). Frame presets inspector when the Frame tool is active with nothing selected (`:317-381`).
- **Design tab** (`properties.rs`). Pane geometry: fields 24 tall (`FIELD_HEIGHT` = `CONTROL_DENSE`, `theme.rs:343`), half fields 88 wide at x 17/113 (`:344-345`), section content 184 wide (`:341`), title rows 40 (`SECTION_TITLE_HEIGHT`, `:339`), sublabels one caption line 14 (`LABEL_BOX_H`, `:362`). Titles `font_body_strong` 11 (`properties.rs:207-227`), sublabels caption 10 (`:285-294`), segmented icon groups (`:299-336`), checkbox (`:338-353`). Sections: page (`:389-453`), styles (`:454-606`) and library styles (`:607-656`) when nothing is selected; selection colours (`:657-710`); selection header, position, rotation, dimensions, resizing, auto layout and grid tracks (`:711-1427`); fill and stroke paint rows (`:1466-1624`); colour picker popup (`:1819-2038`) and on-canvas gradient handles (`:2096-2258`). Literal buttons: "Copy to Slide Deck" `#2C2C2C`/`#383838` (`:759-770`), "Update auto layout" the same (`:1225-1236`). Grey placeholder swatches for image and empty paints `Color32::GRAY`/`from_gray(150)` (`:474`, `:572`, `:634`, `:1444-1445`, `:2316`). Gradient handles on the canvas in white with black-alpha outlines (`:2117-2119`, `:2154-2155`, `:2184`). Document colours are converted by `to_c32` (`:188-191`).
- **Other inspector sections.** `components.rs` (component and instance sections; toggle knob literal white `:86`), `export.rs` (export rows and preview; preview image tint white `:308`), `stroke.rs`, `effects.rs` (`:264` converts a document colour), `typography.rs` (type section and the type-details popover: preview box `#1E1E1E` `:634`, baseline guide `#0D99FF` at 110 alpha `:637`, stylistic-set chips 24 tall with a white label on `BG_SELECTED` when on, `BG_TERTIARY` on hover, `BG_SECONDARY` with `TEXT_SECONDARY` when off, `:760-779`; `:297` converts a document colour), `layoutguide.rs` (`:99` converts), `vertex.rs`, `var_bind.rs` (bind-variable hover button and bound pill), `style_picker.rs` (effect/grid style buttons), `style_edit.rs` + `style_edit_grid_rows.rs` (the "Edit style" popover), `proto_actions.rs` (interaction action list).
- **Prototype tab** (`prototype.rs`). "Show prototype settings" button in literals `#2C2C2C`/`#383838` with a white label (`:58-60`); headings in white (`:87`); help text `#B3B3B3` (`:98`, `:102`); device thumbnails and "Manual" label white when selected (`:705`, `:727`); glyph buttons white when selected (`:848`); `:205` and `:755` convert the document's prototype background colour.
- **Dev Inspect tab** (`inspect.rs:313-823`, drawn when Dev mode is on, `right.rs:153-154`). Header name `font_large_strong` 13 with link/code/more icons that turn white on hover (`inspect.rs:350`, `:358`, `:372`); "Selection details" in `#B3B3B3` (`:441`); MCP section title white (`:415`); prompt box `#1E1E1E` + `#383838` with `#D4D4D4` and a `#0D99FF` URI (`:481-496`); Code Connect title white (`:511`), pill `#164E33`/`#454545`/`#383838` with `#4ADE80`/`#B3B3B3` text (`:516-528`); descriptions `#8C8C8C` (`:550-552`, `:624`); Layer properties title white (`:570`); measurement pill label white on `MEASURE` (`:823`, canvas overlay). `inspect.rs:17-21` converts document colours. The Dev sections inside it (component, code, variables, assets, playground) are `dev/sections.rs` and `dev/playground.rs`, spec G.
- **Colour picker** (`widgets/picker.rs`, opened from paint rows). Handle rings white with black alpha (`:666`, `:670`), grey placeholder `from_gray(0x80)` (`:681`), contrast bar text `#E4E4E7` (`:946`) and contrast badges AAA/AA `#17302A`/`#4CC58B`, AA Large `#332715`/`#F5A524`, Low `#361D21`/`#F3657A` (`:951-957`), eyedropper dot (`:1001-1002`), swatch inner ring white 10 % (`:1193`), slider thumb white with a black-alpha ring (`:1484-1485`). `:158` converts a document colour.

Measured colour census (non-test code, `scripts/rust_literal_scan.clean`; definitions as in spec D section 2):

| File | `theme::` lines | Chrome sites | Site lines | Computed sites |
|---|---|---|---|---|
| `panels/right.rs` | 47 | 13 | 38, 39, 66, 68, 173, 175, 176, 203, 274, 276 | none |
| `panels/properties.rs` | 162 | 19 | 474, 572, 634, 763, 765, 1230, 1232, 1444, 1445, 2117, 2118, 2119, 2154, 2155, 2184, 2316 | 190 |
| `panels/inspect.rs` | 62 | 20 | 350, 358, 372, 415, 441, 481, 482, 488, 496, 511, 516, 518, 520, 528, 550, 552, 570, 624, 823 | 19 |
| `panels/prototype.rs` | 95 | 10 | 58, 59, 60, 87, 98, 102, 705, 727, 848 | 205, 755 |
| `panels/components.rs` | 59 | 1 | 86 | none |
| `panels/export.rs` | 28 | 1 | 308 | none |
| `panels/typography.rs` | 72 | 3 | 634, 637, 779 | 297 |
| `widgets/picker.rs` | 84 | 17 | 666, 670, 681, 946, 951, 953, 955, 957, 1001, 1002, 1193, 1484, 1485 | 158 |
| `panels/effects.rs` | 8 | 0 | | 264 |
| `panels/layoutguide.rs` | 9 | 0 | | 99 |
| `proto_actions.rs`, `stroke.rs`, `vertex.rs`, `var_bind.rs`, `style_picker.rs`, `style_edit.rs`, `style_edit_grid_rows.rs` | 24, 7, 8, 9, 9, 2, 0 | 0 | | none |
| **Total** | 685 | **84** | | 8 (all document-colour converters) |

Command (run from the repo root, or from the root of a `git archive` of the ref; writes nothing). Its output at `205c72ef` equals the table (exit 0, run 2026-10-25 from the root of `git archive 205c72ef crates/designos-app/src scripts/rust_literal_scan.py`).

```bash
PYTHONDONTWRITEBYTECODE=1 python3 - panels/right.rs panels/properties.rs panels/inspect.rs panels/prototype.rs panels/proto_actions.rs panels/components.rs panels/export.rs panels/stroke.rs panels/effects.rs panels/typography.rs panels/layoutguide.rs panels/vertex.rs panels/var_bind.rs panels/style_picker.rs panels/style_edit.rs panels/style_edit_grid_rows.rs widgets/picker.rs <<'PY'
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

Output at `205c72ef` (exit 0, pasted from the run):

```text
panels/right.rs theme_lines=47 chrome_sites=13 [38, 39, 66, 68, 173, 175, 176, 203, 274, 276] computed=[]
panels/properties.rs theme_lines=162 chrome_sites=19 [474, 572, 634, 763, 765, 1230, 1232, 1444, 1445, 2117, 2118, 2119, 2154, 2155, 2184, 2316] computed=[190]
panels/inspect.rs theme_lines=62 chrome_sites=20 [350, 358, 372, 415, 441, 481, 482, 488, 496, 511, 516, 518, 520, 528, 550, 552, 570, 624, 823] computed=[19]
panels/prototype.rs theme_lines=95 chrome_sites=10 [58, 59, 60, 87, 98, 102, 705, 727, 848] computed=[205, 755]
panels/proto_actions.rs theme_lines=24 chrome_sites=0 [] computed=[]
panels/components.rs theme_lines=59 chrome_sites=1 [86] computed=[]
panels/export.rs theme_lines=28 chrome_sites=1 [308] computed=[]
panels/stroke.rs theme_lines=7 chrome_sites=0 [] computed=[]
panels/effects.rs theme_lines=8 chrome_sites=0 [] computed=[264]
panels/typography.rs theme_lines=72 chrome_sites=3 [634, 637, 779] computed=[297]
panels/layoutguide.rs theme_lines=9 chrome_sites=0 [] computed=[99]
panels/vertex.rs theme_lines=8 chrome_sites=0 [] computed=[]
panels/var_bind.rs theme_lines=9 chrome_sites=0 [] computed=[]
panels/style_picker.rs theme_lines=9 chrome_sites=0 [] computed=[]
panels/style_edit.rs theme_lines=2 chrome_sites=0 [] computed=[]
panels/style_edit_grid_rows.rs theme_lines=0 chrome_sites=0 [] computed=[]
widgets/picker.rs theme_lines=84 chrome_sites=17 [666, 670, 681, 946, 951, 953, 955, 957, 1001, 1002, 1193, 1484, 1485] computed=[158]
```

The plan's line method (`grep -cE 'Color32::from_rgba?(_unmultiplied|_premultiplied)?\(|Color32::from_gray\(' <file>`, tests included) gives `inspect.rs` 13, `properties.rs` 6, `right.rs` 7, `prototype.rs` 6, `picker.rs` 7, `typography.rs` 3, `effects.rs` 1, `layoutguide.rs` 1. It counts the 8 converters and misses named colours, so this spec uses the census above.

Classification of the 84 chrome sites (per-line counts from the same command with a `Counter`; 60 + 16 + 7 + 1 = 84):
- **Chrome colour choices (60)**: become contract constants (section 3).
- **Canvas or content overlays (16)**: gradient handles (`properties.rs:2117`, `:2118`, `:2119`, `:2154`, `:2155`, `:2184` ×2; 7 sites), picker handles and thumbs drawn over the colour field (`picker.rs:666`, `:670`, `:1001`, `:1002`, `:1193`, `:1484`, `:1485`; 7 sites), the measurement pill label (`inspect.rs:823`) and the type-details baseline guide (`typography.rs:637`). They keep their values and move to theme constants (A; `CANVAS_HANDLE_FILL` exists, the others are new names).
- **Placeholder swatches (7)**: the grey shown for image or empty paints (`properties.rs:474`, `:572`, `:634`, `:1444`, `:1445`, `:2316`; `picker.rs:681`). They take one constant (decision 4).
- **Identity image tint (1)**: `export.rs:308` passes `Color32::WHITE` as the tint of `Painter::image`, not a colour choice. It is the only allowlisted site.
- The 8 computed sites convert document colours (content) and stay.

Gates today: no colour-literal gate (spec A rule 13 adds `scripts/check_chrome_colour_literals.py`, new name). `scripts/check_spacing_relationships.py` ratchets geometry and font literals (`properties.rs`, `inspect.rs`, `prototype.rs`, `right.rs`, `picker.rs` all have baseline entries). `scripts/zero_fake_ui_gate.py` Z10 (Share copies a link; Dev Mode's 2nd tab is the plugins inspector), Z11 (`dev-code-connect`, `dev-mcp-btn`, `dev-add-redlines`, `copy-to-slides` do real work) and Z12 (picker image mode) cover this surface. Theme tests pin `RIGHT_TABS_H == CONTROL_DENSE` (`theme.rs:846`) and `LABEL_BOX_H + SPACE_FACT <= CONTROL_DENSE` (`theme.rs:887-890`).

## 3. Behaviour rules

1. **Colours from constants only.** The 60 chrome choices and the 7 placeholders become contract constants, the 16 overlay sites become theme constants, and only `export.rs:308` stays literal (allowlisted identity tint).
2. **Role map** (token-contract constants; widget states are C's, rule 3):

| Element | State | Constant (Untitled role, hex) | Today |
|---|---|---|---|
| Panel fill, footer fill | default | `BG` (bg-primary `#0A0A0A`) | `BG` |
| Panel edge, content hairline, footer hairline, section separators | default | `BORDER` (border-secondary `#262626`), C `Divider/Hairline` | `BORDER` |
| Section titles (Position, Auto layout, Fill, MCP, Code Connect, Layer properties, Prototype headings) | default / dim | C `Section/Header`: `TEXT` `#FAFAFA`, text-sm 14/20 semibold; dim titles `TEXT_TERTIARY` `#A3A3A3` | `TEXT`, `TEXT_SECONDARY`, literal white |
| Sublabels (Position, Rotation, Flow …), "Selection details", group labels | default | `TEXT_TERTIARY` `#A3A3A3`, text-xs 12/18 medium (C `Section/Header` sublabel) | caption 10, `#B3B3B3`, `#8C8C8C` |
| Help and description text (Prototype help, Code Connect description) | default | `TEXT_TERTIARY` `#A3A3A3` (C-1.7) | `#B3B3B3`, `#8C8C8C` |
| Field values, numbers, hex codes | default | `TEXT`; numbers stay Roboto Mono (P5, D6) at text-xs 12 | `TEXT`, 11 |
| Property fields (number, text, hex, opacity) | default, hover, focused, disabled | C `Input/PropertyField` xs 32 (R8.4): fill `BG`, border `BORDER_STRONG` `#404040`, value `TEXT`, prefix letter or icon `TEXT_TERTIARY`; focused border `BORDER_SELECTED` `#FB923C` 2 px and `FOCUS_RING`; disabled at 50 % | `BG_SECONDARY` fills, no border |
| "Mixed" value in a field | mixed selection | `TEXT_PLACEHOLDER` `#737373` (R8.1, contract named exception E-1; only on `BG` or `BG_SECONDARY` fills) | `TEXT_SECONDARY` |
| Selects (resizing, blend, font, effect type, variable modes) | default, open | C `Select/Trigger` xs 32: as `Input/PropertyField`, chevron `ICON_QUATERNARY`; open uses the focused look | `BG_SECONDARY` |
| Segmented icon groups (alignment, flow, text align) | default, hover, selected | C `Tabs/Segmented`: track `BG_SECONDARY` `#171717`, glyph `ICON_QUATERNARY` → `ICON_QUATERNARY_HOVER` on hover; selected tile `BG_SELECTED` `#262626` (R8.2, contract row 18's rule) with glyph `ICON_SECONDARY` `#D4D4D4` | `BG_SECONDARY`, `BG_SELECTED`, white |
| Secondary buttons (Copy to Slide Deck, Update auto layout, Show prototype settings, + Add redlines, Copy MCP link, Send to Agent Terminal) | default, hover, pressed, focused | C `Button/Secondary` xs 32: fill `BG`, border `BORDER_STRONG`, label `TEXT_SECONDARY` text-sm 14 Semibold (R8.6); hover `BG_HOVER` with `TEXT_SECONDARY_HOVER`; pressed `BG_PRESSED` | `#2C2C2C`/`#383838` literals, white label |
| Share | default, hover, pressed, focused | C `Button/Primary` xs 32: `BG_BRAND` `#E36323` → `BG_BRAND_HOVER` `#F97316` → `BG_BRAND_PRESSED` `#EA580C`, label `TEXT_ON_BRAND` `#0A0A0A` text-sm 14 Semibold (R8.6; C-1.14–C-1.16), in both modes (decision 1) | `#0D99FF` / `#1BC47D` + white |
| Present, Present chevron, zoom, avatar pill | default, hover | C `Button/Tertiary` xs (zoom: C `Select/Trigger` borderless) | `button_bg`, `BG_HOVER` |
| Avatar circle | default | fill `UTILITY_ORANGE_600` `#FB923C` (contract N46) with the initial in `TEXT_ON_BRAND` (8.75:1) | `#F59E0B` + white |
| Design/Prototype (Inspect/Plugins) tabs | default, hover, selected | C `Tabs/Pill` (C cites `right.rs:103-127`): label `TEXT_QUATERNARY`; hover `BG_HOVER` with `TEXT_SECONDARY`; selected `BG_SELECTED` with `TEXT_SECONDARY`; height 24 (R10.1) | `BG_SECONDARY`, `TEXT_SECONDARY` |
| Checkbox (Clip content, …) | off, on, focused | C `Checkbox/Box`: off border `BORDER_STRONG` `#404040` 1 px (R10.7, contract named exception E-2); on `BG_BRAND` with the check in `TEXT_ON_BRAND` (C-1.14); focused `FOCUS_RING` | `BG_BRAND` + white check |
| Toggle (component boolean props, `components.rs:86`) | off, on | C `Toggle/Switch`: track `BG_TERTIARY` `#262626` off, `BG_BRAND` on; knob `ICON_WHITE` (N17) | white knob |
| Stylistic-set chips in type details (`typography.rs:760-779`) | off, hover, on | C `Tag/Chip` selectable: off `BG_SECONDARY` + `TEXT_SECONDARY`; hover `BG_SECONDARY_HOVER` (S6 on a bg-secondary chip); on `BG_SELECTED` + `TEXT` (C-1.3) | `BG_SECONDARY`, `BG_TERTIARY`, `BG_SELECTED` + white |
| Help button in the footer | default, hover | C `Button/Icon` xs 32 inside the 36 px footer: glyph `ICON_QUATERNARY` → `ICON_QUATERNARY_HOVER`, hover `BG_HOVER` | `#2C2C2C`/`#383838`/`#B3B3B3` |
| Link-like text (MCP URI) | default | `TEXT_BRAND` (fg-brand-secondary `#F97316`, R8.3; C-1.11 on the prompt box) | `#0D99FF` |
| Prompt and code boxes (MCP prompt, type-details preview) | default | fill `BG_SECONDARY` `#171717` + `BORDER`; text `TEXT_SECONDARY` `#D4D4D4`, mono | `#1E1E1E`, `#383838`, `#D4D4D4` |
| Code Connect status pill | connected / not started / hover | status badges (contract G-1): connected `BG_SUCCESS_PRIMARY` `#052E16` + border `UTILITY_GREEN_200` + text `UTILITY_GREEN_700` `#86EFAC` (C-1.21); not started C `Badge/StatusCount` gray `UTILITY_NEUTRAL_50`/`_200`/`_700` (C-1.24); hover per C | `#164E33`/`#4ADE80`, `#383838`/`#B3B3B3`, `#454545` |
| Picker contrast badges AAA, AA / AA Large / Low | default | status badges (G-1): success `BG_SUCCESS_PRIMARY` + `UTILITY_GREEN_200` + `UTILITY_GREEN_700`; warning `BG_WARNING_PRIMARY` + `UTILITY_YELLOW_200` + `UTILITY_YELLOW_700`; error `BG_ERROR_PRIMARY` + `UTILITY_RED_200` + `UTILITY_RED_700` (C-1.21–C-1.23); ratio text `TEXT` | `#17302A`/`#4CC58B`, `#332715`/`#F5A524`, `#361D21`/`#F3657A`, `#E4E4E7` |
| Type-details baseline guide | default | a canvas-overlay constant (it draws a guide over a type preview); value stays blue `#0D99FF` at 110 alpha, moved into `theme.rs` (new name) | literal |
| Placeholder swatch (image or empty paint) | default | `SWATCH_PLACEHOLDER` (new name, value bg-quaternary `#404040`, decision 4) | `GRAY`, `from_gray(150)`, `from_gray(0x80)` |
| Gradient handles, picker handles, slider thumbs, measurement pill label | default | canvas/content overlay constants (white fill, black-alpha outline) in `theme.rs` (A; `CANVAS_HANDLE_FILL` exists, others new names); C `Slider/Handle` for the picker sliders | literals |
| Dev plugins row icon and "Manage & Run Plugins…" button (`right.rs:203`, `:274-276`) | default, hover | `DEV_READY` `#14AE5C` (R10.4; G's Dev accent role), label `TEXT_ON_BRAND` (C-1.35); "Ready" text in `DEV_READY` on `BG` (C-1.36) | `#1BC47D` + white (2.27:1) |
| Icon buttons in the Inspect header (link, code, more) | default, hover | C `Button/Icon`: `ICON_QUATERNARY` → `ICON_QUATERNARY_HOVER`; never pure white | `ICON_SECONDARY` → white |
| Component and instance names, icons, property chips | any | Studio component cyan stays (controller default): `TEXT_COMPONENT`, `COMPONENT` | same |
| Document colours (fills, strokes, effects, page background, prototype background) | n/a | content: drawn from the document, unchanged | `to_c32`, `c32` converters |

3. **Widgets (spec C).** Number, text, hex and opacity fields: `Input/PropertyField` xs 32 (R8.4; prefix letter or icon, suffix unit). Dropdowns (resizing, blend, font, effect type, variable modes, zoom): `Select/Trigger`, opening C's `Menu/Panel`. Segmented icon groups: `Tabs/Segmented`; the tab strip: `Tabs/Pill`. Checkbox: `Checkbox/Box`; toggle: `Toggle/Switch`; stylistic sets: `Tag/Chip`. Section headers with "+"/"−" actions: `Section/Header` with `Button/Icon` xs actions. Buttons: `Button/Primary`, `Button/Secondary`, `Button/Tertiary`, `Button/Icon` at xs 32. Swatches: `Swatch/Color`. Popovers (stroke settings, type details, effect settings, edit style, picker): `Popover/Panel` (radius 16, R8.5; rows 32). Status pills and contrast badges: G-1 status badges; counts: `Badge/StatusCount`. Every hover tip: `Tooltip/Default` (fill `BG_PRIMARY_SOLID`, text `TEXT`, C-1.19). Scrollbar: C's (`panels/mod.rs`).
4. **States.** Fields: Default, Hover, Focused, Disabled, Mixed (today's "Mixed" text stays) and Bound-to-variable (var_bind pill; C `Tag/Chip`). Buttons and tabs: C's Default, Hover, Pressed, Focused, Disabled, Selected. No state is removed; the Pressed state comes with C's widgets.
5. **Sizes (R5, R8.4, R8.6, R10.1) and their layout effect.** Panel width 240, the header, tab strip, section order, field columns (x 17/113, 88 wide) and the 36 px footer keep their places (R1).

| Element | Constant / code | Today | Target | Δ | Layout effect |
|---|---|---|---|---|---|
| Header buttons (avatar, Present + chevron, Share) | `right.rs:28` (`RIGHT_BTN_SIZE.min(24)`), `RIGHT_PRESENT_CHEVRON_W` (`theme.rs:327`) | 24 tall; Present 24, chevron 16, Share 48 wide | button xs 32 tall; Present 32, chevron 24 (P3 pointer floor), Share fits its 14 Semibold label (INFERENCE about 64) | +8 height | they occupy y 8..40; the tab strip stays at y 48; order and right alignment stay; the extra width (about 32 px) comes from the 92 px of free header space between the avatar and Present (decision 6); measured in the PR |
| Tab strip | `theme.rs:328-329` | 24 at y 48 | 24 (R10.1: R1 wins over R5) | 0 | none; `RIGHT_TABS_H == CONTROL_DENSE` (`theme.rs:846`) stays |
| Section title rows | `theme.rs:339` | 40 | 40 | 0 | title type 11 → 14 |
| Field rows | `theme.rs:343`, pitch 24 + 8 | 24 | `CONTROL_XS` 32 (R8.4) | +8 per row | for "Welcome to Untitled UI" (a 1600×6208 auto-layout frame, capture `D-07`) the visible inspector holds about 19 field rows and 2 buttons: about +152 px of rows and +16 px of buttons (INFERENCE: counted on the capture); the inspector scrolls |
| Sublabels | `LABEL_BOX_H` 14 (`theme.rs:362`) | 14 | text-xs 12/18 → 18 | +4 per labelled group | 8 labelled groups on `D-07`: +32 px; B updates `line_heights_fit_their_roles` (18 + `SPACE_FACT` 4 = 22 ≤ 24 still holds for a 24 slot, and fits a 32 slot) |
| Values and labels | `font_body` 11, `font_mono` 11 | 11 | text-xs 12 | 0 | values elide earlier in 88 px half fields (variable-mode names "Auto (Sty…" already elide) |
| Button labels (Share, secondary buttons) | `font_body_strong` 11 | 11 | text-sm 14 Semibold (R8.6) | 0 | labels fit the 184 px content width; Share widens (row 1) |
| Style rows | `STYLE_ROW_H` 32 (`theme.rs:368`) | 32 | row 32 | 0 | none |
| Footer | `right.rs:144` | 36, help 28 | 36, `Button/Icon` xs 32 | 0 | none |
| Popovers (picker, type details, stroke, effects, edit style) | `PROTO_POPOVER_WIDTH`, `VAR_POPOVER_W` and file-local sizes | as today | widths unchanged; rows 32 per C | + per row as above | popovers get taller; they already clamp to the window (C `Popover/Panel`) |

6. **Radii.** Fields, selects, buttons and segmented tracks take `RADIUS_CONTROL` 8 (R5); popovers `RADIUS_FRAME` 16 with rows 8 at inset 8 (R8.5); segment tiles `RADIUS_SEGMENT` 6; checkboxes radius-xs 4 (C); swatches keep their shape (C `Swatch/Color`). Literal radii in owned files (`cr(6)`, `cr(4)`, `cr(3)`, `CornerRadius::same(6)`) become B constants and leave the spacing ratchet's radius count.
7. **No behaviour change.** Widget ids (`share-button`, `right-tab-*`, `copy-to-slides`, `al-upgrade-v1`, `dev-mcp-btn`, `dev-code-connect`, `dev-add-redlines`, `picker-contrast-bar`, …), copy actions, keyboard tab order in fields, scrubbing on prefixes, Z10, Z11 and Z12 stay. Hit boxes change only where R5, R8.4 or the 24 px floor requires it.

## 4. Data model / schema changes

None.

## 5. Contracts

**File ownership across D–K.** F owns:

| File | Owner | Shared surface and rule |
|---|---|---|
| `panels/right.rs` | F | the history panel and comments panel that replace the inspector (`:13-20`) are not restyled here (J); the zoom dropdown's menu is D's (`panels/menus.rs`) |
| `panels/properties.rs` | F | the page-level Styles section (`:454-656`) overlaps K's "Styles" scope: F owns it because it is inspector content; K lists this file as "not mine" |
| `panels/inspect.rs` | F | Dev mode's Inspect panel: F owns the file; the Dev sections it calls (`dev/sections.rs`, `dev/playground.rs`) and the syntax colours are G's |
| `panels/prototype.rs`, `panels/proto_actions.rs` | F | none |
| `panels/components.rs`, `panels/export.rs`, `panels/stroke.rs`, `panels/effects.rs`, `panels/typography.rs`, `panels/layoutguide.rs`, `panels/vertex.rs`, `panels/var_bind.rs` | F | `var_bind.rs` is the inspector side of variables (its `//!` doc); the Variables panel itself is K's |
| `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` | F | shared with K ("Styles"); F owns them because they open from inspector rows; K lists them as "not mine" |
| `widgets/picker.rs` | F | the only `widgets/` file not owned by C (assigned to F by the plan brief) |

Not mine: `theme.rs`, `design/*` (A, B); `widgets/field.rs`, `widgets/menu.rs`, `widgets/mod.rs`, `widgets/popover.rs`, `widgets/field_tab.rs`, `widgets/popup_open.rs`, `panels/mod.rs` (C); `tabs.rs`, `shell.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `dev/*`, `specs.rs` (G); `panels/context.rs`, `history/*`, `comments/*` (J); `panels/assets.rs`, `variables/*`, `libraries/*` (K). Unassigned: `panels/check_designs.rs`.

**Colour-literal gate.** Spec A rule 13 adds `scripts/check_chrome_colour_literals.py` (new name). F's allowlist: `export.rs:308` (identity image tint). Until the gate exists, the section 2 command must print `chrome_sites=0` for every owned file except `panels/export.rs` (1).

## 6. File-format mapping

None. Document colours shown in fields and swatches are unchanged.

## 7. UI

- Read the in-repo design sources first: `design/principles.md`, `design/soul.md`, `specs/token-contract.md` (A), spec B's scale and spec C's widget kit; they win over generic defaults. The Purple Ban stays (R4). Contributors who use the project's agent kit may also load its `es-designer` skill; it is not required.
- One primary action per view: Share is the only `Button/Primary` in the panel (P2). Selected tabs, segments and tool states stay neutral.
- Numbers, hex codes and token paths stay in Roboto Mono (P5, D6).
- Copy is unchanged; Z16 stays green.
- Evidence goes into the PR body (not Figma, R3). Responsive: SKIPPED (owner default).

## 8. Edge cases

1. Long values at 12 px in 88 px half fields (for example "Auto (Style…)" variable modes, long font names): elide with an ellipsis; never overlap the chevron or suffix.
2. Mixed selection: "Mixed" uses `TEXT_PLACEHOLDER` `#737373` (R8.1). It is the contract's named exception E-1 and sits only in fields filled `BG` (4.18:1) or `BG_SECONDARY` (3.78:1), never on `BG_SELECTED` or `BG_PRESSED`.
3. A field bound to a variable shows C's bound `Tag/Chip`; the hover "bind" button keeps its hit area inside the field.
4. Dev mode: Share stays brand orange (decision 1); the Plugins row and its button take `DEV_READY` with a dark label (R10.4).
5. The colour picker over a very light or very dark document colour: handles keep a white fill with a black-alpha outline so they stay visible on any content.
6. Popovers taller after the 32 px rows (picker, edit style): they clamp to the window and scroll as C's `Popover/Panel` does today.
7. The minimum window (900×600): the inspector scrolls; header, tab strip and footer stay fixed.
8. No selection: page and styles sections; Frame tool active with no selection: frame presets list. Both restyle with the same constants.
9. Unchecked checkboxes and field borders: the kit's `BORDER_STRONG` `#404040` is 1.91:1 on `BG`. For checkboxes it is the owner's named exception E-2 (R10.7). Property and select borders use the same kit value and are not covered by E-2 by name: sent to A (section 12).

## 9. Non-goals

- Inspector layout (section order, column x positions, panel width), new sections or new controls.
- The Dev sections' content and syntax highlighting (G), history and comments panels (J), the Variables panel and libraries (K), menus drawn by D.
- Canvas overlay colours (they move into constants with the same values), document content.

## 10. Test plan (red first)

Acceptance criteria:

1. The section 2 command (or A's gate) prints `chrome_sites=0` for every owned file except `panels/export.rs` (`chrome_sites=1`, line 308, allowlisted).
2. `python3 scripts/check_spacing_relationships.py` passes; per-file counts stay at or below `scripts/spacing_baseline.json` (radius literals fall).
3. Spec L's layout gate shows the same regions, widths, column positions and control order as the baseline; height changes match the rule 5 table.
4. Every constant in section 3 is a token-contract constant or a new name A has added; `theme::` tests pass with B's updated invariants (`LABEL_BOX_H`, `FIELD_HEIGHT`; `RIGHT_TABS_H` stays 24 by R10.1).
5. `python3 scripts/zero_fake_ui_gate.py` (Z10, Z11, Z12) and `python3 scripts/design_os_self_evolve.py` pass.
6. **Contrast.** `ui ds a11y` is not accepted: on this store it measured 0 pairs (false green, plan.md and contract (f)). The check is three parts:
   - (a) The contract's C-1 check (token-contract (f)) exits 0. Run 2026-10-25 from the specs folder against the contract as last edited at 22:15 (62 rows, C-1.1 to C-1.62): exit 0, 62 rows ok, 0 FAIL. F relies on C-1.1, C-1.3, C-1.5, C-1.7, C-1.11, C-1.14–C-1.16 (Share), C-1.19 (tooltips), C-1.21–C-1.24 (status and count badges), C-1.25–C-1.26 (Mixed, R8.1), C-1.28 (segmented glyphs on the track), C-1.29 (focus ring), C-1.30 (focused field outline; on a `BG` field the pair is C-1.37's) and C-1.35–C-1.36 (Dev accent, R10.4). Named exceptions E-1 (placeholder) and E-2 (checkbox border) are owner rulings, not passes.
   - (b) The F-P rows below, in the C-1 row shape, pass the contract's snippet with only the id prefix and the file changed. Run 2026-10-25 from the specs folder: exit 0, 12 rows ok (lowest: F-P.9 3.46:1).
   - (c) A adds the F-P rows to C-1 and to the `chrome_contrast_pairs` test (spec A section 10). Until then F's PR runs (b).
7. Before/after captures of every command below are in the PR with exit codes.
8. The Present chevron's hit box is at least `TARGET_MIN` wide (red test below).

F-P rows (foreground on background, WCAG 2.x, the contract's luminance formula):

| Id | Foreground | Background | Min | Use |
|---|---|---|---|---|
| F-P.1 | `TEXT_SECONDARY_HOVER` #E5E5E5 | `BG_HOVER` #171717 | 4.5 | Button/Secondary hover label |
| F-P.2 | `TEXT_ON_BRAND` #0A0A0A | `UTILITY_ORANGE_600` #FB923C | 4.5 | avatar initial |
| F-P.3 | `TEXT_QUATERNARY` #A3A3A3 | `BG` #0A0A0A | 4.5 | Tabs/Pill default label |
| F-P.4 | `TEXT_SECONDARY` #D4D4D4 | `BG_HOVER` #171717 | 4.5 | Tabs/Pill hover label |
| F-P.5 | `TEXT_SECONDARY` #D4D4D4 | `BG_SELECTED` #262626 | 4.5 | Tabs/Pill selected label |
| F-P.6 | `TEXT_SECONDARY` #D4D4D4 | `BG_SECONDARY` #171717 | 4.5 | prompt and code boxes, stylistic-set chip off |
| F-P.7 | `TEXT` #FAFAFA | `BG_MENU` #0A0A0A | 4.5 | picker contrast ratio text and popover titles |
| F-P.8 | `ICON_QUATERNARY` #737373 | `BG` #0A0A0A | 3.0 | field prefix icons, Inspect header icons, footer help |
| F-P.9 | `ICON_WHITE` #FFFFFF | `BG_BRAND` #E36323 | 3.0 | toggle knob on (non-text) |
| F-P.10 | `ICON_WHITE` #FFFFFF | `BG_TERTIARY` #262626 | 3.0 | toggle knob off |
| F-P.11 | `ICON_SECONDARY` #D4D4D4 | `BG_SELECTED` #262626 | 3.0 | segmented selected glyph |
| F-P.12 | `ICON_QUATERNARY_HOVER` #A3A3A3 | `BG_HOVER` #171717 | 3.0 | Button/Icon and segmented hover glyph |

```bash
# from the specs folder; the contract's C-1 snippet with the id prefix and the file changed; exits 1 on a failure
python3 - F-surface-right-panel-inspector-design-prototype.md <<'PY'
import re, sys
def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
bad = 0
for m in re.finditer(r"^\| (F-P\.\d+) \| `\w+` #(\w{6}) \| `\w+` #(\w{6}) \| ([\d.]+) \|", open(sys.argv[1]).read(), re.M):
    hi, lo = sorted((lum(m[2]), lum(m[3])), reverse=True)
    r = (hi + 0.05) / (lo + 0.05)
    ok = r >= float(m[4]); bad += not ok
    print(f"{m[1]:7} {r:5.2f}:1 (min {m[4]}) {'ok' if ok else 'FAIL'}")
sys.exit(1 if bad else 0)
PY
```

| Case | Expected | Fails before |
|---|---|---|
| census of the owned files | 0 (1 allowlisted in `export.rs`) | yes (84 sites) |
| `panels::right::tests::*` (`dev_mode_plugins_tab_and_share_avatar_help_execute_real_logic`, the frame-preset scroll tests) | pass | guard |
| `panels::inspect::tests::*`, `panels::typography::tests::*`, `widgets::picker::tests::*` | pass | guard |
| `theme::tests` `RIGHT_TABS_H == CONTROL_DENSE` (`theme.rs:846`) | pass unchanged (R10.1) | guard |
| new `panels::right::tests::share_is_the_brand_primary_with_a_dark_label` (new name) | Share fill is `BG_BRAND` and its label is `TEXT_ON_BRAND` in both modes | yes |
| new `panels::right::tests::present_chevron_hit_box_meets_the_target_floor` (new name) | the recorded `present-chevron` rect is at least `TARGET_MIN` wide and overlaps neither `present-button` nor `share-button` | yes (16) |
| new `widgets::picker::tests::contrast_badges_use_status_roles` (new name) | the AAA/AA/AA Large/Low badges paint the G-1 success, warning and error constants | yes |
| new `panels::properties::tests::field_rows_use_the_field_height` (new name) | a selected frame's X/Y row is `FIELD_HEIGHT` tall and `FIELD_HEIGHT` equals `CONTROL_XS` (32, R8.4) | yes (24) |

Run: `cargo test -p designos-app --bin designos panels::right`, `cargo test -p designos-app --bin designos panels::properties`, `cargo test -p designos-app --bin designos panels::inspect`, `cargo test -p designos-app --bin designos panels::prototype`, `cargo test -p designos-app --bin designos panels::typography`, `cargo test -p designos-app --bin designos widgets::picker`, `cargo test -p designos-app --bin designos theme::`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**Evidence commands.** `KIT` is a scratch copy of `~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (never the original).

```bash
cd "$(git rev-parse --show-toplevel)"   # repo root: cargo run -p needs it
export PATH="$HOME/.cargo/bin:$PATH"; S="$(mktemp -d)"; mkdir -p "$S/home" "$S/shots"; OUT="$S/shots"
KIT="$S/untitled-ui-kit-copy.fig"; cp "$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$KIT"   # a copy, never the original
RUN() { env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" cargo run -q --release -p designos-app --bin designos -- "$@"; echo "exit $?"; }
RUN "$KIT" --screenshot "$OUT/D-01-editor-default.png" --size 1600x1088 --no-toast                      # no selection: Page and Styles sections, header, tabs, footer (shared with D)
RUN "$KIT" --screenshot "$OUT/D-07-select-frame.png" --size 1600x1088 --no-toast --select "Welcome to Untitled UI"   # Design tab with a frame (shared with D)
RUN "$KIT" --screenshot "$OUT/F-02-prototype-tab.png" --size 1600x1088 --no-toast --tab prototype --select "Welcome to Untitled UI"   # Prototype tab
RUN "$KIT" --screenshot "$OUT/F-03-picker.png" --size 1600x1088 --no-toast --panel picker --select "Welcome to Untitled UI"           # colour picker, contrast badge
RUN "$KIT" --screenshot "$OUT/F-04-type-details.png" --size 1600x1088 --no-toast --panel type-details --select "Welcome to Untitled UI"  # type-details popover
RUN "$KIT" --screenshot "$OUT/D-09-dev-css.png" --size 1600x1088 --no-toast --dev css --select "Welcome to Untitled UI"             # Dev Inspect tab (shared with D and G)
```

Recorded exit codes, with the ref and machine state of each run:

| Capture | Exit | Ref and conditions |
|---|---|---|
| D-01, D-07, F-02, F-04 | 0 | spec writer, local build of `ec47da26`, 2026-10-25, many concurrent cargo builds from other agents |
| F-03 | 1 once ("designos: screenshot was not captured") | spec writer, same build and load; not rerun |
| D-09 | 1 twice (same message) | spec writer, same build and load; not rerun |

No capture was rerun at `205c72ef`. The cause of the exit-1 runs is not diagnosed (a `--screenshot` run gives up after 240 frames, `shell.rs:483`). The implementer reruns F-03 and D-09 on an idle machine and records the exit codes in the PR.

Not capturable from the CLI today: hover, focus, the Mixed state, a bound field (needs a document with variables bound to the selection), the Dev Plugins tab (no `--tab` value for it in Dev mode). The PR covers them with the new paint tests and a manual window capture, labelled manual.

## 11. Performance budgets

- Inspector paint stays one pass per frame with no new allocation per field; constants are `const`.
- Taller rows add no layout passes: the pane advances `y` by the new constants.
- No `request_repaint` added; idle CPU stays 0 % (CLAUDE.md section 4). The picker and gradient handles keep their current repaint behaviour.

## 12. Dependencies and build order

- A, B, C first; L's baselines before the first PR. Build order: constants in `right.rs` (header, tabs, footer), `properties.rs` sections and shared helpers (`title`, `sublabel`, `segmented`, `checkbox`), the other section files, `prototype.rs`, `inspect.rs`, `widgets/picker.rs` and the popovers; then R5/R8.4 sizes; then evidence.
- **G.** The Dev accent role; its value is `DEV_READY` `#14AE5C` (R10.4), so the Dev-mode Plugins items no longer wait on G.
- **Sent to A:** new names `SWATCH_PLACEHOLDER`, the type-details baseline guide constant and the handle/thumb overlay constants; the F-P rows for C-1 and `chrome_contrast_pairs`; whether E-2 (R10.7, unchecked checkbox border `#404040`, 1.91:1) also names the kit's property and select borders, which use the same value (edge case 9).
- **Sent to C:** `Tabs/Pill` at 24 in the inspector strip (R10.1; C derives xs 32); `Tabs/Segmented` Selected fill `BG_SELECTED` without a border (contract row 18 and R8.2; C's table says bg-primary_alt + border-primary).
- **Open PRs touching F files** (`gh pr view 1113` read 2026-10-25: OPEN): #1113 `fix/issue-1026-typography-style-link` (`panels/typography.rs`); stale #770, #768, #765 (`panels/inspect.rs`, `panels/properties.rs`, `panels/components.rs`, `panels/effects.rs`, `panels/style_edit.rs`, `panels/var_bind.rs`). #1112 is merged (inside `205c72ef`). Rebase on #1113 if it merges first.
- #1106 and #1108 touch no F file.

## 13. Owner rulings that apply

- R1 (layout unchanged), R2 (Untitled spacing and type), R3 (no Figma), R4 (Studio orange, dark label `#0A0A0A` 5.73:1, focus ring Orange-500 `#F97316`, Purple Ban), R5 (text-xs 12/18 rows and labels, text-sm 14/20 titles, button xs 32, row 32, radius-md 8), R6 (keep icons), R7 (spec-driven).
- R8.1 placeholder `#737373` (Mixed); R8.2 selected `#262626`; R8.3 accent text `#F97316` (MCP URI); R8.4 inspector field 32 (decides open question 2); R8.5 popovers 16 with rows 8 at inset 8; R8.6 button labels 14 Semibold, tab and row text 12. R8.7 does not apply (no destructive button in F's files). R9: figma-agent read-only is allowed; not used here.
- R10.1 the inspector tab strip stays 24 (decides open question 1); R10.4 Dev accent `DEV_READY` `#14AE5C`; R10.7 unchecked checkbox border `#404040` (E-2).
- Controller defaults: canvas overlays and component cyan keep their values; chrome status colours move to Untitled. 21:35 sweep: "Inspector field 32 is R8.4 (F OQ2); placeholder is R8.1".
- P2: one primary action per view; selection is a neutral fill. P5: numbers in Roboto Mono.
- 2026-09-22: visual evidence in every PR. Standing: protected `main`. 2026-10-10: responsive SKIPPED by default.

## Decisions recorded by the spec writer (technical; owner may reverse)

1. Share is the panel's one primary action and takes `Button/Primary` in both Design and Dev mode (P2). Dev mode is shown by the toolbar, tab badge and Inspect tab, not by recolouring Share.
2. Chrome blue leaves the panel: Share, the MCP URI link and the type-details baseline guide were `#0D99FF`. The baseline guide is a drawn guide over a type preview, so it is treated like a canvas guide and keeps its blue, moved into a constant.
3. Status chips in the picker and Code Connect take the contract's G-1 status badges (success, warning, error), not hand-picked greens and reds.
4. Placeholder swatches for image and empty paints take one constant, `SWATCH_PLACEHOLDER` (bg-quaternary `#404040`), instead of three greys.
5. Handles drawn over document content (gradient handles, picker handles and thumbs) keep white with a black-alpha outline, because they must read on any colour; they move into `theme.rs` constants.
6. The header controls take C's xs sizes and the Present chevron its 24 px hit width (P3 pointer floor); the extra width comes from the free header space left of Present, so the order, the right alignment and the tab strip stay.

## Open questions for the owner

None. The tab strip height is decided by R10.1 (stays 24) and the field height by R8.4 (32).

## Changelog (pass of 2026-10-25 after board 3)

| Item | Resolution |
|---|---|
| Re-anchor to `205c72ef` | `theme.rs` cites moved by −3 (#1106); the other cited files did not shift; census identical at `35a65796` and `205c72ef` (exit 0); drift to `649da124` listed (`export.rs` +2, `theme.rs` +8). |
| Contract names | Provisional Untitled names replaced by contract constants (`TEXT_ON_BRAND`, `ICON_WHITE` N17, `BG_PRIMARY_SOLID` N19, G-1 status badges N24–N32, `UTILITY_ORANGE_600` N46, `UTILITY_NEUTRAL_*` N21–N23); C widget names (`Input/PropertyField`, `Select/Trigger`, `Tabs/Pill`, `Tabs/Segmented`, `Checkbox/Box`, `Toggle/Switch`, `Tag/Chip`, `Section/Header`, `Popover/Panel`, `Swatch/Color`, `Slider/Handle`). |
| AC6 | `ui ds a11y` replaced by C-1 (contract (f)) plus 12 F-P rows (exit 0); E-1 and E-2 named. |
| Open question 1 (tab strip) | Decided by R10.1: stays 24. |
| Open question 2 (field height) | Decided by R8.4: 32 (21:35 sweep). |
| R8 rulings | R8.1 (Mixed), R8.2, R8.3, R8.5 (popovers), R8.6 (button labels) applied. |
| R10 rulings | R10.1, R10.4 (Dev Plugins row), R10.7 (checkbox border) applied. |
| Pointer floor | The Present chevron (16 wide) gets a 24 px hit box (decision 6, red test). |
| Ownership | `updater.rs` is J's; unchanged here. #1112 merged. |
