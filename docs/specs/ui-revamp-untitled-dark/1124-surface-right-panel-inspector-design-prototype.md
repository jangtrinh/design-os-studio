# F Right panel: inspector header, Design and Prototype tabs, Dev Inspect panel, colour picker and inspector popovers in Untitled UI dark

| Field | Value |
|---|---|
| Issue | #1124 (epic #1118, spec key F) |
| Size | L. Four to five days: 17 owned files, 81 literal colour sites (56 chrome choices, 16 canvas overlays that move to theme constants, 8 placeholders, 1 allowlisted image tint), every inspector field and section on C's anatomy, R5/R8.4 field and label sizes in a 240 px panel, the header's new sizes, and four pointer-floor fixes (the 4-item flow group, the swatches, the Present chevron, the segment tiles). Splittable into F1 (header, tabs, footer, `properties.rs` sections) and F2 (`inspect.rs`, `prototype.rs`, picker and popovers) if the PM wants two PRs. |
| Depends on | A (colour constants, `specs/token-contract.md`, contrast check C-1, colour-literal gate), B (type, spacing, radius and control-size scale), C (widget anatomy and states, `specs/C-widget-kit-untitled-anatomy.md`) and L (layout-unchanged gate and baselines). G for the Dev accent role (its value `DEV_READY` `#14AE5C` is R10.4) and for the Dev sections drawn inside the Inspect panel. |
| Unblocks | M (docs and screenshot refresh). |
| Touches | `crates/designos-app/src/panels/right.rs` (`:11-181` header, tabs, footer; `:183-315` Dev plugins; `:317-381` frame presets); `panels/properties.rs` (`:177-368` pane, titles, sublabels, segmented, checkbox; `:404-822` page, styles and selection-colour sections; `:823-1534` selection, dimensions, resizing, auto layout, grid; `:1545-1788` paints; `:1928-2373` picker popup and gradient handles); `panels/inspect.rs` (`:17-21`, `:313-807`); `panels/prototype.rs`; `panels/proto_actions.rs`; `panels/components.rs`; `panels/export.rs`; `panels/stroke.rs`; `panels/effects.rs`; `panels/typography.rs`; `panels/layoutguide.rs`; `panels/vertex.rs`; `panels/var_bind.rs`; `panels/style_picker.rs`; `panels/style_edit.rs`; `panels/style_edit_grid_rows.rs`; `crates/designos-app/src/widgets/picker.rs`. Reads only: the right-panel constants in `theme.rs` (`:323-401`, A and B). |

Code baseline: `origin/main` `8dfaee93` (v0.7.0 merged, 2026-10-28; read with `git show 8dfaee93:<path>`, no checkout). The spec was written against `205c72ef`; every `file.rs:line` cite in sections 2 to 12 was moved to `8dfaee93` from the `git diff -U0` hunks between the two commits (44 of the 142 cites in this file moved) and each moved cite was checked to point at the same source line at both refs, except the ones listed in the drift note, which were re-found by hand. Cites in the changelog at the end are history. The census in section 2 was re-run at `8dfaee93` (section 10).

Drift to `8dfaee93` (measured 2026-10-28; 217 app files changed since `649da124`). `panels/right.rs`, `panels/prototype.rs`, `panels/components.rs`, `panels/stroke.rs` and `panels/layoutguide.rs` did not change. Changed: `panels/properties.rs` +227 (Styles section regrouped: folders with inline rename, the "Styles" title lost its "+", the page-level Export section now comes from `export::section`, the `typo-var-pill` 24×20 button is gone, and a `--panel export-toast` preset), `panels/typography.rs` +9 (a "Style" button in the Type title row, `:194`, and the linked-style row), `panels/effects.rs` +173 (effect-type caps with disabled "+" menu rows and "Up to N …" tips, and a Blend select on shadow settings that reads "Mixed" when the selected shadows differ), `panels/export.rs` +225 (the export toast: "Show in folder" and, with no file dialog, "Enter a path…"; spec D paints the toast), `panels/style_picker.rs` +279 (the "Style" button and menu for text, effect and grid styles, create and linked rows), `panels/style_edit.rs` (a "Description" row, so the popover has one more row), `widgets/picker.rs` +49 (style description tooltips, the blend dropdown's Figma group dividers) and `panels/inspect.rs` −16 lines. `theme.rs` +8 after `:290` (#1108). **Rules the drift touches.** (1) The Dev MCP prompt box moved out of `inspect.rs` into `dev/code_view.rs::prompt_box` (spec G's new file): it now paints `BG_SECONDARY`, a 1 px `BORDER_STRONG`, `TEXT` mono text and the link in `INFO` (`#0D99FF`, a chrome blue), so `inspect.rs` lost 4 chrome sites (81 sites now, was 84; one placeholder `Color32::GRAY` was added at `properties.rs:2490`); F's rows for the prompt box and its link now bind G (rule 2, section 5, section 12), and F decision 2 ("chrome blue leaves the panel") must reach `dev/code_view.rs`. (2) "Mixed" has a new form, the shadow Blend select (rule 2). (3) The pointer-floor audit lost one target (`typo-var-pill`, 20 tall) and gained one that already complies (the Style button is a `PANEL_ACTION_BUTTON` 24 at the 28 px pitch left of the Type settings button). (4) The Styles title row without "+" and the page-level Export title row are covered by the same title rules as before. Re-anchor against the HEAD you branch from and report drift before writing code. Open PR #1113 still changes `panels/typography.rs` (section 12).

Naming follows the plan's contract: a colour is named by the token-contract Rust constant, its Untitled token name and the dark hex. A name the contract does not have is marked "new name" and is a request to A. Widgets are named by spec C's anatomy names.

## 1. Summary and user story

As a designer, I read and edit properties in the right panel in Untitled UI dark: Untitled property fields, selects, segmented controls, checkboxes and section headers at the small end of the scale, neutral fills, the Studio orange only on the one primary action (Share) and on live state, and a clear focus ring. In Dev mode the same panel shows the Inspect tab in the same style. The panel stays 240 px wide with its header, tab strip, sections and footer where they are (R1). Sections get longer where R8.4 raises fields to 32 and R5 raises labels; the inspector already scrolls.

## 2. Current state (FACT unless labelled)

