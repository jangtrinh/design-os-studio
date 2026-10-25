# F Right panel: inspector header, Design and Prototype tabs, Dev Inspect panel, colour picker and inspector popovers in Untitled UI dark

| Field | Value |
|---|---|
| Issue | #1124 (epic #1118, spec key F) |
| Size | L. Four to five days: 17 owned files, 84 literal colour sites (60 chrome choices, 16 canvas overlays that move to theme constants, 7 placeholders, 1 allowlisted image tint), every inspector field and section on C's anatomy, R5 field and label sizes in a 240 px panel. Splittable into F1 (header, tabs, footer, `properties.rs` sections) and F2 (`inspect.rs`, `prototype.rs`, picker and popovers) if the PM wants two PRs. |
| Depends on | A (colour roles and `theme.rs` colour constants), B (type, spacing, radius and control-size scale), C (widget anatomy and states, `specs/C-widget-kit-untitled-anatomy.md`) and L (layout-unchanged gate and baselines). G for the Dev role (Dev-mode Share and plugins row) and for the Dev sections drawn inside the Inspect panel. |
| Unblocks | M (docs and screenshot refresh). |
| Touches | `crates/designos-app/src/panels/right.rs` (`:11-181` header, tabs, footer; `:183-315` Dev plugins; `:317-381` frame presets); `panels/properties.rs` (`:177-353` pane, titles, sublabels, segmented, checkbox; `:389-710` page, styles and selection-colour sections; `:711-1427` selection, dimensions, resizing, auto layout, grid; `:1438-1680` paints; `:1819-2247` picker popup and gradient handles); `panels/inspect.rs` (`:17-21`, `:302-812`); `panels/prototype.rs`; `panels/proto_actions.rs`; `panels/components.rs`; `panels/export.rs`; `panels/stroke.rs`; `panels/effects.rs`; `panels/typography.rs`; `panels/layoutguide.rs`; `panels/vertex.rs`; `panels/var_bind.rs`; `panels/style_picker.rs`; `panels/style_edit.rs`; `panels/style_edit_grid_rows.rs`; `crates/designos-app/src/widgets/picker.rs`. Reads only: the right-panel constants in `theme.rs` (`:318-396`, A and B). |

Code baseline: HEAD `ec47da26` (the local `main` this spec was written on). `origin/main` is `35a65796`, four commits ahead (release 0.6.1); it changes `panels/export.rs` (220 changed lines), `panels/typography.rs` (36), `panels/properties.rs` (35), `panels/inspect.rs` (19), `panels/prototype.rs` (3) and `widgets/picker.rs` (1) (`git diff --stat ec47da26 35a65796 -- <files>`). The colour-site counts in section 2 are identical at `35a65796`; only line numbers move. Re-anchor every cited line against the HEAD you branch from and report drift before writing code. Open PR #1113 changes `panels/typography.rs` and #1112 changes `panels/style_edit.rs` and `panels/style_picker.rs` (section 12).

Colour names follow the plan's naming contract: Untitled token name plus the dark-mode hex, resolved this session from the kit export. Brand roles carry the R4 remap. Provisional names wait for spec A's token contract, which wins on conflict. Names Untitled does not have are marked "new name".

## 1. Summary and user story

As a designer, I read and edit properties in the right panel in Untitled UI dark: Untitled input fields, segmented controls, dropdowns, checkboxes and section headers at the small end of the scale, neutral fills, the Studio orange only on the one primary action (Share) and on live state, and a clear focus ring. In Dev mode the same panel shows the Inspect tab in the same style. The panel stays 240 px wide with its header, tab strip, sections and footer where they are (R1). Sections get longer where R5 raises field and label sizes; the inspector already scrolls.

## 2. Current state (FACT unless labelled)