- **Panel shell** (`right.rs:11-181`). Version history and comment mode replace the whole panel (`:13-20`, specs J/history and J/comments). Fill `BG`, left hairline `BORDER` (`:22-23`). Header: avatar pill 38×24 with a literal amber `#F59E0B` circle and a white initial (`:28-46`); Present button 24 + chevron 16 (`:49-61`; the chevron is `RIGHT_PRESENT_CHEVRON_W` 16, `theme.rs:335`, below the 24 px pointer floor; 6 px gap before Share); Share 48×24 filled literal `#0D99FF` (or `#1BC47D` in Dev mode) with a white label (`:64-70`; white on `#0D99FF` is 2.99:1); every header button is `RIGHT_BTN_SIZE.min(24.0)` = 24 (`:28`; `RIGHT_BTN_SIZE` itself is 32, `theme.rs:334`, so dropping the `.min` is the whole height change); Present starts at `right − 102` and the avatar ends at x 46, so about 92 px of the header row are free. Tab strip at y 48, height `RIGHT_TABS_H` 24 (`theme.rs:336-337`): "Design | Prototype" or, in Dev mode, "Inspect | Plugins" (`:104-129`), selected fill `BG_SECONDARY`, hover `BG_HOVER`, labels `TEXT`/`TEXT_SECONDARY`. Zoom dropdown 55×24 at x 177..232, y 48..72 inside the 24 px strip (`:132-141`, menu drawn by spec D). Content hairline at `RIGHT_CONTENT_TOP` 81 (`theme.rs:342`, `:143-145`). Footer 36 px with a help circle in literals `#2C2C2C`/`#383838`/`#B3B3B3` (`:161-180`). Dev plugins inspector (`:183-315`, literals `#1BC47D` at `:203`, `:274` and a white label at `:276`). Frame presets inspector when the Frame tool is active with nothing selected (`:317-381`).
- **Design tab** (`properties.rs`). Pane geometry: fields 24 tall (`FIELD_HEIGHT` = `CONTROL_DENSE`, `theme.rs:351`), half fields 88 wide at x 17/113 (`:352-353`), section content 184 wide (`:349`), title rows 40 (`SECTION_TITLE_HEIGHT`, `:347`), sublabels one caption line 14 (`LABEL_BOX_H`, `:370`). Titles `font_body_strong` 11 (`properties.rs:207-229`), sublabels caption 10 (`:300-309`), segmented icon groups (`:314-351`), checkbox (`:353-368`). Sections: page (`:404-468`), styles (`:469-696`) and library styles (`:697-768`) when nothing is selected; selection colours (`:769-822`); selection header, position, rotation, dimensions, resizing, auto layout and grid tracks (`:823-1534`); fill and stroke paint rows (`:1573-1732`); colour picker popup (`:1928-2153`) and on-canvas gradient handles (`:2211-2373`). Literal buttons: "Copy to Slide Deck" `#2C2C2C`/`#383838` (`:871-882`), "Update auto layout" the same (`:1332-1343`). Grey placeholder swatches for image and empty paints `Color32::GRAY`/`from_gray(150)` (`:525`, `:656`, `:725`, `:1551-1552`, `:2473`). Gradient handles on the canvas in white with black-alpha outlines (`:2232-2234`, `:2269-2270`, `:2299`). Document colours are converted by `to_c32` (`:188-191`).
- **Other inspector sections.** `components.rs` (component and instance sections; toggle knob literal white `:86`), `export.rs` (export rows and preview; preview image tint white `:309`), `stroke.rs`, `effects.rs` (`:299` converts a document colour), `typography.rs` (type section and the type-details popover: preview box `#1E1E1E` `:643`, baseline guide `#0D99FF` at 110 alpha `:646`, stylistic-set chips 24 tall with a white label on `BG_SELECTED` when on, `BG_TERTIARY` on hover, `BG_SECONDARY` with `TEXT_SECONDARY` when off, `:769-788`; `:306` converts a document colour), `layoutguide.rs` (`:99` converts), `vertex.rs`, `var_bind.rs` (bind-variable hover button and bound pill), `style_picker.rs` (effect/grid style buttons), `style_edit.rs` + `style_edit_grid_rows.rs` (the "Edit style" popover), `proto_actions.rs` (interaction action list).
- **Prototype tab** (`prototype.rs`). "Show prototype settings" button in literals `#2C2C2C`/`#383838` with a white label (`:58-60`); headings in white (`:87`); help text `#B3B3B3` (`:98`, `:102`); device thumbnails and "Manual" label white when selected (`:705`, `:727`); glyph buttons white when selected (`:848`); `:205` and `:755` convert the document's prototype background colour.
- **Dev Inspect tab** (`inspect.rs:313-807`, drawn when Dev mode is on, `right.rs:153-154`). Header name `font_large_strong` 13 with link/code/more icons that turn white on hover (`inspect.rs:350`, `:358`, `:372`); "Selection details" in `#B3B3B3` (`:441`); MCP section title white (`:415`); the MCP prompt box is no longer drawn here: `inspect.rs:481-482` calls `crate::dev::code_view::prompt_box` (spec G's file), which paints `BG_SECONDARY`, a 1 px `BORDER_STRONG`, `TEXT` mono text and the URI in `INFO` `#0D99FF`; Code Connect title white (`:495`), pill `#164E33`/`#454545`/`#383838` with `#4ADE80`/`#B3B3B3` text (`:500-512`); descriptions `#8C8C8C` (`:534-536`, `:608`); Layer properties title white (`:554`); measurement pill label white on `MEASURE` (`:807`, canvas overlay). `inspect.rs:17-21` converts document colours. The Dev sections inside it (component, code, variables, assets, playground) are `dev/sections.rs` and `dev/playground.rs`, spec G.
- **Colour picker** (`widgets/picker.rs`, opened from paint rows). Handle rings white with black alpha (`:675`, `:679`), grey placeholder `from_gray(0x80)` (`:690`), contrast bar text `#E4E4E7` (`:955`) and contrast badges AAA/AA `#17302A`/`#4CC58B`, AA Large `#332715`/`#F5A524`, Low `#361D21`/`#F3657A` (`:960-966`), eyedropper dot (`:1010-1011`), swatch inner ring white 10 % (`:1202`), slider thumb white with a black-alpha ring (`:1525-1526`). `:158` converts a document colour.
- **New since `205c72ef` (0.7.0).** Section title rows can hold two 24 px actions: the Type section's "Style" button sits at `tb.left() − SPACE_FACT − PANEL_ACTION_BUTTON` = x 181 beside "Type settings" at x 209 (`typography.rs:190-197`, `style_picker.rs:217`), and the linked-style row follows the title; the Styles section title has no "+" (`properties.rs:469-480`); style folders rename inline (`properties.rs:557-610`); effect "+" menu rows turn off at their caps with an "Up to N …" tip (`effects.rs`, `EFFECT_CAPS`); shadow settings gain a Blend select (`effects.rs`, `dropdown_items`); the edit-style popover has a Description row (`style_edit.rs:147-156`). They use the same helpers and hold no new colour literal; the census does not move for them.

Measured colour census (non-test code, `scripts/rust_literal_scan.clean`; definitions as in spec D section 2):

| File | `theme::` lines | Chrome sites | Site lines | Computed sites |
|---|---|---|---|---|
| `panels/right.rs` | 47 | 13 | 38, 39, 66, 68, 173, 175, 176, 203, 274, 276 | none |
| `panels/properties.rs` | 172 | 20 | 525, 656, 725, 875, 877, 1337, 1339, 1551, 1552, 2232, 2233, 2234, 2269, 2270, 2299, 2473, 2490 | 190 |
| `panels/inspect.rs` | 58 | 16 | 350, 358, 372, 415, 441, 495, 500, 502, 504, 512, 534, 536, 554, 608, 807 | 19 |
| `panels/prototype.rs` | 95 | 10 | 58, 59, 60, 87, 98, 102, 705, 727, 848 | 205, 755 |
| `panels/components.rs` | 59 | 1 | 86 | none |
| `panels/export.rs` | 28 | 1 | 309 | none |
| `panels/typography.rs` | 73 | 3 | 643, 646, 788 | 306 |
| `widgets/picker.rs` | 85 | 17 | 675, 679, 690, 955, 960, 962, 964, 966, 1010, 1011, 1202, 1525, 1526 | 158 |
| `panels/effects.rs` | 8 | 0 | | 299 |
| `panels/layoutguide.rs` | 9 | 0 | | 99 |
| `proto_actions.rs`, `stroke.rs`, `vertex.rs`, `var_bind.rs`, `style_picker.rs`, `style_edit.rs`, `style_edit_grid_rows.rs` | 24, 7, 8, 9, 13, 2, 0 | 0 | | none |
| **Total** | 697 | **81** | | 8 (all document-colour converters) |

Command (run from the repo root, or from the root of a `git archive` of the ref; writes nothing). Its output at `8dfaee93` equals the table (exit 0, run 2026-10-28 from the root of `git archive 8dfaee93 crates/designos-app/src scripts/rust_literal_scan.py`).

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

Output at `8dfaee93` (exit 0, pasted from the run, 2026-10-28):

```text
panels/right.rs theme_lines=47 chrome_sites=13 [38, 39, 66, 68, 173, 175, 176, 203, 274, 276] computed=[]
panels/properties.rs theme_lines=172 chrome_sites=20 [525, 656, 725, 875, 877, 1337, 1339, 1551, 1552, 2232, 2233, 2234, 2269, 2270, 2299, 2473, 2490] computed=[190]
panels/inspect.rs theme_lines=58 chrome_sites=16 [350, 358, 372, 415, 441, 495, 500, 502, 504, 512, 534, 536, 554, 608, 807] computed=[19]
panels/prototype.rs theme_lines=95 chrome_sites=10 [58, 59, 60, 87, 98, 102, 705, 727, 848] computed=[205, 755]
panels/proto_actions.rs theme_lines=24 chrome_sites=0 [] computed=[]
panels/components.rs theme_lines=59 chrome_sites=1 [86] computed=[]
panels/export.rs theme_lines=28 chrome_sites=1 [309] computed=[]
panels/stroke.rs theme_lines=7 chrome_sites=0 [] computed=[]
panels/effects.rs theme_lines=8 chrome_sites=0 [] computed=[299]
panels/typography.rs theme_lines=73 chrome_sites=3 [643, 646, 788] computed=[306]
panels/layoutguide.rs theme_lines=9 chrome_sites=0 [] computed=[99]
panels/vertex.rs theme_lines=8 chrome_sites=0 [] computed=[]
panels/var_bind.rs theme_lines=9 chrome_sites=0 [] computed=[]
panels/style_picker.rs theme_lines=13 chrome_sites=0 [] computed=[]
panels/style_edit.rs theme_lines=2 chrome_sites=0 [] computed=[]
panels/style_edit_grid_rows.rs theme_lines=0 chrome_sites=0 [] computed=[]
widgets/picker.rs theme_lines=85 chrome_sites=17 [675, 679, 690, 955, 960, 962, 964, 966, 1010, 1011, 1202, 1525, 1526] computed=[158]
```

The plan's line method (`grep -cE 'Color32::from_rgba?(_unmultiplied|_premultiplied)?\(|Color32::from_gray\(' <file>`, tests included) gives `inspect.rs` 13, `properties.rs` 6, `right.rs` 7, `prototype.rs` 6, `picker.rs` 7, `typography.rs` 3, `effects.rs` 1, `layoutguide.rs` 1. It counts the 8 converters and misses named colours, so this spec uses the census above.

Classification of the 81 chrome sites at `8dfaee93` (per-line counts from the same command with a `Counter`; 56 + 16 + 8 + 1 = 81; at `205c72ef` it was 60 + 16 + 7 + 1 = 84: the 4 prompt-box sites left `inspect.rs` for `dev/code_view.rs`, and `properties.rs:2490` added a placeholder):
- **Chrome colour choices (56)**: become contract constants (section 3).
- **Canvas or content overlays (16)**: gradient handles (`properties.rs:2232`, `:2233`, `:2234`, `:2269`, `:2270`, `:2299` ×2; 7 sites), picker handles and thumbs drawn over the colour field (`picker.rs:675`, `:679`, `:1010`, `:1011`, `:1202`, `:1525`, `:1526`; 7 sites), the measurement pill label (`inspect.rs:807`) and the type-details baseline guide (`typography.rs:646`). They keep their values and move to theme constants (A): white is `CONTENT_HANDLE_FILL` (an alias of the existing `CANVAS_HANDLE_FILL`), and the black and white alphas the sites use are the new names in rule 2 (`CONTENT_HANDLE_RING` black α`4D`, `CONTENT_HANDLE_CASING` black α`66`, `CONTENT_HANDLE_AXIS` white α`B2`, `CONTENT_HANDLE_STOP_IDLE` white α`CC`, `CONTENT_MARK_SHADOW` black α`6E`, `CONTENT_MARK_LIGHT` white α`DC`, `TYPE_PREVIEW_BASELINE` blue α`6E`); the picker's swatch inner ring (`picker.rs:1202`, white α`1A`) is the existing `BORDER_TRANSLUCENT`, and the measurement label (`inspect.rs:807`) the existing `CANVAS_SIZE_BADGE_TEXT`.
- **Placeholder swatches (8)**: the grey shown for image or empty paints (`properties.rs:525`, `:656`, `:725`, `:1551`, `:1552`, `:2473`, `:2490`; `picker.rs:690`). They take one constant (decision 4).
- **Identity image tint (1)**: `export.rs:309` passes `Color32::WHITE` as the tint of `Painter::image`, not a colour choice. It is the only allowlisted site.
- The 8 computed sites convert document colours (content) and stay.

Gates today: no colour-literal gate (spec A rule 13 adds `scripts/check_chrome_colour_literals.py`, new name). `scripts/check_spacing_relationships.py` ratchets geometry and font literals (`properties.rs`, `inspect.rs`, `prototype.rs`, `right.rs`, `picker.rs` all have baseline entries). `scripts/zero_fake_ui_gate.py` Z10 (Share copies a link; Dev Mode's 2nd tab is the plugins inspector), Z11 (`dev-code-connect`, `dev-mcp-btn`, `dev-add-redlines`, `copy-to-slides` do real work) and Z12 (picker image mode) cover this surface. Theme tests pin `RIGHT_TABS_H == CONTROL_DENSE` (`theme.rs:854`) and `LABEL_BOX_H + SPACE_FACT <= CONTROL_DENSE` (`theme.rs:895-898`).

## 3. Behaviour rules

1. **Colours from constants only.** The 56 chrome choices and the 8 placeholders become contract constants, the 16 overlay sites become theme constants, and only `export.rs:309` stays literal (allowlisted identity tint).
2. **Role map** (token-contract constants; widget states are C's, rule 3):

| Element | State | Constant (Untitled role, hex) | Today |
|---|---|---|---|
| Panel fill, footer fill | default | `BG` (bg-primary `#0A0A0A`) | `BG` |
| Panel edge, content hairline, footer hairline, section separators | default | `BORDER` (border-secondary `#262626`), C `Divider/Hairline` | `BORDER` |
| Section titles (Position, Auto layout, Fill, MCP, Code Connect, Layer properties, Prototype headings) | default / dim | C `Section/Header`: `TEXT` `#FAFAFA`, text-sm 14/20 semibold; dim titles `TEXT_TERTIARY` `#A3A3A3` | `TEXT`, `TEXT_SECONDARY`, literal white |
| Sublabels (Position, Rotation, Flow …), "Selection details", group labels | default | `TEXT_TERTIARY` `#A3A3A3`, text-xs 12/18 medium (C `Section/Header` sublabel) | caption 10, `#B3B3B3`, `#8C8C8C` |
| Help and description text (Prototype help, Code Connect description) | default | `TEXT_TERTIARY` `#A3A3A3` (C-1.7) | `#B3B3B3`, `#8C8C8C` |
| Field values, numbers, hex codes | default | `TEXT`; numbers stay Roboto Mono (P5, D6) at text-xs 12 | `TEXT`, 11 |
| Property fields (number, text, hex, opacity) | default, hover, focused, disabled | C `Input/PropertyField` xs 32 (R8.4): fill `BG`, border `BORDER_STRONG` `#404040` 1 px (E-2: field and select borders join it, plan.md 22:20), value `TEXT`, prefix letter or icon `TEXT_TERTIARY`; focused: border 2 px `BORDER_SELECTED` `#FB923C` and **no ring** (C S4: text fields and selects draw none); disabled at 50 % | `BG_SECONDARY` fills, no border |
| "Mixed" value in a field or select | mixed selection | `TEXT_TERTIARY` `#A3A3A3` (a kit role: 7.85:1 on `BG`, 7.11:1 on `BG_SECONDARY`, C-1.7 and C-1.8; plan.md C-R). Not `TEXT_PLACEHOLDER`: E-1 stays placeholder-only. Every form uses it: number fields through the hint path (`widgets/field.rs:20`, `:558`, C's file), the type section (`typography.rs:337`, today `TEXT`), the select labels (`prototype.rs:313`, `:342`, `vertex.rs:74`, `:97`, `export.rs:290`) and the shadow Blend select (`effects.rs`, 0.7.0) | `TEXT_SECONDARY` |
| Selects (resizing, blend, font, effect type, variable modes) | default, open | C `Select/Trigger` xs 32: as `Input/PropertyField` (1 px `BORDER_STRONG`, E-2), chevron `ICON_QUATERNARY`; open uses the focused look (2 px `BORDER_SELECTED`, no ring) | `BG_SECONDARY` |
| Segmented icon groups (alignment, flow, text align) | default, hover, selected | C `Tabs/Segmented` at the surface's height (R10.1, as C says): track 24 tall, pad `INSET_SEGMENT` 2, radius `RADIUS_CONTROL` 8, fill `BG_SECONDARY` `#171717`; tiles 20 tall at `RADIUS_SEGMENT` 6 inside slots that are the hit areas (24 tall, 29 wide for the 3-item groups as `SEGMENT_W`, 24 wide for the 4-item flow group); glyph `ICON_QUATERNARY` → `ICON_QUATERNARY_HOVER` on hover; selected tile `BG_SELECTED` `#262626` (R8.2, contract row 18's rule) with glyph `ICON_SECONDARY` `#D4D4D4`, no border. The group sits centred in its 32 px field row (4 px above and below). The 1 px `BG` gaps between segments go (the tiles are separated by the pad) | `BG_SECONDARY`, `BG_SELECTED`, white |
| Secondary buttons (Copy to Slide Deck, Update auto layout, Show prototype settings, + Add redlines, Copy MCP link, Send to Agent Terminal) | default, hover, pressed, focused | C `Button/Secondary` xs 32: fill `BG`, border `BORDER_STRONG`, label `TEXT_SECONDARY` text-sm 14 Semibold (R8.6); hover `BG_HOVER` with `TEXT_SECONDARY_HOVER`; pressed `BG_PRESSED` | `#2C2C2C`/`#383838` literals, white label |
| Share | default, hover, pressed, focused | C `Button/Primary` xs 32: `BG_BRAND` `#E36323` → `BG_BRAND_HOVER` `#F97316` → `BG_BRAND_PRESSED` `#EA580C`, label `TEXT_ON_BRAND` `#0A0A0A` text-sm 14 Semibold (R8.6; C-1.14–C-1.16), in both modes (decision 1) | `#0D99FF` / `#1BC47D` + white |
| Present, Present chevron, zoom, avatar pill | default, hover | C `Button/Tertiary` xs (Present 32×32 icon-only, avatar pill 38×32); the chevron's visible cell stays 16 wide (`RIGHT_PRESENT_CHEVRON_W`, frozen) and its hit box is 24 wide; zoom is C `Select/Trigger` borderless and **stays 24 tall inside the 24 px tab strip** (R10.1 logic; C's xs 32 is not applied, sent to C) | `button_bg`, `BG_HOVER` |
| Avatar circle | default | fill `AVATAR_COLORS[avatar_index(name)]` (identity colours, contract row 79, picked from the user's name exactly as comments do, `comments/ui.rs:9-11`; plan.md C-R: the soul spends one orange, on the primary action and live state) with the initial in `TEXT_ON_BRAND` (5.40:1 on the worst of the six fills, 5.60 to 10.12 on the others) | `#F59E0B` + white |
| Design/Prototype (Inspect/Plugins) tabs | default, hover, selected | C `Tabs/Pill` (C cites `right.rs:103-127`): label `TEXT_QUATERNARY`; hover `BG_HOVER` with `TEXT_SECONDARY`; selected `BG_SELECTED` with `TEXT_SECONDARY`; height 24 (R10.1) | `BG_SECONDARY`, `TEXT_SECONDARY` |
| Checkbox (Clip content, …) | off, on, focused | C `Checkbox/Box`: off border `BORDER_STRONG` `#404040` 1 px (R10.7, contract named exception E-2); on `BG_BRAND` with the check in `TEXT_ON_BRAND` (C-1.14); focused `FOCUS_RING` | `BG_BRAND` + white check |
| Toggle (component boolean props, `components.rs:69-88`) | off, on | C `Toggle/Switch`: track 36×20 (was 28×16, `components.rs:72`), radius full, pad 2, knob 16 `ICON_WHITE` (N17) (was radius 6, `:86`); track `BG_TERTIARY` `#262626` off, `BG_BRAND` on; hit box stays the 88 px column cell (`pane.col(1)`, ≥ `TARGET_MIN`) | white knob 12 |
| Stylistic-set chips in type details (`typography.rs:769-788`) | off, hover, on | C `Tag/Chip` selectable: off `BG_SECONDARY` + `TEXT_SECONDARY`; hover `BG_SECONDARY_HOVER` (S6 on a bg-secondary chip); on `BG_SELECTED` + `TEXT` (C-1.3) | `BG_SECONDARY`, `BG_TERTIARY`, `BG_SELECTED` + white |
| Help button in the footer | default, hover | C `Button/Icon` xs 32 inside the 36 px footer: glyph `ICON_QUATERNARY` → `ICON_QUATERNARY_HOVER`, hover `BG_HOVER` | `#2C2C2C`/`#383838`/`#B3B3B3` |
| Link-like text (MCP URI) | default | `TEXT_BRAND` (fg-brand-secondary `#F97316`, R8.3; C-1.11 on the prompt box). The prompt box is now spec G's `dev/code_view.rs::prompt_box`, which paints the link in `INFO`: G changes it to `TEXT_BRAND` | `INFO` `#0D99FF` |
| Prompt and code boxes (type-details preview here; the MCP prompt box is G's) | default | fill `BG_SECONDARY` `#171717` + `BORDER`; text `TEXT_SECONDARY` `#D4D4D4`, mono. The MCP prompt box moved to `dev/code_view.rs::prompt_box` (spec G): it already uses `BG_SECONDARY` and `TEXT`; G sets its border to `BORDER` and its link to `TEXT_BRAND` | `#1E1E1E`, `#383838`, `#D4D4D4` |
| Code Connect status pill | connected / not started / hover | status badges (contract G-1): connected `BG_SUCCESS_PRIMARY` `#052E16` + border `UTILITY_GREEN_200` + text `UTILITY_GREEN_700` `#86EFAC` (C-1.21); not started a gray status badge (the same `UTILITY_NEUTRAL_50`/`_200`/`_700` constants as C's count badge, contract G-1 anatomy; it is not a count, so not `Badge/StatusCount`; C-1.24); hover per C | `#164E33`/`#4ADE80`, `#383838`/`#B3B3B3`, `#454545` |
| Picker contrast badges AAA, AA / AA Large / Low | default | status badges (G-1): success `BG_SUCCESS_PRIMARY` + `UTILITY_GREEN_200` + `UTILITY_GREEN_700`; warning `BG_WARNING_PRIMARY` + `UTILITY_YELLOW_200` + `UTILITY_YELLOW_700`; error `BG_ERROR_PRIMARY` + `UTILITY_RED_200` + `UTILITY_RED_700` (C-1.21–C-1.23); ratio text `TEXT` | `#17302A`/`#4CC58B`, `#332715`/`#F5A524`, `#361D21`/`#F3657A`, `#E4E4E7` |
| Type-details baseline guide | default | a content-overlay constant (it draws a guide over a type preview): `TYPE_PREVIEW_BASELINE` (new name, `rgba(0x0D99FF, 0x6E)`: blue `#0D99FF` at 110 alpha, unchanged), moved into `theme.rs` | literal |
| Placeholder swatch (image or empty paint) | default | `SWATCH_PLACEHOLDER` (new name, value bg-quaternary `#404040`, decision 4) | `GRAY`, `from_gray(150)`, `from_gray(0x80)` |
| Gradient handles, picker handles, slider thumbs, measurement pill label | default | content-overlay constants in `theme.rs`, values unchanged: white `CONTENT_HANDLE_FILL` (alias of `CANVAS_HANDLE_FILL`); `CONTENT_HANDLE_RING` black α`4D` (`picker.rs:675`, `:1525`, `properties.rs:2269`); `CONTENT_HANDLE_CASING` black α`66` (`properties.rs:2232`); `CONTENT_HANDLE_AXIS` white α`B2` (`:2234`); `CONTENT_HANDLE_STOP_IDLE` white α`CC` (`:2299`); `CONTENT_MARK_SHADOW` black α`6E` and `CONTENT_MARK_LIGHT` white α`DC` (the threshold dot, `picker.rs:1010-1011`); the measurement label is the existing `CANVAS_SIZE_BADGE_TEXT`. The picker's track handles (hue, alpha, opacity) keep the **white fill and dark ring**: C's orange `Slider/Handle` is for range sliders only, and an orange handle would vanish on the orange end of the hue bar (plan.md C-R) | literals |
| Dev plugins row icon and "Manage & Run Plugins…" button (`right.rs:203`, `:274-276`) | default, hover | `DEV_READY` `#14AE5C` (R10.4; G's Dev accent role), label `TEXT_ON_BRAND` (C-1.35); "Ready" text in `DEV_READY` on `BG` (C-1.36) | `#1BC47D` + white (2.27:1) |
| Icon buttons in the Inspect header (link, code, more) | default, hover | C `Button/Icon`: `ICON_QUATERNARY` → `ICON_QUATERNARY_HOVER`; never pure white | `ICON_SECONDARY` → white |
| Section and row action buttons (section "+"/"−", Style, Type settings, name actions, `al-space`, eye, minus) | default, hover, pressed | C `Button/Icon`, **kept at `PANEL_ACTION_BUTTON` 24** (≥ `TARGET_MIN`) at `SECTION_TRAILING_X` 209 and the 28 px pitch of `RIGHT_NAME_ACTION_X` [153, 181, 209] (`properties.rs:218`, `:1606-1608`): 209 + 32 = 241 would overflow the 240 px panel (B rule 6), so C's xs 32 is not applied; the deviation is sent to C. In a 32 px field row they are 24×24 centred (`al-space`, `properties.rs:1316`, was 24 × `FIELD_HEIGHT`) | `button_bg` 24 |
| Component and instance names, icons, property chips | any | Studio component cyan stays (controller default): `TEXT_COMPONENT`, `COMPONENT` | same |
| Document colours (fills, strokes, effects, page background, prototype background) | n/a | content: drawn from the document, unchanged | `to_c32`, `c32` converters |

3. **Widgets (spec C).** Number, text, hex and opacity fields: `Input/PropertyField` xs 32 (R8.4; prefix letter or icon, suffix unit). Dropdowns (resizing, blend, font, effect type, variable modes, zoom): `Select/Trigger`, opening C's `Menu/Panel`. Segmented icon groups: `Tabs/Segmented`; the tab strip: `Tabs/Pill`. Checkbox: `Checkbox/Box`; toggle: `Toggle/Switch`; stylistic sets: `Tag/Chip`. Section headers with "+"/"−" actions: `Section/Header` with `Button/Icon` actions that stay 24 (rule 2, last row). Buttons: `Button/Primary`, `Button/Secondary`, `Button/Tertiary` at xs 32; `Button/Icon` at xs 32 only where a 32 px box fits (the header, the footer help button), and at 24 for the inspector's trailing and name actions. Swatches: `Swatch/Color`. Popovers (stroke settings, type details, effect settings, edit style, picker): `Popover/Panel` (radius 16, R8.5; rows 32). Status pills and contrast badges: G-1 status badges; counts: `Badge/StatusCount`. Every hover tip: `Tooltip/Default` (fill `BG_PRIMARY_SOLID`, text `TEXT`, C-1.19). Scrollbar: C's (`panels/mod.rs`). Text fields and selects draw no focus ring (C S4): their focused border is 2 px `BORDER_SELECTED`.
4. **States.** Fields: Default, Hover, Focused, Disabled, Mixed (today's "Mixed" text stays) and Bound-to-variable (var_bind pill; C `Tag/Chip`). Buttons and tabs: C's Default, Hover, Pressed, Focused, Disabled, Selected. No state is removed; the Pressed state comes with C's widgets.
5. **Sizes (R5, R8.4, R8.6, R10.1) and their layout effect.** Panel width 240, the header, tab strip, section order, field columns (x 17/113, 88 wide) and the 36 px footer keep their places (R1).

| Element | Constant / code | Today | Target | Δ | Layout effect |
|---|---|---|---|---|---|
| Header buttons (avatar, Present + chevron, Share) | `right.rs:28` (`RIGHT_BTN_SIZE.min(24)`; `RIGHT_BTN_SIZE` is 32), `RIGHT_PRESENT_CHEVRON_W` 16 and `RIGHT_PRESENT_FROM_RIGHT` 56 stay frozen (contract appendix) | 24 tall; Present 24, chevron 16, Share 48 wide | button xs 32 tall (`RIGHT_BTN_SIZE`, no `.min`); Present 32; chevron cell 16 visible, hit box 24; Share 60 (C `Button/Primary` xs hug: 10 + "Share" 39.2 → 40 at 14 Semibold + 10, R11) | +8 height, Share +12 | the positions are derived in `right.rs` from Share's measured width, not from the frozen constants: Share's right edge stays at `right − INSET_CONTROL` (8), so Share is x 172..232; gap `INSET_CONTROL` 8; chevron cell x 148..164, its hit box 148..172 (the extra 8 px are the gap, so it abuts Share and overlaps nothing); Present x 116..148 abuts the chevron; avatar pill x 8..46 unchanged. Free header space between the avatar and Present: 92 → 70 px. They occupy y 8..40; the tab strip stays at y 48 (8 px clear) |
| Zoom control | `right.rs:132` | 55×24 | 55×24 (stays inside the 24 px strip, R10.1) | 0 | none; sent to C (its `Select/Trigger` xs is 32) |
| Segmented icon groups | `properties.rs:319-351` (`seg_w` 22 for 4 items, 29 for 3) | tiles = hit areas 22 / 29 × 24 | track 24, tiles 20, hit slots 24 tall; 3-item groups keep 87 px (slots 29); the 4-item flow group `al-dir` (`properties.rs:1220`) grows 88 → 96 (slots 24, so every hit area is ≥ `TARGET_MIN`) | +8 on `al-dir` | `al-dir` spans x 17..113 and the Gap field column starts at x 113 (`FIELD_COL_X[1]`), so the 8 px gap between them goes to 0; the columns are frozen (R1), so the group grows into the gap rather than moving the field. Flagged to the controller in the report. 4 px above and below the 24 px group in its 32 px row |
| Swatch hit areas | page swatch `properties.rs:423-424` (12 + 2 × 2 = 16), selection colours `:795`, `:800` (16×16), paint rows `:1602`, `:1646` (16 + 2 × 2 = 20) | 16 / 16 / 20 | 24×24 each, centred on the visible swatch (16 and 12 px visuals unchanged) | 0 painted | page swatch: x g.left..g.left + 24, exactly up to the hex field at g.left + `PAGE_HEX_X` 24; selection colours: x 13..37 (swatch 17..33, hex field from 41); paint rows: x 17..41 (group left edge to the hex field at g.left + 24); all fit the 32 px rows vertically |
| Tab strip | `theme.rs:336-337` | 24 at y 48 | 24 (R10.1: R1 wins over R5) | 0 | none; `RIGHT_TABS_H == CONTROL_DENSE` (`theme.rs:854`) stays |
| Section title rows | `theme.rs:347` | 40 | 40 | 0 | title type 11 → 14 |
| Field rows | `theme.rs:351`, pitch 24 + 8 | 24 | `CONTROL_XS` 32 (R8.4) | +8 per row | for "Welcome to Untitled UI" (a 1600×6208 auto-layout frame, capture `D-07`) the visible inspector holds about 19 field rows and 2 buttons: about +152 px of rows and +16 px of buttons (INFERENCE: counted on the capture); the inspector scrolls. The Toggle track grows 28×16 → 36×20 inside the same 88 px cell |
| Sublabels | `LABEL_BOX_H` 14 (`theme.rs:370`) | 14 | text-xs 12/18 → 18 | +4 per labelled group | 8 labelled groups on `D-07`: +32 px; B updates `line_heights_fit_their_roles` (18 + `SPACE_FACT` 4 = 22 ≤ 24 still holds for a 24 slot, and fits a 32 slot) |
| Values and labels | `font_body` 11, `font_mono` 11 | 11 | text-xs 12 | 0 | values elide earlier in 88 px half fields (variable-mode names "Auto (Sty…" already elide) |
| Button labels (Share, secondary buttons) | `font_body_strong` 11 | 11 | text-sm 14 Semibold (R8.6) | 0 | labels fit the 184 px content width; Share widens (row 1) |
| Style rows | `STYLE_ROW_H` 32 (`theme.rs:376`) | 32 | row 32 | 0 | none |
| Footer | `right.rs:144` | 36, help 28 | 36, `Button/Icon` xs 32 | 0 | none (2 px above and below the button) |
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
| `panels/properties.rs` | F | the page-level Styles section (`:469-768`) overlaps K's "Styles" scope: F owns it because it is inspector content; K lists this file as "not mine" |
| `panels/inspect.rs` | F | Dev mode's Inspect panel: F owns the file; the Dev sections it calls (`dev/sections.rs`, `dev/playground.rs`), the code and MCP prompt boxes it now delegates to `dev/code_view.rs` (`code_block`, `prompt_box`) and the syntax colours are G's |
| `panels/prototype.rs`, `panels/proto_actions.rs` | F | none |
| `panels/components.rs`, `panels/export.rs`, `panels/stroke.rs`, `panels/effects.rs`, `panels/typography.rs`, `panels/layoutguide.rs`, `panels/vertex.rs`, `panels/var_bind.rs` | F | `var_bind.rs` is the inspector side of variables (its `//!` doc); the Variables panel itself is K's |
| `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` | F | shared with K ("Styles"); F owns them because they open from inspector rows; K lists them as "not mine" |
| `widgets/picker.rs` | F | the only `widgets/` file not owned by C (assigned to F by the plan brief) |

Not mine: `theme.rs`, `design/*` (A, B); `widgets/field.rs`, `widgets/menu.rs`, `widgets/mod.rs`, `widgets/popover.rs`, `widgets/field_tab.rs`, `widgets/popup_open.rs`, `panels/mod.rs` (C); `tabs.rs`, `shell.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `dev/*`, `specs.rs` (G); `panels/context.rs`, `history/*`, `comments/*` (J); `panels/assets.rs`, `variables/*`, `libraries/*` (K). `panels/check_designs.rs` (K, plan.md 22:20); `dev/code_view.rs` (G).

Also F's, for colour-literal and constant re-points only (controller, plan.md H–K check rulings, 2026-10-29 01:05): `canvas/proto.rs`, the prototype-flow overlay (4 chrome sites); no layout change (R1).

**Colour-literal gate.** Spec A rule 13 adds `scripts/check_chrome_colour_literals.py` (new name). F's allowlist: `export.rs:309` (identity image tint). Until the gate exists, the section 2 command must print `chrome_sites=0` for every owned file except `panels/export.rs` (1).

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
2. Mixed selection: "Mixed" uses `TEXT_TERTIARY` `#A3A3A3` in every form (rule 2): it is state the user must read, not a hint, so it passes AA (7.85:1 on `BG`, 7.11:1 on `BG_SECONDARY`, 6.00:1 on `BG_SELECTED`) and does not use the placeholder exception E-1 (plan.md C-R).
3. A field bound to a variable shows C's bound `Tag/Chip`; the hover "bind" button keeps its hit area inside the field.
4. Dev mode: Share stays brand orange (decision 1); the Plugins row and its button take `DEV_READY` with a dark label (R10.4).
5. The colour picker over a very light or very dark document colour: handles keep a white fill with a black-alpha outline so they stay visible on any content.
6. Popovers taller after the 32 px rows (picker, edit style): they clamp to the window and scroll as C's `Popover/Panel` does today.
7. The minimum window (900×600): the inspector scrolls; header, tab strip and footer stay fixed.
8. No selection: page and styles sections; Frame tool active with no selection: frame presets list. Both restyle with the same constants.
9. Unchecked checkboxes and field borders: the kit's `BORDER_STRONG` `#404040` is 1.91:1 on `BG`. Checkbox, property-field and select borders are all covered by E-2 (R10.7 for checkboxes; plan.md 22:20 for text fields and selects), so nothing is sent to A: the contract's E-2 row is A's to update.
10. The 4-item flow group meets the Gap field (rule 5, segmented row): the group's last slot ends at x 113 and the field's border starts at x 113, so the two rounded shapes touch tangentially; no hit area overlaps. This is below the soul's "a group at least twice its own inner gap from its neighbours" (inner gap 2, so 4 px). The three ways out each break something: keep 22 px slots (the hit area is below `TARGET_MIN`, the soul's pointer floor), narrow or move the Gap field (R1: columns and positions stay), or accept the 0 px gap (this spec's choice). The controller may choose another.
11. A name action or section button next to a 32 px field: the 24 px button is centred in the 32 px row (4 px above and below); its hit box is the 24 px square.

## 9. Non-goals

- Inspector layout (section order, column x positions, panel width), new sections or new controls.
- The Dev sections' content and syntax highlighting (G), history and comments panels (J), the Variables panel and libraries (K), menus drawn by D.
- Canvas overlay colours (they move into constants with the same values), document content.

## 10. Test plan (red first)

Acceptance criteria:

1. The section 2 command (or A's gate) prints `chrome_sites=0` for every owned file except `panels/export.rs` (`chrome_sites=1`, line 308, allowlisted).
2. `python3 scripts/check_spacing_relationships.py` passes; per-file counts stay at or below `scripts/spacing_baseline.json` (radius literals fall).
3. Spec L's layout gate shows the same regions, panel width, column positions and control order as the baseline, apart from the entries of the "L allow entries" table in section 10: the header (avatar +8 tall; Present 24 → 32 wide and tall and 22 px left; the chevron cell 24 → 32 tall (its 24 px hit box is a new recorded name); Share 48 → 60 wide, left edge 12 px left), the field rows (+8), the `al-dir` group (+8 wide), the swatch hit areas, the footer button and the Toggle.
4. Every constant in section 3 is a token-contract constant or a new name A has added; `theme::` tests pass with B's updated invariants (`LABEL_BOX_H`, `FIELD_HEIGHT`; `RIGHT_TABS_H` stays 24 by R10.1).
5. `python3 scripts/zero_fake_ui_gate.py` (Z10, Z11, Z12) and `python3 scripts/design_os_self_evolve.py` pass.
6. **Contrast.** `ui ds a11y` is not accepted: on this store it measured 0 pairs (false green, plan.md and contract (f)). The check is three parts:
   - (a) The contract's C-1 check (token-contract (f)) exits 0. Run 2026-10-28 from the specs folder against the contract as on disk on 2026-10-28 (23:19) and a scratch `theme.rs` built from `8dfaee93` with the contract's rows applied (C-1.1 to C-1.62 minus the retired C-1.25 and C-1.26): exit 0, "every C-1 row (62/62 on 2026-10-28; rerun against the final contract) pass, 4/4 exception floors held, 0 failure(s)". F relies on C-1.1, C-1.3, C-1.5, C-1.7, C-1.11, C-1.14–C-1.16 (Share), C-1.19 (tooltips), C-1.21–C-1.24 (status and count badges), C-1.7 and C-1.8 ("Mixed" in `TEXT_TERTIARY`, plan.md C-R), C-1.28 (segmented glyphs on the track), C-1.29 (focus ring), C-1.30 (focused field outline; on a `BG` field the pair is C-1.37's) and C-1.35–C-1.36 (Dev accent, R10.4). Named exceptions E-1 (placeholder, floors E-1.a/b) and E-2 (checkbox, field and select borders, floors E-2.a/b) are rulings, not passes.
   - (b) The F-P rows below, in the C-1 row shape, pass the contract's snippet with only the id prefix and the file changed. Run 2026-10-28 from the specs folder: exit 0, 15 rows ok (lowest: F-P.9 3.46:1). F-P.13 to F-P.15 are the avatar initial and the "Mixed" pairs.
   - (c) A adds the F-P rows to C-1 and to the `chrome_contrast_pairs` test (spec A section 10). Until then F's PR runs (b).
7. Before/after captures of every command below are in the PR with exit codes.
8. The Present chevron's hit box, every segment of the 4-item flow group and every swatch hit area (`sw-hit:*`, `selsw-hit:*`, `page-swatch-hit`) are at least `TARGET_MIN` on both axes (red tests below).

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
| F-P.13 | `TEXT_ON_BRAND` #0A0A0A | `AVATAR_COLORS_0` #F24822 | 4.5 | header avatar initial on `AVATAR_COLORS[0]` (an array element, not a constant: a composite row like E's `*_ON_BG`; A names it when it copies the row); the worst of the six identity fills, the others 5.60 to 10.12 |
| F-P.14 | `TEXT_TERTIARY` #A3A3A3 | `BG` #0A0A0A | 4.5 | "Mixed" value (= C-1.7) |
| F-P.15 | `TEXT_TERTIARY` #A3A3A3 | `BG_SECONDARY` #171717 | 4.5 | "Mixed" in a field inside a card (= C-1.8) |

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
| `theme::tests` `RIGHT_TABS_H == CONTROL_DENSE` (`theme.rs:854`) | pass unchanged (R10.1) | guard |
| new `panels::right::tests::share_is_the_brand_primary_with_a_dark_label` (new name) | Share fill is `BG_BRAND` and its label is `TEXT_ON_BRAND` in both modes | yes |
| new `panels::right::tests::present_chevron_hit_box_meets_the_target_floor` (new name) | the recorded `present-chevron-hit` rect is at least `TARGET_MIN` wide and `RIGHT_BTN_SIZE` tall, starts at the right edge of `present-button`, ends at or before the left edge of `share-button`, and overlaps neither, while `present-chevron` stays 16 wide; `RIGHT_PRESENT_CHEVRON_W` and `RIGHT_PRESENT_FROM_RIGHT` still equal 16 and 56 | yes (16) |
| new `panels::right::tests::header_controls_are_xs_tall_and_do_not_overlap` (new name) | avatar, Present, chevron and Share are `RIGHT_BTN_SIZE` tall at y 8..40; Share ends 8 px from the right edge; the tab strip starts at y 48; the zoom control is 24 tall | yes (24) |
| new `panels::right::tests::avatar_uses_identity_colours_and_a_dark_initial` (new name) | the avatar disc is one of `AVATAR_COLORS` picked by the user's name and its initial is `TEXT_ON_BRAND` | yes (`#F59E0B`, white) |
| new `panels::properties::tests::segmented_hit_areas_meet_the_target_floor` (new name) | every recorded `seg` rect of a 3-item and the 4-item group is at least `TARGET_MIN` on both axes; the 4-item group is 96 wide and does not overlap the Gap field | yes (22 wide) |
| new `panels::properties::tests::swatch_hit_areas_meet_the_target_floor` (new name) | the recorded `page-swatch-hit`, `selsw-hit:*` and `sw-hit:*` rects are at least 24×24, share their centres with the visual swatches and overlap no hex field | yes (16 and 20) |
| new `panels::properties::tests::mixed_values_use_text_tertiary` (new name) | a mixed X field, a mixed font size and a mixed shadow blend paint "Mixed" in `TEXT_TERTIARY` | yes (`TEXT_SECONDARY`, `TEXT`) |
| new `widgets::picker::tests::track_handles_keep_a_white_fill_and_a_dark_ring` (new name) | the hue and alpha handles paint `CONTENT_HANDLE_FILL` with a `CONTENT_HANDLE_RING` outline, not `ICON_BRAND_PRIMARY` | guard |
| new `widgets::picker::tests::contrast_badges_use_status_roles` (new name) | the AAA/AA/AA Large/Low badges paint the G-1 success, warning and error constants | yes |
| new `panels::properties::tests::field_rows_use_the_field_height` (new name) | a selected frame's X/Y row is `FIELD_HEIGHT` tall and `FIELD_HEIGHT` equals `CONTROL_XS` (32, R8.4) | yes (24) |

Run: `cargo test -p designos-app --bin designos panels::right`, `cargo test -p designos-app --bin designos panels::properties`, `cargo test -p designos-app --bin designos panels::inspect`, `cargo test -p designos-app --bin designos panels::prototype`, `cargo test -p designos-app --bin designos panels::typography`, `cargo test -p designos-app --bin designos widgets::picker`, `cargo test -p designos-app --bin designos theme::`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**L allow entries F needs** (L's schema; `spec: "F"`; the header row is right-anchored, so L's derived cumulative shift carries the widths):

| Target | Entry | Arithmetic |
|---|---|---|
| `share-button`, `right-top-share` | `anchor_x: right`, `max_dw: 12`, `max_dh: 8` | 48 → 60 wide, 24 → 32 tall, right edge fixed at 232 |
| `present-button` | `anchor_x: right`, `dx: -2`, `max_dw: 8`, `max_dh: 8` | right edge 162 → 148 = −12 (Share's growth, derived; the chevron cell's width does not change) − 2 (the gap 6 → 8); 24 → 32 |
| `present-chevron` (stays the visual cell) | `anchor_x: right`, `dx: -2`, `max_dh: 8` | 162..178 → 148..164: right edge −14 = −12 derived − 2 (the gap); width stays 16, 24 → 32 tall. The 24 px hit box is the new name `present-chevron-hit` (148..172), reported, not baselined |
| `right-top-avatar` | `anchor_y: top`, `max_dh: 8` | 24 → 32, x unchanged |
| Field rows (unnamed) | `h_base: 24`, `region: right_panel`, `max_dh: 8` | +8 per row; the derived shift covers everything below |
| Segmented slots (unnamed) | `h_base: 24`, `max_dw: 2`; and for the field at x 113 right of `al-dir` (the Gap field) `dx: -8` | 4-item group slots 22 → 24 wide (3-item slots stay 29); the group's +8 would push the Gap field by L's derived shift, and it does not move |
| Swatch hit rects | none: `sw:*` and `selsw:*` stay the visual 16 px rects; the 24×24 hit areas are new recorded names (`sw-hit:*`, `selsw-hit:*`, `page-swatch-hit`), reported, not baselined | 16 / 20 → 24 around the same centre |
| Footer help (unnamed) | `h_base: 28`, `max_dw: 4`, `max_dh: 4` | 28 → 32 |
| Toggle (`toggle:*`) | none | the recorded rect is the 88 px cell, unchanged; the 28×16 track is painted, not recorded |

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

No capture was rerun at `8dfaee93`; the recorded runs are from 0.6.x builds. v0.7.0 changed `--select` (it takes `id:`, `fig:`, `name:` or a layer name and exits 2 on a miss) and the screenshot exit paths, so the implementer reruns the list on v0.7.0 (`tools/designos-latest`, an idle machine) and records the exit codes in the PR. New in 0.7.0: `--panel export-toast`, `fill-picker-libraries[:hover]` and `effect-styles`/`grid-styles` presets cover the new inspector surfaces.

Not capturable from the CLI today: hover, focus, the Mixed state, a bound field (needs a document with variables bound to the selection), the Dev Plugins tab (no `--tab` value for it in Dev mode). The PR covers them with the new paint tests and a manual window capture, labelled manual.

## 11. Performance budgets

- Inspector paint stays one pass per frame with no new allocation per field; constants are `const`.
- Taller rows add no layout passes: the pane advances `y` by the new constants.
- No `request_repaint` added; idle CPU stays 0 % (CLAUDE.md section 4). The picker and gradient handles keep their current repaint behaviour.

## 12. Dependencies and build order

- A, B, C first; L's baselines before the first PR. Build order: constants in `right.rs` (header, tabs, footer), `properties.rs` sections and shared helpers (`title`, `sublabel`, `segmented`, `checkbox`), the other section files, `prototype.rs`, `inspect.rs`, `widgets/picker.rs` and the popovers; then R5/R8.4 sizes; then evidence.
- **G.** The Dev accent role; its value is `DEV_READY` `#14AE5C` (R10.4), so the Dev-mode Plugins items no longer wait on G.
- **Sent to A (new names):** `SWATCH_PLACEHOLDER` (`#404040`), `TYPE_PREVIEW_BASELINE` (`rgba(0x0D99FF, 0x6E)`), `CONTENT_HANDLE_FILL` (alias of `CANVAS_HANDLE_FILL`, `#FFFFFF`), `CONTENT_HANDLE_RING` (`rgba(0x000000, 0x4D)`), `CONTENT_HANDLE_CASING` (`rgba(0x000000, 0x66)`), `CONTENT_HANDLE_AXIS` (`rgba(0xFFFFFF, 0xB2)`), `CONTENT_HANDLE_STOP_IDLE` (`rgba(0xFFFFFF, 0xCC)`), `CONTENT_MARK_SHADOW` (`rgba(0x000000, 0x6E)`), `CONTENT_MARK_LIGHT` (`rgba(0xFFFFFF, 0xDC)`); the F-P rows for C-1 and `chrome_contrast_pairs`; the contract's E-2 row (plan.md 22:20) must name field and select borders. `RIGHT_PRESENT_CHEVRON_W` and `RIGHT_PRESENT_FROM_RIGHT` keep their values (contract appendix) and are not edited; right.rs derives the header positions (rule 5). `RIGHT_PRESENT_FROM_RIGHT` then loses its only two readers (`right.rs:49`, `:64`), so the bin target would warn `dead_code` and `clippy -D warnings` would fail: A allows it with `#[allow(dead_code)]` (A rule 21's staged pattern) or deletes it (the contract freezes the value, not the name); a test that reads it does not count. `RIGHT_PRESENT_CHEVRON_W` is still read (the visible 16 px cell).
- **Sent to C:** `Tabs/Pill` at 24 in the inspector strip (R10.1; C derives xs 32); `Tabs/Segmented` Selected tile without the 1 px `#404040` border (the fill `BG_SELECTED` already matches, contract row 18 and R8.2); the inspector's groups are 24 tall with 20 px tiles in 24 px slots (R10.1, as C says) and the zoom `Select/Trigger` stays 24; `Button/Icon` stays 24 at x 209 and on the 28 px name-action pitch (xs 32 does not fit); `Slider/Handle` (orange) is for range sliders only: the picker's track handles keep a white fill and a dark ring, so C's retirement of `widgets/picker.rs:658-683` excludes them; `Toggle/Switch` 36×20 applies to `components.rs`.
- **Open PRs touching F files** (`gh pr list --state open --json files`, 2026-10-28): #1113 `fix/issue-1026-typography-style-link` (`panels/typography.rs`); #1156 `fix/issue-1152-export-error-path` (`panels/export.rs`); #1187 `pm/spec-v0.6` and #1177 `release/v0.6.2` (`panels/export.rs`, `inspect.rs`, `properties.rs`, `style_picker.rs`, `dev/*`: stale against the merged 0.7.0); stale #770, #768, #765. #1112 is merged. Rebase on #1113 and #1156 if either merges first.
- #1106 and #1108 touch no F file. `dev/code_view.rs` (G) now holds what `inspect.rs` used to paint for the MCP prompt box: G applies F's prompt-box rules there (rule 2, section 5).

## 13. Owner rulings that apply

- R1 (layout unchanged), R2 (Untitled spacing and type), R3 (no Figma), R4 (Studio orange, dark label `#0A0A0A` 5.73:1, focus ring Orange-500 `#F97316`, Purple Ban), R5 (text-xs 12/18 rows and labels, text-sm 14/20 titles, button xs 32, row 32, radius-md 8), R6 (keep icons), R7 (spec-driven).
- R8.1 placeholder `#737373` (no longer used for "Mixed": plan.md C-R puts it on `TEXT_TERTIARY`); R8.2 selected `#262626`; R8.3 accent text `#F97316` (MCP URI); R8.4 inspector field 32 (decides open question 2); R8.5 popovers 16 with rows 8 at inset 8; R8.6 button labels 14 Semibold, tab and row text 12. R8.7 does not apply (no destructive button in F's files). R9: figma-agent read-only is allowed; not used here.
- R10.1 the inspector tab strip stays 24 (decides open question 1); R10.4 Dev accent `DEV_READY` `#14AE5C`; R10.7 unchecked checkbox border `#404040` (E-2).
- Controller defaults: canvas overlays and component cyan keep their values; chrome status colours move to Untitled. 21:35 sweep: "Inspector field 32 is R8.4 (F OQ2); placeholder is R8.1".
- P2: one primary action per view; selection is a neutral fill. P5: numbers in Roboto Mono.
- Plan.md C-R (2026-10-25, 22:55): `PANEL_ACTION_BUTTON` stays 24; colour-picker track handles keep a white ring (C's `Slider/Handle` is for range sliders only); the header avatar keeps identity colours `AVATAR_COLORS`; "Mixed" is `TEXT_TERTIARY` `#A3A3A3`; drag dividers get a 24 px band (no F divider). Plan.md 22:20: field and select borders join E-2; style editors are F's.
- 2026-09-22: visual evidence in every PR. Standing: protected `main`. 2026-10-10: responsive SKIPPED by default.

## Decisions recorded by the spec writer (technical; owner may reverse)

1. Share is the panel's one primary action and takes `Button/Primary` in both Design and Dev mode (P2). Dev mode is shown by the toolbar, tab badge and Inspect tab, not by recolouring Share.
2. Chrome blue leaves the panel: Share, the MCP URI link and the type-details baseline guide were `#0D99FF`. The baseline guide is a drawn guide over a type preview, so it is treated like a canvas guide and keeps its blue, moved into a constant.
3. Status chips in the picker and Code Connect take the contract's G-1 status badges (success, warning, error), not hand-picked greens and reds.
4. Placeholder swatches for image and empty paints take one constant, `SWATCH_PLACEHOLDER` (bg-quaternary `#404040`), instead of three greys.
5. Handles drawn over document content (gradient handles, picker handles and thumbs) keep white with a black-alpha outline, because they must read on any colour; they move into `theme.rs` constants.
6. The header controls take C's xs sizes and the Present chevron keeps its 16 px visible cell with a 24 px hit box (P3 pointer floor): the extra width (Share +12, Present +8, the gap +2) comes from the free header space left of Present, derived in `right.rs`, so the order, the right alignment, the tab strip and the frozen `RIGHT_PRESENT_*` constants stay.
7. The inspector's segmented groups are 24 tall with 20 px tiles (R10.1, as C says), not 32: the 4-item flow group grows 8 px so each slot is a 24 px hit area, which closes its gap to the Gap field (edge case 10).
8. Swatches keep their visible size and get 24×24 hit areas centred on them; all three fit without moving a neighbour.
9. The header avatar uses `AVATAR_COLORS` (identity, as comments do) with a dark initial; the soul's one orange stays on Share.
10. "Mixed" is `TEXT_TERTIARY` in every form, including the new shadow Blend select.

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

## Changelog (fix round 2026-10-28, review `reviews/DEFG-curator-review.md`, plan.md C-R)

| Review item | Resolution |
|---|---|
| F1 header growth needs frozen constants; chevron ambiguous; zoom height | Fixed. `RIGHT_PRESENT_CHEVRON_W` and `RIGHT_PRESENT_FROM_RIGHT` stay frozen and right.rs derives the positions from Share's measured width (60); the chevron's visible cell stays 16, its hit box is 24; the zoom control stays 24 tall (R10.1 logic, sent to C). The arithmetic (x, width, gap, overlap) is in rule 5; L allow entries listed. |
| F2 `Button/Icon` xs 32 at x 209 overflows | Fixed. Section and name-action buttons stay `PANEL_ACTION_BUTTON` 24; deviation sent to C. |
| F3 pointer floor incomplete (22 px segments, swatches) | Fixed. The 4-item flow group gets 24 px slots (96 wide; its gap to the Gap field goes 8 → 0, flagged); the three swatch kinds get 24×24 hit areas (numbers in rule 5); two red tests. |
| F4 segmented height unstated | Fixed per owner ruling R10.1 (Inspector segmented strip 24) and controller ruling C-K (segmented controls keep today's height): track 24, tiles 20, slots 24, as C says. The review's track-32 suggestion is withdrawn. |
| F5 picker slider handle contradicts C | Fixed: track handles keep a white fill and a dark ring; C's `Slider/Handle` is for range sliders only; sent to C. |
| F6 "Mixed" stretches E-1 | Fixed: `TEXT_TERTIARY` in every form, including the new shadow Blend select. |
| F7 orange avatar | Fixed: `AVATAR_COLORS` by the user's name, dark initial (F-P.13). |
| F8 ring on fields | Fixed: 2 px `BORDER_SELECTED`, no ring. |
| F9 question to A already answered | Fixed: cites plan.md 22:20 (field and select borders join E-2); `check_designs.rs` is K's. |
| F10 AC3 vs the header moves | Fixed: AC3 lists the header changes and points at the "L allow entries" table. |
| F11 unnamed constants | Fixed: nine names (rule 2, section 12). |
| F12 Code Connect pill named `Badge/StatusCount` | Fixed: a gray status badge. |
| F13 `Toggle/Switch` size unstated | Fixed: 36×20, knob 16, hit box the 88 px cell. |
| Re-anchor to `8dfaee93` | 44 of 142 cites moved and were verified; the prompt box moved to G's `dev/code_view.rs`; 81 sites (was 84); drift note above. |