- **Panel shell** (`right.rs:11-181`). Version history and comment mode replace the whole panel (`:13-20`, specs J/history and J/comments). Fill `BG`, left hairline `BORDER` (`:22-23`). Header: avatar pill 38×24 with a literal amber `#F59E0B` circle and a white initial (`:28-46`); Present button 24 + chevron 16 (`:49-61`); Share 48×24 filled literal `#0D99FF` (or `#1BC47D` in Dev mode) with a white label (`:64-70`; white on `#0D99FF` is 2.99:1); every header button is `RIGHT_BTN_SIZE.min(24.0)` = 24 (`:28`). Tab strip at y 48, height `RIGHT_TABS_H` 24 (`theme.rs:331-332`): "Design | Prototype" or, in Dev mode, "Inspect | Plugins" (`:104-129`), selected fill `BG_SECONDARY`, hover `BG_HOVER`. Zoom dropdown 55×24 (`:132-141`, menu drawn by spec D). Content hairline at `RIGHT_CONTENT_TOP` 81 (`theme.rs:337`, `:143-145`). Footer 36 px with a help circle in literals `#2C2C2C`/`#383838`/`#B3B3B3` (`:161-180`). Dev plugins inspector (`:183-315`, literals `#1BC47D` at `:203`, `:274` and a white label at `:276`). Frame presets inspector when the Frame tool is active with nothing selected (`:317-381`).
- **Design tab** (`properties.rs`). Pane geometry: fields 24 tall (`FIELD_HEIGHT` = `CONTROL_DENSE`, `theme.rs:346`), half fields 88 wide at x 17/113 (`:347-348`), section content 184 wide (`:344`), title rows 40 (`SECTION_TITLE_HEIGHT`, `:342`), sublabels one caption line 14 (`LABEL_BOX_H`, `:365`). Titles `font_body_strong` 11 (`properties.rs:207-227`), sublabels caption 10 (`:285-294`), segmented icon groups (`:299-336`), checkbox (`:338-353`). Sections: page (`:389-453`), styles (`:454-606`) and library styles (`:607-656`) when nothing is selected; selection colours (`:657-710`); selection header, position, rotation, dimensions, resizing, auto layout and grid tracks (`:711-1427`); fill and stroke paint rows (`:1466-1624`); colour picker popup (`:1819-2047`) and on-canvas gradient handles (`:2085-2247`). Literal buttons: "Copy to Slide Deck" `#2C2C2C`/`#383838` (`:759-770`), "Update auto layout" the same (`:1225-1236`). Grey placeholder swatches for image and empty paints `Color32::GRAY`/`from_gray(150)` (`:474`, `:572`, `:634`, `:1444-1445`, `:2305`). Gradient handles on the canvas in white with black-alpha outlines (`:2106-2108`, `:2143-2144`, `:2173`). Document colours are converted by `to_c32` (`:188-191`).
- **Other inspector sections.** `components.rs` (component and instance sections; toggle knob literal white `:86`), `export.rs` (export rows and preview; preview image tint white `:307`), `stroke.rs`, `effects.rs` (`:264` converts a document colour), `typography.rs` (type section and the type-details popover: preview box `#1E1E1E` `:616`, baseline guide `#0D99FF` at 110 alpha `:619`, active toggle text white `:761`; `:282` converts a document colour), `layoutguide.rs` (`:99` converts), `vertex.rs`, `var_bind.rs` (bind-variable hover button and bound pill), `style_picker.rs` (effect/grid style buttons), `style_edit.rs` + `style_edit_grid_rows.rs` (the "Edit style" popover), `proto_actions.rs` (interaction action list).
- **Prototype tab** (`prototype.rs`). "Show prototype settings" button in literals `#2C2C2C`/`#383838` with a white label (`:58-60`); headings in white (`:87`); help text `#B3B3B3` (`:98`, `:102`); device thumbnails and "Manual" label white when selected (`:702`, `:724`); glyph buttons white when selected (`:845`); `:205` and `:752` convert the document's prototype background colour.
- **Dev Inspect tab** (`inspect.rs:302-812`, drawn when Dev mode is on, `right.rs:153-154`). Header name `font_large_strong` 13 with link/code/more icons that turn white on hover (`:339`, `:347`, `:361`); "Selection details" in `#B3B3B3` (`:430`); MCP section title white (`:404`); prompt box `#1E1E1E` + `#383838` with `#D4D4D4` and a `#0D99FF` URI (`:470-485`); Code Connect title white (`:500`), pill `#164E33`/`#454545`/`#383838` with `#4ADE80`/`#B3B3B3` text (`:505-517`); descriptions `#8C8C8C` (`:539-541`, `:613`); Layer properties title white (`:559`); measurement pill label white on `MEASURE` (`:812`, canvas overlay). `inspect.rs:17-21` converts document colours. The Dev sections inside it (component, code, variables, assets, playground) are `dev/sections.rs` and `dev/playground.rs`, spec G.
- **Colour picker** (`widgets/picker.rs`, opened from paint rows). Handle rings white with black alpha (`:666`, `:670`), grey placeholder `from_gray(0x80)` (`:681`), contrast bar text `#E4E4E7` (`:946`) and contrast badges AAA/AA `#17302A`/`#4CC58B`, AA Large `#332715`/`#F5A524`, Low `#361D21`/`#F3657A` (`:951-957`), eyedropper dot (`:1001-1002`), swatch inner ring white 10 % (`:1193`), slider thumb white with a black-alpha ring (`:1483-1484`). `:158` converts a document colour.

Measured colour census (non-test code, `scripts/rust_literal_scan.clean`; definitions as in spec D section 2):

| File | `theme::` lines | Chrome sites | Site lines | Computed sites |
|---|---|---|---|---|
| `panels/right.rs` | 47 | 13 | 38, 39, 66, 68, 173, 175, 176, 203, 274, 276 | none |
| `panels/properties.rs` | 162 | 19 | 474, 572, 634, 763, 765, 1230, 1232, 1444, 1445, 2106, 2107, 2108, 2143, 2144, 2173, 2305 | 190 |
| `panels/inspect.rs` | 62 | 20 | 339, 347, 361, 404, 430, 470, 471, 477, 485, 500, 505, 507, 509, 517, 539, 541, 559, 613, 812 | 19 |
| `panels/prototype.rs` | 95 | 10 | 58, 59, 60, 87, 98, 102, 702, 724, 845 | 205, 752 |
| `panels/components.rs` | 59 | 1 | 86 | none |
| `panels/export.rs` | 28 | 1 | 307 | none |
| `panels/typography.rs` | 72 | 3 | 616, 619, 761 | 282 |
| `widgets/picker.rs` | 84 | 17 | 666, 670, 681, 946, 951, 953, 955, 957, 1001, 1002, 1193, 1483, 1484 | 158 |
| `panels/effects.rs` | 8 | 0 | | 264 |
| `panels/layoutguide.rs` | 9 | 0 | | 99 |
| `proto_actions.rs`, `stroke.rs`, `vertex.rs`, `var_bind.rs`, `style_picker.rs`, `style_edit.rs`, `style_edit_grid_rows.rs` | 24, 7, 8, 9, 9, 2, 0 | 0 | | none |
| **Total** | 685 | **84** | | 8 (all document-colour converters) |

Command (run from the repo root; writes nothing):

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

The plan's line method (`grep -cE 'Color32::from_rgba?(_unmultiplied|_premultiplied)?\(|Color32::from_gray\(' <file>`, tests included) gives `inspect.rs` 13, `properties.rs` 6, `right.rs` 7, `prototype.rs` 6, `picker.rs` 7, `typography.rs` 3, `effects.rs` 1, `layoutguide.rs` 1. It counts the 8 converters and misses named colours, so this spec uses the census above.

Classification of the 84 chrome sites (per-line counts from the same command with a `Counter`; 60 + 16 + 7 + 1 = 84):
- **Chrome colour choices (60)**: become A roles (section 3).
- **Canvas or content overlays (16)**: gradient handles (`properties.rs:2106`, `:2107`, `:2108`, `:2143`, `:2144`, `:2173` ×2; 7 sites), picker handles and thumbs drawn over the colour field (`picker.rs:666`, `:670`, `:1001`, `:1002`, `:1193`, `:1483`, `:1484`; 7 sites), the measurement pill label (`inspect.rs:812`) and the type-details baseline guide (`typography.rs:619`). They keep their values and move to theme constants (A; `CANVAS_HANDLE_FILL` exists, the others are new names).
- **Placeholder swatches (7)**: the grey shown for image or empty paints (`properties.rs:474`, `:572`, `:634`, `:1444`, `:1445`, `:2305`; `picker.rs:681`). They take one role (decision 4).
- **Identity image tint (1)**: `export.rs:307` passes `Color32::WHITE` as the tint of `Painter::image`, not a colour choice. It is the only allowlisted site.
- The 8 computed sites convert document colours (content) and stay.

Gates today: no colour-literal gate (spec D section 5 proposes one, owned by A). `scripts/check_spacing_relationships.py` ratchets geometry and font literals (`properties.rs`, `inspect.rs`, `prototype.rs`, `right.rs`, `picker.rs` all have baseline entries). `scripts/zero_fake_ui_gate.py` Z10 (Share copies a link; Dev Mode's 2nd tab is the plugins inspector), Z11 (`dev-code-connect`, `dev-mcp-btn`, `dev-add-redlines`, `copy-to-slides` do real work) and Z12 (picker image mode) cover this surface. Theme tests pin `RIGHT_TABS_H == CONTROL_DENSE` (`theme.rs:847`) and `LABEL_BOX_H + SPACE_FACT <= CONTROL_DENSE` (`theme.rs:888-891`).

## 3. Behaviour rules

1. **Colours from roles only.** The 60 chrome choices and the 7 placeholders become A roles, the 16 overlay sites become theme constants, and only `export.rs:307` stays literal (allowlisted identity tint).
2. **Role map** (provisional names until A lands; A wins):

| Element | State | Role (Untitled dark) | Today |
|---|---|---|---|
| Panel fill, footer fill | default | `bg-primary` `#0A0A0A` (A's role for `BG`) | `BG` |
| Panel edge, content hairline, footer hairline, section separators | default | `border-secondary` `#262626` | `BORDER` |
| Section titles (Position, Auto layout, Fill, MCP, Code Connect, Layer properties, Prototype headings) | default / dim | `text-primary` `#FAFAFA`, `text-sm` 14/20 semibold; dim titles `text-tertiary` `#A3A3A3` | `TEXT`, `TEXT_SECONDARY`, literal white |
| Sublabels (Position, Rotation, Flow …), "Selection details", group labels | default | `text-tertiary` `#A3A3A3`, `text-xs` 12/18 medium | caption 10, `#B3B3B3`, `#8C8C8C` |
| Help and description text (Prototype help, Code Connect description) | default | `text-tertiary` `#A3A3A3` (7.85:1 on `#0A0A0A`) | `#B3B3B3`, `#8C8C8C` |
| Field values, numbers, hex codes | default | `text-primary`; numbers stay Roboto Mono (P5, D6) at `text-xs` 12 | `TEXT`, 11 |
| Field (number, text, dropdown, hex) | default, hover, focused, disabled | C `Input field · Size=sm` states: `bg-primary` fill, `border-primary` `#404040` border, hover per C, focused `border-brand` + `focus-ring` `#F97316`, disabled per C | `BG_SECONDARY` fills, no border |
| Field prefix labels and icons (X, Y, W, H, rotation) | default | `fg-quaternary` `#737373` icons, `text-quaternary` `#A3A3A3` letters | `TEXT_SECONDARY`, `ICON_SECONDARY` |
| Segmented icon groups (alignment, flow, text align) | default, hover, selected | C segmented control: track `bg-secondary`, selected segment C Selected (neutral), icon `fg-primary` when selected | `BG_SECONDARY`, `BG_SELECTED`, white |
| Secondary buttons (Copy to Slide Deck, Update auto layout, Show prototype settings, + Add redlines, Copy MCP link, Send to Agent Terminal) | default, hover, focused | `Buttons/Button · Size=xs · Hierarchy=Secondary`: `bg-primary` fill, `border-primary`, `text-secondary` → hover per C | `#2C2C2C`/`#383838` literals |
| Share | default, hover, focused | `Buttons/Button · Size=xs · Hierarchy=Primary`: `bg-brand-solid` `#E36323`, label `on-brand-label` `#0A0A0A` (new name for the R4 dark label; `Neutral-950` in spec C's draft; 5.73:1), hover `bg-brand-solid_hover` per C, in both modes (decision 1) | `#0D99FF` / `#1BC47D` + white |
| Present, zoom, avatar | default, hover | `Buttons/Button · Size=xs · Hierarchy=Tertiary` (zoom: C dropdown trigger) | `button_bg`, `BG_HOVER` |
| Avatar circle | default | Untitled avatar placeholder: `utility-orange-600` `#FB923C` fill with `on-brand-label` initial (new name), or C's avatar if C defines one | `#F59E0B` + white |
| Design/Prototype (Inspect/Plugins) tabs | default, hover, selected | C segmented control (`Horizontal tabs`, button type; C name TBD): selected C Selected (neutral) + `text-primary`; others `text-quaternary` | `BG_SECONDARY`, `TEXT_SECONDARY` |
| Checkbox (Clip content, …) | off, on, focused | C checkbox: on = `bg-brand-solid` `#E36323` with `on-brand-label` check (R4) | `BG_BRAND` + white check |
| Toggle (component boolean props, `components.rs:86`; type-details toggles, `typography.rs:761`) | off, on | C toggle: knob `fg-white` `#FFFFFF`, track on `bg-brand-solid` | white knob, white text |
| Help button in the footer | default, hover | `Buttons/Button · Size=xs · Hierarchy=Tertiary` icon-only, circular per C | `#2C2C2C`/`#383838`/`#B3B3B3` |
| Link-like text (MCP URI) | default | `text-brand-secondary` per R4 (provisional `#F97316`; A decides) | `#0D99FF` |
| Prompt and code boxes (MCP prompt, type-details preview) | default | `bg-secondary` `#171717` + `border-secondary`; text `text-secondary` `#D4D4D4`, mono | `#1E1E1E`, `#383838`, `#D4D4D4` |
| Code Connect status pill | connected / not started / hover | C badge: connected `bg-success-primary` `#052E16` + `text-success-primary` `#4ADE80`; not started C gray badge (`bg-secondary` + `text-tertiary`); hover per C | `#164E33`/`#4ADE80`, `#383838`/`#B3B3B3`, `#454545` |
| Picker contrast badges AAA, AA / AA Large / Low | default | C badges: success `bg-success-primary` `#052E16` + `text-success-primary` `#4ADE80`; warning `bg-warning-primary` `#422006` + `text-warning-primary` `#FACC15`; error `bg-error-primary` `#450A0A` + `text-error-primary` `#F87171`; ratio text `text-primary` | `#17302A`/`#4CC58B`, `#332715`/`#F5A524`, `#361D21`/`#F3657A`, `#E4E4E7` |
| Type-details baseline guide | default | a canvas-overlay constant (it draws a guide over a type preview); value stays blue `#0D99FF` at 110 alpha, moved into `theme.rs` (new name) | literal |
| Placeholder swatch (image or empty paint) | default | `bg-quaternary` `#404040` (decision 4) | `GRAY`, `from_gray(150)`, `from_gray(0x80)` |
| Gradient handles, picker handles, slider thumbs, measurement pill label | default | canvas/content overlay constants (white fill, black-alpha outline) in `theme.rs` (A; `CANVAS_HANDLE_FILL` exists, others new names) | literals |
| Dev plugins row icon and "Manage & Run Plugins…" button (`right.rs:203`, `:274-276`) | default, hover | G's Dev role (controller default `DEV_READY` `#14AE5C`); label `on-brand-label` `#0A0A0A` (6.82:1) | `#1BC47D` + white (2.27:1) |
| Icon buttons in the Inspect header (link, code, more) | default, hover | `fg-quaternary` → `fg-quaternary_hover`; never pure white | `ICON_SECONDARY` → white |
| Component and instance names, icons, property chips | any | Studio component cyan stays (controller default) | `TEXT_COMPONENT`, `COMPONENT` |
| Document colours (fills, strokes, effects, page background, prototype background) | n/a | content: drawn from the document, unchanged | `to_c32`, `c32` converters |

3. **Widgets (spec C).** Number, text, hex and opacity fields: `Input field · Size=sm` (with prefix icon or letter, suffix unit). Dropdowns (resizing, blend, font, effect type, variable modes, zoom): C's select / `Dropdown menu`. Segmented icon groups and the tab strip: C's segmented control. Checkbox and toggle: C's. Section headers with "+"/"−" actions: C's panel header. Buttons: `Buttons/Button · Size=xs` with the hierarchy in rule 2. Swatches: C's swatch (radius B's mark role). Popovers (stroke settings, type details, effect settings, edit style, picker): C's floating surface. Every hover tip: `Tooltip`. Scrollbar: C's (`panels/mod.rs:49`).
4. **States.** Fields: Default, Hover, Focused, Disabled, Mixed (today's "Mixed" text stays) and Bound-to-variable (var_bind pill; C's tag/badge anatomy). Buttons and tabs: C's Default, Hover, Pressed, Focused, Disabled, Selected. No state is added or removed.
5. **Sizes (R5) and their layout effect.** Panel width 240, the header, tab strip, section order, field columns (x 17/113, 88 wide) and the 36 px footer keep their places (R1).

| Element | Constant / code | Today | Target | Δ | Layout effect |
|---|---|---|---|---|---|
| Header buttons (avatar, Present, Share) | `right.rs:28` (`RIGHT_BTN_SIZE.min(24)`) | 24 | button `xs` 32 | +8 height | they occupy y 8..40; the tab strip stays at y 48; no region moves |
| Tab strip | `theme.rs:331-332` | 24 at y 48 | C segmented (Open question 1) | 0 or +8 | at 24: none. At C's 32 px track: the content hairline moves from y 80 to 88 (`RIGHT_CONTENT_TOP` 81 → 89), the inspector starts 8 px lower |
| Section title rows | `theme.rs:342` | 40 | 40 | 0 | title type 11 → 14 |
| Field rows | `theme.rs:346`, pitch 24 + 8 | 24 | C `Input field · Size=sm` (C draft: 32) | +8 per row | for "Welcome to Untitled UI" (a 1600×6208 auto-layout frame, capture `D-07`) the visible inspector holds about 19 field rows and 2 buttons: about +152 px of rows and +14 px of buttons (INFERENCE: counted on the capture); the inspector scrolls |
| Sublabels | `LABEL_BOX_H` 14 (`theme.rs:365`) | 14 | `text-xs` 12/18 → 18 | +4 per labelled group | 8 labelled groups on `D-07`: +32 px; B updates `line_heights_fit_their_roles` (18 + `SPACE_FACT` 4 = 22 ≤ 24 still holds for a 24 slot, and fits a 32 slot) |
| Values and labels | `font_body` 11, `font_mono` 11 | 11 | `text-xs` 12 | 0 | values elide earlier in 88 px half fields (variable-mode names "Auto (Sty…" already elide) |
| Style rows | `STYLE_ROW_H` 32 (`theme.rs:371`) | 32 | row 32 | 0 | none |
| Footer | `right.rs:144` | 36, help 28 | 36, C icon button that fits 36 (Open question 2) | 0 | none |
| Popovers (picker, type details, stroke, effects, edit style) | `PROTO_POPOVER_WIDTH`, `VAR_POPOVER_W` and file-local sizes | as today | widths unchanged; rows per C | + per row as above | popovers get taller; they already clamp to the window (spec C's floating surface) |

6. **Radii.** Fields, buttons and segmented tracks take `radius-md` 8 (R5); segment tiles and swatches take B's inner roles, concentric with their track (P4). Literal radii in owned files (`cr(6)`, `cr(4)`, `cr(3)`, `CornerRadius::same(6)`) become B roles and leave the spacing ratchet's radius count.
7. **No behaviour change.** Widget ids (`share-button`, `right-tab-*`, `copy-to-slides`, `al-upgrade-v1`, `dev-mcp-btn`, `dev-code-connect`, `dev-add-redlines`, `picker-contrast-bar`, …), copy actions, keyboard tab order in fields, scrubbing on prefixes, Z10, Z11 and Z12 stay.

## 4. Data model / schema changes

None.

## 5. Contracts

**File ownership across D–K.** F owns:

| File | Owner | Shared surface and rule |
|---|---|---|
| `panels/right.rs` | F | the history panel and comments panel that replace the inspector (`:13-20`) are not restyled here (J) |
| `panels/properties.rs` | F | the page-level Styles section (`:454-656`) overlaps K's "Styles" scope: F owns it because it is inspector content; K lists this file as "not mine" |
| `panels/inspect.rs` | F | Dev mode's Inspect panel: F owns the file; the Dev sections it calls (`dev/sections.rs`, `dev/playground.rs`) and the syntax colours are G's |
| `panels/prototype.rs`, `panels/proto_actions.rs` | F | none |
| `panels/components.rs`, `panels/export.rs`, `panels/stroke.rs`, `panels/effects.rs`, `panels/typography.rs`, `panels/layoutguide.rs`, `panels/vertex.rs`, `panels/var_bind.rs` | F | `var_bind.rs` is the inspector side of variables (its `//!` doc); the Variables panel itself is K's |
| `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` | F | shared with K ("Styles"); proposed owner F because they open from inspector rows; K lists them as "not mine" |
| `widgets/picker.rs` | F | the only `widgets/` file not owned by C (assigned to F by the plan brief) |

Not mine: `theme.rs`, `design/*` (A, B); `widgets/field.rs`, `widgets/menu.rs`, `widgets/mod.rs`, `widgets/popover.rs`, `panels/mod.rs`, and at `origin/main` `widgets/field_tab.rs`, `widgets/popup_open.rs` (C); `tabs.rs`, `shell.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `dev/*`, `specs.rs` (G); `panels/context.rs`, `panels/check_designs.rs` (J, or unassigned), `history/*`, `comments/*` (J); `panels/assets.rs`, `variables/*`, `libraries/*` (K).

**Colour-literal gate.** As spec D section 5 (A's proposed `scripts/check_chrome_colour_literals.py`, new name). F's allowlist: `export.rs:307` (identity image tint). Until the gate exists, the section 2 command must print `chrome_sites=0` for every owned file except `panels/export.rs` (1).

## 6. File-format mapping

None. Document colours shown in fields and swatches are unchanged.

## 7. UI

- UI implementers load the `es-designer` skill first and follow the project's design system: `design/principles.md`, `design/soul.md`, A's token contract, B's scale and C's widget kit win over generic defaults. Purple Ban stays (R4).
- One primary action per view: Share is the only `Hierarchy=Primary` button in the panel (P2). Selected tabs, segments and tool states stay neutral.
- Numbers, hex codes and token paths stay in Roboto Mono (P5, D6).
- Copy is unchanged; Z16 stays green.
- Evidence goes into the PR body (not Figma, R3). Responsive: SKIPPED (owner default).

## 8. Edge cases

1. Long values at 12 px in 88 px half fields (for example "Auto (Style…)" variable modes, long font names): elide with an ellipsis; never overlap the chevron or suffix.
2. Mixed selection: "Mixed" placeholder text uses `text-placeholder` `#737373` only if it reaches 4.5:1 on the field fill; otherwise `text-tertiary` (C decides the placeholder role; `#737373` on `#171717` is 3.78:1).
3. A field bound to a variable shows C's bound pill; the hover "bind" button keeps its hit area inside the field.
4. Dev mode: Share stays brand orange (decision 1); the Plugins row and its button take G's Dev role with a dark label.
5. The colour picker over a very light or very dark document colour: handles keep white fill with a black-alpha outline so they stay visible on any content.
6. Popovers taller after the R5 rows (picker, edit style): they clamp to the window and scroll as C's floating surface does today.
7. The minimum window (900×600): the inspector scrolls; header, tab strip and footer stay fixed.
8. No selection: page and styles sections; Frame tool active with no selection: frame presets list. Both restyle with the same roles.

## 9. Non-goals

- Inspector layout (section order, column x positions, panel width), new sections or new controls.
- The Dev sections' content and syntax highlighting (G), history and comments panels (J), the Variables panel and libraries (K), menus drawn by D.
- Canvas overlay colours (they move into constants with the same values), document content.

## 10. Test plan (red first)

Acceptance criteria:

1. The section 2 command (or A's gate) prints `chrome_sites=0` for every owned file except `panels/export.rs` (`chrome_sites=1`, line 307, allowlisted).
2. `python3 scripts/check_spacing_relationships.py` passes; per-file counts stay at or below `scripts/spacing_baseline.json` (radius literals fall).
3. Spec L's layout gate shows the same regions, widths, column positions and control order as the baseline; height changes match the section 3 table.
4. Every role in section 3 traces to an A constant; `theme::` tests pass with B's updated invariants (`RIGHT_TABS_H`, `LABEL_BOX_H`).
5. `python3 scripts/zero_fake_ui_gate.py` (Z10, Z11, Z12) and `python3 scripts/design_os_self_evolve.py` pass.
6. Contrast: text roles at least 4.5:1 on their fills, icons and focus ring at least 3:1 (computed this session: `#0A0A0A` on `#E36323` 5.73; `#A3A3A3` on `#0A0A0A` 7.85; `#4ADE80` on `#171717` 10.29; `#FACC15` on `#171717` 11.71; `#F87171` on `#171717` 6.48; `#F97316` on `#0A0A0A` 7.06). Share no longer fails (white on `#0D99FF` is 2.99 today).
7. Before/after captures of every command below are in the PR with exit codes.

| Case | Expected | Fails before |
|---|---|---|
| census of the owned files | 0 (1 allowlisted in `export.rs`) | yes (84 sites) |
| `panels::right::tests::*` (`dev_mode_plugins_tab_and_share_avatar_help_execute_real_logic`, the frame-preset scroll tests) | pass | guard |
| `panels::inspect::tests::*`, `panels::typography::tests::*`, `widgets::picker::tests::*` | pass | guard |
| new `panels::right::tests::share_is_the_brand_primary_with_a_dark_label` (new name) | Share fill is the brand role and its label is `on-brand-label` in both modes | yes |
| new `widgets::picker::tests::contrast_badges_use_status_roles` (new name) | the AAA/AA/AA Large/Low badges paint A's success, warning and error roles | yes |
| new `panels::properties::tests::field_rows_use_the_field_height` (new name) | a selected frame's X/Y row is `FIELD_HEIGHT` tall and `FIELD_HEIGHT` equals C's `Input field · Size=sm` height | yes (24) |

Run: `cargo test -p designos-app --bin designos panels::right`, `cargo test -p designos-app --bin designos panels::properties`, `cargo test -p designos-app --bin designos panels::inspect`, `cargo test -p designos-app --bin designos panels::prototype`, `cargo test -p designos-app --bin designos panels::typography`, `cargo test -p designos-app --bin designos widgets::picker`, `cargo test -p designos-app --bin designos theme::`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**Evidence commands.** The exit codes were recorded through the spec writer's wrapper (`run-shot.sh` in the writer's scratch folder, not in the repo), which ran `cargo run -q --release -p designos-app --bin designos -- <the same arguments>` from the repo root with an isolated `HOME` and absolute scratch paths. Each was run once on HEAD `ec47da26` by the spec writer with an isolated `HOME`; exit codes shown. `KIT` is a scratch copy of `~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (never the original).

```bash
cd "$(git rev-parse --show-toplevel)"   # repo root: cargo run -p needs it
export PATH="$HOME/.cargo/bin:$PATH"; S="$(mktemp -d)"; mkdir -p "$S/home" "$S/shots"; OUT="$S/shots"
KIT="$S/untitled-ui-kit-copy.fig"; cp "$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$KIT"   # a copy, never the original
RUN() { env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" cargo run -q --release -p designos-app --bin designos -- "$@"; echo "exit $?"; }
RUN "$KIT" --screenshot "$OUT/D-01-editor-default.png" --size 1600x1088 --no-toast                      # exit 0: no selection: Page and Styles sections, header, tabs, footer (shared with D)
RUN "$KIT" --screenshot "$OUT/D-07-select-frame.png" --size 1600x1088 --no-toast --select "Welcome to Untitled UI"   # exit 0: Design tab with a frame (shared with D)
RUN "$KIT" --screenshot "$OUT/F-02-prototype-tab.png" --size 1600x1088 --no-toast --tab prototype --select "Welcome to Untitled UI"   # exit 0: Prototype tab
RUN "$KIT" --screenshot "$OUT/F-03-picker.png" --size 1600x1088 --no-toast --panel picker --select "Welcome to Untitled UI"           # exit 1 once ("designos: screenshot was not captured"): colour picker, contrast badge
RUN "$KIT" --screenshot "$OUT/F-04-type-details.png" --size 1600x1088 --no-toast --panel type-details --select "Welcome to Untitled UI"  # exit 0: type-details popover
RUN "$KIT" --screenshot "$OUT/D-09-dev-css.png" --size 1600x1088 --no-toast --dev css --select "Welcome to Untitled UI"             # exit 1 twice ("designos: screenshot was not captured"): Dev Inspect tab (shared with D and G)
```

Not capturable from the CLI today: hover, focus, the Mixed state, a bound field (needs a document with variables bound to the selection), the Dev Plugins tab (no `--tab` value for it in Dev mode). The PR covers them with the new paint tests and a manual window capture, labelled manual.

## 11. Performance budgets

- Inspector paint stays one pass per frame with no new allocation per field; roles are `const`.
- Taller rows add no layout passes: the pane advances `y` by the new constants.
- No `request_repaint` added; idle CPU stays 0 % (CLAUDE.md section 4). The picker and gradient handles keep their current repaint behaviour.

## 12. Dependencies and build order

- A, B, C first; L's baselines before the first PR. G's Dev role decision before the Dev-mode Share/Plugins items. Build order: roles in `right.rs` (header, tabs, footer), `properties.rs` sections and shared helpers (`title`, `sublabel`, `segmented`, `checkbox`), the other section files, `prototype.rs`, `inspect.rs`, `widgets/picker.rs` and the popovers; then R5 sizes; then evidence.
- **Open PRs touching F files** (`gh pr list --state open`, 2026-10-25): #1113 `fix/issue-1026-typography-style-link` (`panels/typography.rs`); #1112 `jangtrinh/agy-127-delete-style` (`panels/style_edit.rs`, `panels/style_picker.rs`); stale #770, #768, #765 (`panels/inspect.rs`, `panels/properties.rs`, `panels/components.rs`, `panels/effects.rs`, `panels/style_edit.rs`, `panels/var_bind.rs`). Rebase on #1113 and #1112 if they merge first.
- #1106 and #1108 touch no F file.

## 13. Owner rulings that apply

- R1 (layout unchanged), R2 (Untitled spacing and type), R4 (Studio orange, dark label `#0A0A0A`, focus ring Orange-500 `#F97316`, Purple Ban), R5 (`text-xs` 12/18 rows and labels, `text-sm` 14/20 titles, button `xs` 32, row 32, `radius-md` 8), R6 (keep icons), R3 (no Figma), R7 (spec-driven).
- Controller default (2026-10-25): canvas overlays and component cyan keep their values; chrome status colours move to Untitled.
- P2: one primary action per view; selection is a neutral fill. P5: numbers in Roboto Mono.
- 2026-09-22: visual evidence in every PR. Standing: protected `main`. 2026-10-10: responsive SKIPPED by default.

## Decisions recorded by the spec writer (technical; owner may reverse)

1. Share is the panel's one primary action and takes the brand primary button in both Design and Dev mode (P2). Dev mode is shown by the toolbar, tab badge and Inspect tab, not by recolouring Share.
2. Chrome blue leaves the panel: Share, the MCP URI link and the type-details baseline guide were `#0D99FF`. The baseline guide is a drawn guide over a type preview, so it is treated like a canvas guide and keeps its blue, moved into a constant.
3. Status chips in the picker and Code Connect take Untitled status roles (success, warning, error), not hand-picked greens and reds.
4. Placeholder swatches for image and empty paints take `bg-quaternary` `#404040`, one neutral role instead of three greys.
5. Handles drawn over document content (gradient handles, picker handles and thumbs) keep white with a black-alpha outline, because they must read on any colour; they move into `theme.rs` constants.

## Open questions for the owner

1. **Inspector tab strip height.** P3 says segmented tab strips are 24 px and a theme test pins it (`theme.rs:847`); spec C's draft segmented control is a 32 px track (28 px tabs). Recommended: keep the 24 px strip with C's anatomy (no change to the header; one less layout shift). Alternative: C's 32 px track; the inspector content starts 8 px lower (`RIGHT_CONTENT_TOP` 81 → 89) and B rewrites the test.
2. **Inspector field height.** R5 names buttons `xs` 32 and rows 32 but not fields; C's draft field is 32. Recommended: fields at C's smallest Untitled input size (32), accepting about +150 to +200 px of scroll for a frame with auto layout, because R2 takes Untitled sizes and a 24 px field is below any Untitled input. Alternative: keep 24 px fields (dense, Figma-like) with Untitled colours and radius only.
