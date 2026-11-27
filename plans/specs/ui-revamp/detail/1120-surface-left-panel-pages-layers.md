---
id: detail-1120-surface-left-panel-pages-layers
type: note
title: "E Left panel: file header, Pages list and Layers tree in Untitled UI dark"
updated: "2026-10-25"
status: draft
---

## Nguồn
Bản sao từ nhánh `docs/specs-untitled-ui-dark` (`docs/specs/ui-revamp-untitled-dark/1120-surface-left-panel-pages-layers.md`), lấy ngày 25/10/2026 để pack tự đủ. Nguồn chuẩn chi tiết cho US tương ứng.


| Field | Value |
|---|---|
| Issue | #1120 (epic #1118, spec key E) |
| Size | S/M. About one and a half days: 3 owned files, 2 chrome colour sites and 2 derived tints, pages on C's `List/Row` and layers on C's `Tree/Row`, the R5 row and type sizes (Pages rows 24 → 32, panel action buttons 24 → 32), six layer tests moved off the font-size discriminator. |
| Depends on | A (colour constants, `specs/token-contract.md`, contrast check C-1, colour-literal gate), B (type, spacing, radius and control-size scale; `PAGE_ROW_HEIGHT` and `PANEL_ACTION_BUTTON`), C (widget anatomy and states, `specs/C-widget-kit-untitled-anatomy.md`), L (layout-unchanged gate and baselines), G (the Dev accent role; its value `DEV_READY` `#14AE5C` is R10.4). #1108 (Pages/Layers separator) merged after the baseline ref; see section 12. |
| Unblocks | M (docs and screenshot refresh). |
| Touches | `crates/designos-app/src/panels/left.rs` (`:12-190` panel, `:192-194` section title, `:196-307` pages list, `:309-331` page drag, `:333-360` page menu, `:362-459` file header); `crates/designos-app/src/panels/layers.rs` (`:235-250` row text split and type-label colour, `:425-766` rows, drop indicator and scrollbar call, tests `:925-1145`); `crates/designos-app/src/find/filter.rs` (`:50-62` match highlight, `:64-105` layer filter field). Reads only: the left-panel constants in `theme.rs` (`:277-293`, `:310`), owned by A and B. |

Code baseline: `origin/main` `205c72ef` (checked with `git show 205c72ef:<path>`, no checkout). Between `35a65796` and `205c72ef` none of E's three files changed; `theme.rs` lost 3 lines after `:263` (#1106), so `LEFT_HEADER_HEIGHT` is `:277`, `PANEL_ACTION_BUTTON` `:284`, `PAGE_ROW_HEIGHT` `:287`, `PAGES_MAX_VISIBLE_ROWS` `:290`, `LAYER_ROW_HEIGHT` `:293`, `FIND_HIGHLIGHT` `:310`. The census in section 2 was re-measured at `205c72ef`.

Drift after `205c72ef` (measured 2026-10-25 against `origin/main` `7e15ddae`): #1108 merged as `07b961c8`. It changes `panels/left.rs` (+3 lines at the top, edits at `:65-100`, +1 line after `:188`) and adds 8 lines to `theme.rs` after `:290`; `panels/layers.rs` and `find/filter.rs` did not change. Every `left.rs` cite above `:65` moves by 3, the Pages and Dev-section cites need a fresh look, and `theme.rs` cites after `:290` move by 8. `origin/main` then moved to `649da124` (#1151), which touches only `crates/designos-mcp`. Re-anchor against the HEAD you branch from and report drift before writing code.

Naming follows the plan's contract: a colour is named by the token-contract Rust constant, its Untitled token name and the dark hex. A name the contract does not have is marked "new name" and is a request to A. Widgets are named by spec C's anatomy names.

## 1. Summary and user story

As a designer, I read and work the left panel (file name, Pages, Layers) in Untitled UI dark: neutral row fills, Untitled type at the small end of the scale, C's list-row and tree-row states, Studio orange only for live state (drop targets, canvas hover), and component layers still in cyan. The panel keeps its width (240), its order (header, Pages, optional Dev section, Layers) and every control where it is (R1). Page rows grow to the R5 row height (32, set by B), and the panel's icon buttons grow to button xs 32.

## 2. Current state (FACT unless labelled)

- **Panel frame** (`left.rs:12-16`): fill `theme::BG`, right hairline `BORDER`. The Agents/Tools branch (`:17-41`) hands the panel to the agent terminal (spec I) and the Assets branch (`:43-46`) to Assets (spec K); neither is restyled here.
- **File header** (`left.rs:52-61`, `:362-459`): 65 px (`LEFT_HEADER_HEIGHT`, `theme.rs:277`). The file name is `font_large_strong()` 13 px `TEXT` (`left.rs:371`), the project prefix "Project / " `font_body` 11 `TEXT_SECONDARY` (`:395`, `:412`); rename field `BG_SECONDARY` (`:393`); hover pill `BG_HOVER` (`:409`); second line (`:426-458`) in `font_body` `TEXT_SECONDARY` → `TEXT` on hover, and the "Save to library" link in `TEXT_BRAND` (hover `ICON_SELECTED`, `:442`); hide-sidebar button `PANEL_ACTION_BUTTON` 24 (`:53-59`).
- **Pages** (`left.rs:63-89`, `:196-307`): title row 40 (`PANEL_TITLE_ROW_HEIGHT`) with "Pages" in `font_body_strong` 11 `TEXT` (`section_title`, `:192-194`); search and `+` buttons 24 at `tr.top() + 8` (`:66-80`); rows 24 (`PAGE_ROW_HEIGHT` = `CONTROL_DENSE`, `theme.rs:287`), at most 5 visible (`PAGES_MAX_VISIBLE_ROWS`, `:290`; `left.rs:83-86`); selected page `BG_SECONDARY` (`:272-273`, not `BG_SELECTED`), hover `BG_HOVER` (`:274-275`); divider pages as a `BORDER` hairline (`:264-265`); group-label pages in `font_caption_strong` `TEXT_TERTIARY` (`:279-282`); page names `font_body`/`font_body_strong` `TEXT` (`:283-286`); a Ready-for-dev icon in literal `#1BC47D` (`:288-291`); inline rename field `BG_SECONDARY` (`:229-236`); drop line `DROP_INDICATOR` 2 px (`:309-321`); page menu via `popup_menu` (`:333-360`); scrollbar (`:300`, C's `scrollbar`).
- **Dev section** (`left.rs:91-142`, Dev mode only): 52 px block (`:95`); icon and count in literal `#1BC47D` (`:97`, `:105`, `:107-113`), the count as bare `font_caption_strong` text; a 24 px "Mark selection ready" button filled with the Dev green at `gamma_multiply(0.16)`, `0.24` on hover (`:116-124`), label in the Dev green or `TEXT_SECONDARY` (`:132`).
- **Find takes the place of Layers** (`left.rs:145-148`): `find::panel` is spec J's.
- **Layers header** (`left.rs:150-187`): 40 px (`LAYER_HEADER_HEIGHT`), "Layers" title (or the Dev tabs, `dev::list::tabs`, spec G), search and collapse-all buttons 24; filter row 32 (`:182-187`): `find/filter.rs:64-105` draws a 28 px field inset 2 px (`:66`), search icon `ICON_SECONDARY` 14 (`:69`), `TextEdit` in `TEXT` with the hint in `TEXT_TERTIARY` (`:83-91`), and a 20×20 clear button (`:70`, `:94-101`).
- **Layer rows** (`layers.rs:425-766`): rows 32 (`LAYER_ROW_HEIGHT` = `CONTROL_TOOL`, `theme.rs:293`), indent 16, chevron box 24, icon 16, toggles 24. Row states share one pill at the frame inset (`:519-530`): selected `BG_SELECTED`, selected + hover or canvas hover `BG_SELECTED_HOVER`, child of a selected layer `BG_SELECTED_SECONDARY`, canvas hover a 1 px `BORDER_SELECTED` outline, pointer hover a 1 px `BORDER` outline. Chevron colour `ICON_SELECTED`/`ICON`/`ICON_TERTIARY` (`:532-545`). Kind icon `TEXT_COMPONENT` (cyan `#22D3EE`) for component-like layers, else `ICON_SELECTED`/`ICON_TERTIARY` (`:580-586`). Name colour: hidden `TEXT_TERTIARY`, component `TEXT_COMPONENT`, else `TEXT` (`:593-599`). Type label `font_caption` 10 (`:653-675`) coloured by `type_label_color` (`:244-250`): `TEXT_COMPONENT` at `gamma_multiply(0.75)` for visible component-like layers (`:246`), else `TEXT_TERTIARY`. The name elides (`:659`) and the label hides first when the column is short (`split_row_text`, `:235-241`). An image-job progress chip can sit in the row (`:651`, `crate::image_jobs`). Locked rows show only the lock icon; there is no locked name colour. Inline rename: field in `BG` with a `BORDER_SELECTED` outline at `RADIUS_MARK` (`:600-604`). Eye and lock toggles: 14 px icons `ICON_SECONDARY` → `ICON` on hover (`:684-700`). Multi-select is shift/cmd click (`:726-733`) and paints the same selected pill on every selected row. Drop indicator: inside = 2 px `DROP_INDICATOR` outline (`:755`), before/after = 2 px line (`:760`). Filter matches: `find/filter.rs:50-62` paints `FIND_HIGHLIGHT` (`#0D99FF` at 35 %) behind matched characters. Scrollbar call `:766`.

Measured colour census (non-test code, `scripts/rust_literal_scan.clean`; definitions as in spec D section 2):

| File | `theme::` lines | Chrome sites | Site lines | Computed sites |
|---|---|---|---|---|
| `panels/left.rs` | 77 | 2 | 97, 289 | none |
| `panels/layers.rs` | 52 | 0 | | none |
| `find/filter.rs` | 6 | 0 | | none |
| **Total** | 135 | **2** | | 0 |

Derived tints the census does not count (they scale a colour with `gamma_multiply`, not a `Color32` constructor): `left.rs:123` (Dev green at 0.16 and 0.24) and `layers.rs:246` (`TEXT_COMPONENT` at 0.75). Rule 1 gives each a named alpha.

Command (run from the repo root, or from the root of a `git archive` of the ref; writes nothing). Its output at `205c72ef` is pasted in section 10 (exit 0).

```bash
PYTHONDONTWRITEBYTECODE=1 python3 - panels/left.rs panels/layers.rs find/filter.rs <<'PY'
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

The plan's line method (`grep -cE 'Color32::from_rgba?(_unmultiplied|_premultiplied)?\(|Color32::from_gray\(' <file>`) gives at `205c72ef`: `left.rs` 2, `layers.rs` 0, `filter.rs` 0. With tests counted, `layers.rs` has 68 `theme::` lines (51 outside tests).

Gates today: no colour-literal gate (spec A rule 13 adds `scripts/check_chrome_colour_literals.py`, new name). `scripts/check_spacing_relationships.py` ratchets geometry and font literals; `left.rs` and `layers.rs` have entries in `scripts/spacing_baseline.json`. Theme tests pin the control family (`theme.rs:842`) and that selection fills are neutral (`:899`).

## 3. Behaviour rules

1. **Colours from constants only.** The 2 chrome sites go (`left.rs:97`, `:289`); both are the Dev green and take the Dev accent `DEV_READY` `#14AE5C` (R10.4; G's `dev-accent` role). The derived tints get named alphas (new names, requests to A): `DEV_TINT_REST` 0.16 and `DEV_TINT_HOVER` 0.24 (`left.rs:123`), `TYPE_LABEL_COMPONENT_ALPHA` 0.75 (`layers.rs:246`).
2. **Role map.** Surfaces and marks E paints directly (row states are in rule 3):

| Element | State | Constant (Untitled role, hex) | Today |
|---|---|---|---|
| Panel fill | default | `BG` (bg-primary `#0A0A0A`, contract row 1) | `BG` `#121215` |
| Panel edge and section hairlines (`left.rs:15`, `:61`, `:89`, `:141`), divider pages | default | `BORDER` (border-secondary `#262626`), C `Divider/Hairline` | `BORDER` |
| Section titles "Pages", "Layers" | default | C `Panel/Header` title: `TEXT` `#FAFAFA`, text-sm 14/20 semibold | `TEXT`, 11 strong |
| File name | default | `TEXT`, text-sm 14/20 semibold (`FONT_SIZE_LARGE` 13 → 14, contract (c)) | `TEXT`, 13 strong |
| Project prefix and second line | default, hover | `TEXT_TERTIARY` `#A3A3A3` → `TEXT_SECONDARY` `#D4D4D4` on hover, text-xs 12/18 | `TEXT_SECONDARY` → `TEXT`, 11 |
| "Save to library" link | default, hover | `TEXT_BRAND` (fg-brand-secondary `#F97316`, R8.3) → `TEXT` on hover | `TEXT_BRAND` `#FF9E66` → `ICON_SELECTED` |
| Group-label page | default | `TEXT_TERTIARY` `#A3A3A3`, text-xs 12/18 semibold (C `Section/Header` sublabel role) | `TEXT_TERTIARY`, caption 10 strong |
| Type label ("Auto layout", "Frame") | default; component-like | `TEXT_TERTIARY` `#A3A3A3`, text-xs 12/18; component-like: `TEXT_COMPONENT` at `TYPE_LABEL_COMPONENT_ALPHA`; hidden: `TEXT_TERTIARY` (as today) | `TEXT_TERTIARY` / `TEXT_COMPONENT` × 0.75, caption 10 |
| Component, instance, component-set names and kind icons | any | Studio component cyan stays (controller default): `TEXT_COMPONENT` `#22D3EE` (contract row 30) | unchanged |
| Inline rename (page, layer, file name) | editing | C `Input/Field` Focused: fill `BG`, outline `BORDER_SELECTED` (border-brand `#FB923C`, contract row 23), ring `FOCUS_RING`; radius `RADIUS_CONTROL` 8 | `BG`/`BG_SECONDARY` fill, `BORDER_SELECTED`, `RADIUS_MARK` 2 (layers) |
| Drop indicator (pages, layers) | dragging | `DROP_INDICATOR` `#E36323` (contract row 67, kept; live state) | same |
| Canvas hover on a layer row | canvas pointer over the node | 1 px `BORDER_SELECTED` outline (border-brand `#FB923C`) | `BORDER_SELECTED` `#E36323` |
| Filter match highlight (`find/filter.rs:58`) | default | `FIND_HIGHLIGHT` unchanged (contract row 74, Studio-only info family; the 21:35 sweep confirms it for J and E) | same |
| Ready-for-dev icon on a page row; Dev section icon | default | `DEV_READY` `#14AE5C` (R10.4; G's `dev-accent` role) | `#1BC47D` literal |
| Dev section count | default | C `Badge/StatusCount` with G's count-badge rule: fill `DEV_READY` at 22 %, label `TEXT`, mono digits (`UI_MONO_FAMILY`); 0 shows the gray `Badge/StatusCount` (`UTILITY_NEUTRAL_*`) | bare text, Dev green or `TEXT_TERTIARY` |
| "Mark selection ready" button | can mark; hover; cannot mark | fill `DEV_READY` at `DEV_TINT_REST`, `DEV_TINT_HOVER` on hover; label `DEV_READY`; cannot mark: fill `BG_SECONDARY`, label `TEXT_SECONDARY` | same values, literal green |
| Layer filter field | default, focused | C `Input/Search` at `CONTROL_XS` 32: search icon `ICON_QUATERNARY`, typed text `TEXT`, placeholder `TEXT_PLACEHOLDER` `#737373` (R8.1 named exception), clear button glyph `ICON_QUATERNARY` → `ICON_QUATERNARY_HOVER`, focus ring `FOCUS_RING` | `ICON_SECONDARY`, `TEXT`, hint `TEXT_TERTIARY`, `ICON_SECONDARY` → `ICON` |

3. **Widgets and states (spec C).** Pages rows are C `List/Row` xs (cited by C for `left.rs:272-276`); layer rows are C `Tree/Row`. Both take C §3.4's states verbatim, with the owner rulings that override C (R8.2 selected `#262626`; row radius 8 per R5). Section titles are C `Panel/Header` (height 40, title left at 16, actions at the right). The search, `+`, collapse-all and hide-sidebar buttons are C `Button/Icon` xs 32 (R5 "button xs 32"; C's `Panel/Header` names `Button/Icon` xs as its action). The layer filter is C `Input/Search`; the page menu is C `Menu/Panel` and `Menu/Item` (painted by C's `widgets/menu.rs`); every hover tip is C `Tooltip/Default`; scrollbars are C's (`panels/mod.rs`).

| Widget | Default | Hover | Selected | Other states |
|---|---|---|---|---|
| `List/Row` (pages) | no fill; label `TEXT_SECONDARY` `#D4D4D4`; glyph `ICON_QUATERNARY` `#737373` | `BG_HOVER` `#171717`; label `TEXT_SECONDARY_HOVER` `#E5E5E5`; glyph `ICON_QUATERNARY_HOVER` `#A3A3A3` | `BG_SELECTED` `#262626` (was `BG_SECONDARY`); label `TEXT_SECONDARY_HOVER`; glyph `ICON_QUATERNARY_HOVER` | Focused: C's ring `FOCUS_RING` 2 px at gap 2 |
| `Tree/Row` (layers) | no fill; label `TEXT_TERTIARY` `#A3A3A3`; glyphs (chevron, kind icon) `ICON_QUATERNARY` | `BG_HOVER`; label and glyphs unchanged (C) | `BG_SELECTED`; label and glyphs unchanged (C) | Selected + hover and canvas hover on a selected row: `BG_SELECTED_HOVER` (`#262626`, contract row 9: the kit keeps the current fill under the pointer); child of a selected layer: `BG_SELECTED_SECONDARY` (`#171717`, row 10); canvas hover: the 1 px `BORDER_SELECTED` outline (decision 1); Focused: none (C says n/a; layer rows take no keyboard focus today) |
| `Button/Icon` xs (panel actions) | no fill; glyph `ICON_QUATERNARY` | `BG_HOVER`; glyph `ICON_QUATERNARY_HOVER` | `BG_SELECTED`; glyph `ICON_SECONDARY_HOVER` (page search while Find is open, `left.rs:74`) | Pressed `BG_PRESSED` with `ICON_QUATERNARY_HOVER` |
| Eye and lock toggles | glyph `ICON_QUATERNARY`; hidden or locked rows show it without hover, as today | glyph `ICON_QUATERNARY_HOVER` | unchanged | n/a |

Hidden layers keep the Tree/Row label colour (`TEXT_TERTIARY`); the eye-off icon carries the state, and a hidden component-like layer drops its cyan for `TEXT_TERTIARY` (name and type label together, as today). Under C's table a hidden plain layer now looks like a visible one except for its icon; that collision is sent to C (section 12). `ICON_QUATERNARY` never sits on `BG_PRESSED` (2.19:1, contract rule).

4. **States.** Rows show Default, Hover, Selected and, for layers, Selected-Hover, Child-of-selected and Canvas-hover; page rows add C's Focused ring. Hidden and locked are content states on top of these. Disabled has no row form today and gets none. The pointer-hover outline on layer rows goes (C's hover fill replaces it, decision 1).
5. **Sizes (R5) and their layout effect.** Width 240, region order and positions stay (R1).

| Element | Constant / code | Today | Target | Δ | Layout effect |
|---|---|---|---|---|---|
| File header | `theme.rs:277` | 65 | 65 | 0 | the name moves to text-sm 14/20 semibold, centred at y + 24 (`left.rs:369`): the 20 px line spans y + 14 to y + 34 inside the 65 px header |
| Section title rows | `PANEL_TITLE_ROW_HEIGHT`, `LAYER_HEADER_HEIGHT` | 40 | 40 | 0 | title type 11 → 14 |
| Panel action buttons (search, `+`, collapse-all, hide-sidebar) | `PANEL_ACTION_BUTTON` (`theme.rs:284`; B sets it to `CONTROL_XS`) | 24 | 32 | +8 width and height | they sit centred in the 40 px rows (`left.rs:67` `tr.top() + 8.0` becomes the centring offset 4); the two-button clusters widen by 16 px, `PAGES_SEARCH_DX` grows by 8; titles keep their position and order. D's show-sidebar button uses the same constant |
| Page rows | `PAGE_ROW_HEIGHT` (`theme.rs:287`; B sets it to `CONTROL_XS`, contract (c) row 310) | 24 | 32 | +8 per row | the Pages list grows by 8 × visible rows, at most 5: +40 px; Layers starts up to 40 px lower and shows about one fewer row at 1088 px (decision 3) |
| Layer rows | `theme.rs:293` | 32 | 32 | 0 | none |
| Filter row and field | `left.rs:183`, `:186`; `filter.rs:66` | row 32, field 28 | row 32, `Input/Search` 32 (`CONTROL_XS`) | 0 | the field fills the row height (vertical inset 2 → 0); horizontal inset 8 stays |
| Filter clear button hit box | `filter.rs:70` | 20×20 | 24×24 (`TARGET_MIN`, P3 floor); glyph 12 unchanged | 0 | none: the box fits the 32 px field; the text column ends 2 px left of it, as today |
| Row names, prefix, second line, type labels | `font_body` 11, `font_caption` 10 | 10–11 | text-xs 12/18 (`FONT_SIZE_BODY` = `FONT_SIZE_CAPTION` = 12, contract (c) rows 324–325) | 0 | names elide about 8 % earlier at the fixed name column (`layers.rs:659`); the type label hides sooner (`split_row_text`) |
| Dev section | `left.rs:95`, `:140` | 52 / 54 | 52 / 54 (region); count badge `Badge/StatusCount` 22 inside the 28 px title line; button stays 24 | 0 | none; the 24 px button is below C's xs 32, sent to G and C because the 52 px block is a region (R1) |

6. **Radii.** Row pills, inline fields and the filter field take `RADIUS_CONTROL` 8 (R5 "radius md 8", the coordinator's ruling on row and pill radius; C's `Tree/Row` and `List/Row` say radius-sm 6, sent to C). The rename field's `RADIUS_MARK` 2 becomes `RADIUS_CONTROL`. The match highlight keeps `RADIUS_MARK` (it marks characters). Nested shapes stay concentric (P4).
7. **No behaviour change.** Click, double-click, shift/cmd-click, Alt-click on a chevron, drag and drop, rename with Tab, filter and keyboard flow stay. Widget ids recorded through `widgets::record` (`left-dev-ready-section`, `left-dev-mark-ready`, `layers-search`, `layers-collapse-all`, `layers-filter`) stay. Hit boxes change only where R5 or the 24 px floor requires it (panel action buttons, filter clear button).

## 4. Data model / schema changes

None.

## 5. Contracts

**File ownership across D–K.** E owns:

| File | Owner | Shared surface and rule |
|---|---|---|
| `crates/designos-app/src/panels/left.rs` | E | the Agents/Tools branch (`:17-41`) is spec I's surface and the Assets branch (`:43-46`) spec K's: E does not restyle them. The Dev section (`:91-142`), the page-row Ready icon (`:288-291`) and the Dev tabs call (`dev::list::tabs`, `:153-154`) are Dev surfaces: E applies the Dev accent (`DEV_READY`, R10.4) in this file (G cites the same block as `:93-160`). Find takes the panel at `:145-148` (J). #1108's separator is now in this file (section 12). The hide-sidebar button pairs with D's show-sidebar button (`shell.rs:1532-1537`): both `Button/Icon` at `PANEL_ACTION_BUTTON` |
| `crates/designos-app/src/panels/layers.rs` | E | the image-job progress chip in a row (`:651`, `crate::image_jobs`) is drawn by `image_jobs.rs`, which no spec owns (D lists it as unassigned): until the controller assigns it, the chip keeps its current look inside restyled rows |
| `crates/designos-app/src/find/filter.rs` | E | `FIND_HIGHLIGHT` is shared with J's `find/panel.rs`; both keep it (contract row 74) |

Not mine: `theme.rs`, `design/*` (A, B); `widgets/*` and `panels/mod.rs` (C; `widgets/picker.rs` is F's); `tabs.rs`, `shell.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); right-panel files (F); `dev/*` including `dev/list.rs` (Ready-for-dev list and tabs drawn in the left panel), `specs.rs` (G); `terminal.rs`, `agent_*.rs` (I); `find/panel.rs`, `find/mod.rs`, `find/select.rs`, `panels/context.rs` (J); `panels/assets.rs` (K).

**Colour-literal gate.** Spec A rule 13 adds `scripts/check_chrome_colour_literals.py` (new name) with spec D's regex and a falling baseline; until it exists, the section 2 command must print `chrome_sites=0` for the three files.

## 6. File-format mapping

None. The Pages height chosen by #1108 is a preference, not file content (#1108 decides where it persists).

## 7. UI

- Read the in-repo design sources first: `design/principles.md`, `design/soul.md`, `specs/token-contract.md` (A), spec B's scale and spec C's widget kit; they win over generic defaults. The Purple Ban stays (R4). Contributors who use the project's agent kit may also load its `es-designer` skill; it is not required.
- Component, instance and component-set rows stay cyan (controller default; P2 names cyan the component colour).
- Copy is unchanged; Z16 stays green.
- Evidence goes into the PR body (not Figma, R3). Responsive: SKIPPED (owner default).

## 8. Edge cases

1. A file with more than 5 pages: the list scrolls inside 5 × 32 = 160 px (was 120). The scrollbar keeps working (`left.rs:300`, C's scrollbar).
2. With #1108 merged, the user's dragged Pages height is in pixels; after rows grow to 32 a stored height that fitted 5 rows of 24 shows 3.75 rows of 32. #1108's clamps (at least 1 row plus header, at most the content height) still apply; E re-checks the minimum and default with the 32 px row.
3. Long layer names at 12 px: the name elides first, then the type label hides (`split_row_text`); neither overlaps the eye and lock toggles (`layers.rs:590-592`).
4. A selected component row: the selected pill is neutral and the name stays cyan.
5. Canvas hover on a selected row: Selected-Hover fill, not the orange outline (today's order, `layers.rs:519-530`).
6. Hidden and locked together: eye-off and lock both stay visible without hover, as today.
7. Filter on: drag and drop is off (`layers.rs:411-412`, `start_row_drag` refuses while filtering); matched characters keep the `FIND_HIGHLIGHT` mark under the selected pill.
8. Dev mode with Ready rows: the Layers header swaps to the Dev tabs (`left.rs:153-157`, spec G); the Layers rows below keep E's styling.
9. Panel action buttons at 32 px: the Pages cluster (search + `+`) and the Layers cluster (search + collapse-all) each take 16 px more of the 40 px title row's right side; "Pages" and "Layers" are short titles at x 16, so no title elides at the 240 px panel width (INFERENCE from the widths; the PR measures it).

## 9. Non-goals

- The Pages/Layers separator and resizing (#1108); the agent terminal in the left panel (I); Assets (K); Find and replace (J); the Ready-for-dev list body (`dev/list.rs`, G); the image-job chip (`image_jobs.rs`, unassigned).
- Canvas overlays, document content and icon art (R6).
- Changing the left panel width (240) or its resize handle.

## 10. Test plan (red first)

Acceptance criteria:

1. The section 2 command (or A's gate) prints `chrome_sites=0` for `panels/left.rs`, `panels/layers.rs` and `find/filter.rs`.
2. `python3 scripts/check_spacing_relationships.py` passes; per-file counts stay at or below `scripts/spacing_baseline.json`.
3. Spec L's layout gate shows the same regions, panel width and control order as the baseline; the only height change is the Pages list (+8 per visible page row, at most +40) and what follows from it.
4. Every constant in section 3 is a token-contract constant or a new name A has added; `theme::` tests pass with B's updated invariants.
5. `python3 scripts/zero_fake_ui_gate.py` and `python3 scripts/design_os_self_evolve.py` pass.
6. **Contrast.** `ui ds a11y` is not accepted: on this store it measured 0 pairs (false green, plan.md and contract (f)). The check is three parts:
   - (a) The contract's C-1 check (token-contract (f)) exits 0. Run 2026-10-25 from the specs folder against the contract as last edited at 22:15 (62 rows, C-1.1 to C-1.62): exit 0, 62 rows ok, 0 FAIL. E relies on C-1.1 (titles, file name, typed text), C-1.5 (`List/Row` label), C-1.7 (`Tree/Row` label, prefix, second line, group labels, type labels), C-1.9 (`Tree/Row` label on a selected row), C-1.10 ("Save to library"), C-1.25 (filter placeholder, R8.1 exception), C-1.29 (focus ring) and C-1.36 (`DEV_READY` on `BG`: Dev section icon and the Ready icon on a page row, R10.4).
   - (b) The E-P rows below, in the C-1 row shape, pass the contract's snippet with only the id prefix and the file changed. Run 2026-10-25 from the specs folder: exit 0, 18 rows ok (lowest non-text row E-P.13 3.19:1; lowest text row E-P.9 4.91:1).
   - (c) A adds the E-P rows to C-1 and to the `chrome_contrast_pairs` test (spec A section 10). Until then E's PR runs (b).
7. Before/after captures of every command below are in the PR with exit codes.
8. The filter clear button's hit box is at least `TARGET_MIN` (red test below).

E-P rows (foreground on background, WCAG 2.x, the contract's luminance formula; E-P.8 to E-P.10 use composited colours, named in the Use column):

| Id | Foreground | Background | Min | Use |
|---|---|---|---|---|
| E-P.1 | `TEXT_TERTIARY` #A3A3A3 | `BG_HOVER` #171717 | 4.5 | Tree/Row label on hover |
| E-P.2 | `TEXT_TERTIARY` #A3A3A3 | `BG_SELECTED_SECONDARY` #171717 | 4.5 | Tree/Row label inside a selected parent |
| E-P.3 | `TEXT_SECONDARY_HOVER` #E5E5E5 | `BG_HOVER` #171717 | 4.5 | List/Row hover label |
| E-P.4 | `TEXT_SECONDARY_HOVER` #E5E5E5 | `BG_SELECTED` #262626 | 4.5 | List/Row selected label |
| E-P.5 | `TEXT_COMPONENT` #22D3EE | `BG` #0A0A0A | 4.5 | component name |
| E-P.6 | `TEXT_COMPONENT` #22D3EE | `BG_HOVER` #171717 | 4.5 | component name on a hovered row |
| E-P.7 | `TEXT_COMPONENT` #22D3EE | `BG_SELECTED` #262626 | 4.5 | component name on a selected row |
| E-P.8 | `TYPE_LABEL_COMPONENT_ON_SELECTED` #23A8BC | `BG_SELECTED` #262626 | 4.5 | component type label: `TEXT_COMPONENT` at alpha 0.75 over the selected row (worst case; over `BG` 6.42:1) |
| E-P.9 | `DEV_READY` #14AE5C | `DEV_TINT_HOVER_ON_BG` #0C311E | 4.5 | "Mark selection ready" label on its hovered tint (`DEV_READY` at 0.24 over `BG`; 5.65:1 at rest) |
| E-P.10 | `TEXT` #FAFAFA | `DEV_COUNT_BADGE_ON_BG` #0C2E1C | 4.5 | Dev count badge label (`DEV_READY` at 22 % over `BG`) |
| E-P.11 | `ICON_QUATERNARY` #737373 | `BG` #0A0A0A | 3.0 | row glyphs, chevrons, panel action icons, eye and lock, filter icons |
| E-P.12 | `ICON_QUATERNARY` #737373 | `BG_HOVER` #171717 | 3.0 | Tree/Row glyphs on a hovered row |
| E-P.13 | `ICON_QUATERNARY` #737373 | `BG_SELECTED` #262626 | 3.0 | Tree/Row glyphs on a selected row |
| E-P.14 | `ICON_QUATERNARY` #737373 | `BG_SELECTED_SECONDARY` #171717 | 3.0 | Tree/Row glyphs inside a selected parent |
| E-P.15 | `ICON_QUATERNARY_HOVER` #A3A3A3 | `BG_HOVER` #171717 | 3.0 | List/Row and Button/Icon hover glyphs, toggles on hover |
| E-P.16 | `ICON_QUATERNARY_HOVER` #A3A3A3 | `BG_SELECTED` #262626 | 3.0 | List/Row selected glyph, toggles hovered on a selected row |
| E-P.17 | `DROP_INDICATOR` #E36323 | `BG` #0A0A0A | 3.0 | drop indicator |
| E-P.18 | `BORDER_SELECTED` #FB923C | `BG` #0A0A0A | 3.0 | canvas-hover outline, rename field outline |

```bash
# from the specs folder; the contract's C-1 snippet with the id prefix and the file changed; exits 1 on a failure
python3 - E-surface-left-panel-pages-layers.md <<'PY'
import re, sys
def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
bad = 0
for m in re.finditer(r"^\| (E-P\.\d+) \| `\w+` #(\w{6}) \| `\w+` #(\w{6}) \| ([\d.]+) \|", open(sys.argv[1]).read(), re.M):
    hi, lo = sorted((lum(m[2]), lum(m[3])), reverse=True)
    r = (hi + 0.05) / (lo + 0.05)
    ok = r >= float(m[4]); bad += not ok
    print(f"{m[1]:7} {r:5.2f}:1 (min {m[4]}) {'ok' if ok else 'FAIL'}")
sys.exit(1 if bad else 0)
PY
```

Tests. B's scale makes `FONT_SIZE_BODY` and `FONT_SIZE_CAPTION` both 12 (contract (c) rows 324–325), so every layers test that tells the name from the type label by `t.size == FONT_SIZE_BODY` / `FONT_SIZE_CAPTION` stops working: at 12/12 both filters return the name first. They move to a new test helper, `name_and_label(row)`, that orders a row's texts by `rect.left()` (the name is always left of the type label, `layers.rs:653-675`); for the all-rows count, a text is a type label when it is the right-hand text of its row.

| Case | Expected | Fails before |
|---|---|---|
| census of the owned files | `chrome_sites=0` | yes (2 sites) |
| `panels::layers::tests::drop_zones` (`:813`), `flatten_lists_top_first_and_honors_expansion` (`:801`), `rows_cache_not_invalidated_by_canvas_dirty_or_transform_drag` (`:858`), `a_floating_panel_over_the_list_takes_the_press_from_the_row` (`:1148`) | pass; `drop_zones` uses `LAYER_ROW_HEIGHT`, unchanged at 32 | guard |
| `row_text_split_keeps_a_readable_name_or_drops_the_label` (`:946`) | pass unchanged: it calls `split_row_text` with literal widths, so it does not depend on the font | guard |
| updated `hidden_layers_grey_the_type_label_with_the_name` (`:1112`) | picks name and label with `name_and_label` instead of the size filters at `:1138-1139`; assertions unchanged: the visible instance is (`TEXT_COMPONENT`, `TEXT_COMPONENT` at `TYPE_LABEL_COMPONENT_ALPHA`), the hidden instance and the hidden rectangle are (`TEXT_TERTIARY`, `TEXT_TERTIARY`) | breaks with B alone (12/12); green after the update |
| updated `only_rows_inside_the_viewport_paint_a_type_label` (`:925`) | counts names and labels with the row helper instead of the size filters at `:938-939`; same expected counts | breaks with B alone |
| updated `type_label_never_overlaps_the_name_or_the_layer_icon_at_any_depth` (`:1045`), `type_label_is_clipped_to_the_text_column` (`:1064`), `type_label_stays_on_wide_rows_and_gives_way_to_the_name_on_narrow_ones` (`:1082`) | `for_each_nested_auto_layout_row` (`:1028`, size filters at `:1038-1039`) and `:1090-1091` use the helper; the geometry assertions stay and are re-run at 12 px | breaks with B alone |
| new `panels::layers::tests::row_states_use_tree_row_roles` (new name) | built on the `paint_panel` / `row_texts` harness (`:968`, `:995`): a selected, a hovered, a hidden and a component row; asserts the fills (`BG_SELECTED`, `BG_HOVER`), the label colour `TEXT_TERTIARY`, the component name `TEXT_COMPONENT`, and no 1 px `BORDER` hover outline | yes (`TEXT` names, outline hover) |
| new `panels::left::tests::pages_list_is_five_rows_of_the_row_height` (new name; `left.rs` has no test module at `205c72ef`, check #1108's) | the list rect is `min(n, 5) × PAGE_ROW_HEIGHT`, and `PAGE_ROW_HEIGHT` equals `CONTROL_XS` (32, contract (c) row 310, B's change) | red only if E lands before B; it guards B's value afterwards |
| new `find::filter::tests::match_highlight_uses_find_highlight` (new name) | `paint_highlights` fills with `theme::FIND_HIGHLIGHT` | guard (passes today; pins contract row 74) |
| new `find::filter::tests::clear_button_hit_box_meets_the_target_floor` (new name) | the clear button's interact rect is at least `TARGET_MIN` on both axes | yes (20) |
| #1108's separator tests | pass with the 32 px page row | per #1108 |

Run: `cargo test -p designos-app --bin designos panels::layers`, `cargo test -p designos-app --bin designos panels::left`, `cargo test -p designos-app --bin designos find::filter`, `cargo test -p designos-app --bin designos theme::`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**Evidence commands.** `KIT` is a scratch copy of `~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (never the original). `FIX` is a small fixture built with `--apply-spec` from the JSON below (component, instance, hidden layer, nested group, long name); the CLI cannot lock a layer (NodeSpec has `visible` but no `locked`). E-03b is the required capture of the layer rows; E-03 is optional (its tree stays collapsed).

```bash
cd "$(git rev-parse --show-toplevel)"   # repo root: cargo run -p needs it
export PATH="$HOME/.cargo/bin:$PATH"; S="$(mktemp -d)"; mkdir -p "$S/home" "$S/shots"; OUT="$S/shots"
KIT="$S/untitled-ui-kit-copy.fig"; cp "$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$KIT"   # a copy, never the original
FIX="$S/e-layers.designos"
RUN() { env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" cargo run -q --release -p designos-app --bin designos -- "$@"; echo "exit $?"; }
cat > "$S/e-layers.spec.json" <<'JSON'
{"kind":"frame","name":"Revamp fixture","width":480,"height":360,"fill":"#FFFFFF","layout":{"direction":"column","gap":12,"padding":[24,24,24,24]},
 "children":[
  {"kind":"component","name":"Button","layout":{"direction":"row","padding":[8,12,8,12]},"fill":"#E36323","children":[{"kind":"text","name":"Label","text":"Share"}]},
  {"kind":"instance","name":"Button instance","component":"Button"},
  {"kind":"rectangle","name":"Hidden layer","width":120,"height":40,"fill":"#D9D9D9","visible":false},
  {"kind":"frame","name":"Group A","layout":{"direction":"column","gap":4},"children":[{"kind":"text","name":"Nested text","text":"Nested"},{"kind":"rectangle","name":"Nested box","width":60,"height":20,"fill":"#A3A3A3"}]},
  {"kind":"text","name":"A very long layer name that has to elide in the layers panel","text":"Long"}
 ]}
JSON
RUN "$FIX" --apply-spec "$S/e-layers.spec.json"                                                         # builds the fixture
RUN "$FIX" --screenshot "$OUT/E-03b-fixture-rows-expanded.png" --size 1600x1088 --no-toast --select "Nested text"   # required: ancestors expand; component, instance, hidden, nested and long-name rows
RUN "$FIX" --screenshot "$OUT/E-03-fixture-rows.png" --size 1600x1088 --no-toast --select "Revamp fixture"   # optional: selected top-level row, tree collapsed
RUN "$KIT" --screenshot "$OUT/D-01-editor-default.png" --size 1600x1088 --no-toast                      # header, Pages (group label, divider), Layers (shared with spec D)
RUN "$KIT" --screenshot "$OUT/D-07-select-frame.png" --size 1600x1088 --no-toast --select "Welcome to Untitled UI"   # selected layer row (shared with spec D)
RUN "$KIT" --screenshot "$OUT/E-04-dev-left.png" --size 1600x1088 --no-toast --dev css --dev-demo --select "Welcome to Untitled UI"  # Dev section, Ready icon on the page row, Dev tabs
```

Recorded exit codes, with the ref and machine state of each run:

| Capture | Exit | Ref and conditions |
|---|---|---|
| E-00 `--apply-spec` | 0 | spec writer and reviewer, local build of `ec47da26`, 2026-10-25 (`{"root_node_id": 2, "saved": true}`) |
| E-03 | 0 | spec writer and reviewer, same build: one collapsed row |
| E-03b | 1 once ("designos: screenshot was not captured"), then 0 | spec writer's run failed under many concurrent cargo builds; the reviewer's rerun (20:10, same build) exited 0 and shows every fixture row: long-name elision, Group A and its nested rows, the hidden row (grey, eye-off), the component and instance rows (cyan) |
| D-01, D-07 | 0 | spec writer, same build and load (shared with spec D) |
| E-04 | 1 once (same message) | spec writer, same build and load; not rerun |

No capture was rerun at `205c72ef` (E's files did not change between `ec47da26`'s re-anchor and `205c72ef`). The cause of the exit-1 runs is not diagnosed (a `--screenshot` run gives up after 240 frames, `shell.rs:483`). The implementer reruns E-04 on an idle machine and records the exit code in the PR.

Not capturable from the CLI today: hover, keyboard focus, multi-select (`--select` takes one name), locked rows, the layer filter row (no flag opens it), inline rename and drag indicators. The PR covers them with the paint tests above and a manual window capture, labelled manual.

Census output pasted from the run at `205c72ef` (the section 2 command, from the root of `git archive 205c72ef crates/designos-app/src scripts/rust_literal_scan.py`, 2026-10-25, exit 0):

```text
panels/left.rs theme_lines=77 chrome_sites=2 [97, 289] computed=[]
panels/layers.rs theme_lines=52 chrome_sites=0 [] computed=[]
find/filter.rs theme_lines=6 chrome_sites=0 [] computed=[]
exit 0
```

## 11. Performance budgets

- The layers list stays virtualised (`layers.rs:467-469`: only rows in view are painted); a taller page row does not change the per-frame row count of the layers list.
- No allocation added per row; constants are `const`. No `request_repaint` added; idle CPU stays 0 % (CLAUDE.md section 4). `rows_cache` invalidation rules (`layers.rs:300-320`) are unchanged.

## 12. Dependencies and build order

- A, B, C first; L's baselines before the PR. Then: constants in `left.rs` and `layers.rs`, then `find/filter.rs`, then R5 sizes (page row 32, action buttons 32, type 12/14), the test updates, then evidence.
- **B.** `PAGE_ROW_HEIGHT` → `CONTROL_XS` 32 (contract (c) row 310), `PANEL_ACTION_BUTTON` → `CONTROL_XS` 32, `FONT_SIZE_BODY` = `FONT_SIZE_CAPTION` = 12, `FONT_SIZE_LARGE` 14. B's type change breaks the six layers tests listed in section 10 until E's helper lands, so B and E land together or E's test update lands first.
- **#1108 (Pages/Layers separator) is merged** (`07b961c8`, after `205c72ef`). It edits `panels/left.rs` and adds `theme.rs` layout constants, a brand-colour drag highlight and a focus ring. E rebases on it, maps its highlight to `BG_BRAND` and its focus ring to `FOCUS_RING` (R4), and uses the 32 px row for the Pages minimum and default heights.
- **#1106 (icon-only rail).** No E file; only the panel's x position follows the rail width (handled by D and L).
- **G.** The Dev accent role (value `DEV_READY` `#14AE5C`, R10.4) and the count-badge rule.
- **Sent to C** (open items, not owner questions): `Tree/Row` hidden text equals its default text, so a hidden plain layer is told apart only by its icon; `Tree/Row` Focused is n/a while `List/Row` has a ring; `Tree/Row` has no child-of-selected state (E uses `BG_SELECTED_SECONDARY`, contract row 10); row radius 6 in C vs 8 by R5 (E uses 8); the 24 px "Mark selection ready" button inside the 52 px Dev region.
- **Sent to A:** new names `DEV_TINT_REST`, `DEV_TINT_HOVER`, `TYPE_LABEL_COMPONENT_ALPHA`; the E-P rows for C-1 and `chrome_contrast_pairs`.
- **Controller:** `image_jobs.rs` has no owner; its chip sits inside E's rows.
- **Open PRs touching E files** (reviewer, `gh pr view <n> --json files`, 2026-10-25): #770, #768, #765 (stale `task/5xx` branches touching `panels/left.rs` and `panels/layers.rs`). No other open PR touches them.

## 13. Owner rulings that apply

- R1 (layout unchanged), R2 (Untitled spacing and type), R3 (no Figma), R4 (Studio orange, dark label, focus ring Orange-500 `#F97316`, Purple Ban), R5 (text-xs 12/18 rows and labels, text-sm 14/20 panel titles, button xs 32, row 32, radius-md 8), R6 (keep icons), R7 (spec-driven).
- R10.4 Dev accent `DEV_READY` `#14AE5C` (Dev section icon, Ready icon, count badge, Mark-ready button).
- R8.1 placeholder `#737373` (filter field); R8.2 selected `#262626` (page and layer rows); R8.3 accent text `#F97316` ("Save to library"); R8.4 field 32 (filter field); R8.6 row and label text stays 12. R8.5 and R8.7 do not apply (no floating chrome and no destructive button in E's files). R9: figma-agent read-only is allowed; not used here.
- Controller defaults: component/instance cyan stays; canvas overlays keep Figma-parity colours. 21:35 sweep: `FIND_HIGHLIGHT` stays info blue.
- 2026-09-22: visual evidence in every PR. Standing: protected `main`. 2026-10-10: responsive SKIPPED by default.

## Decisions recorded by the spec writer (technical; owner may reverse)

1. The pointer-hover outline on layer rows becomes C's hover fill, so pages and layers hover the same way; the canvas-hover outline stays as an outline (it marks a different source of hover).
2. Page rows take the R5 row (32) like layer rows; the five-row cap stays. A five-page list is 40 px taller and pushes Layers down; #1108 lets the user drag the split (R5 names rows 32; plan.md: heights may grow where R5 raises a control).
3. The "Mark selection ready" button keeps its Dev-tinted look with named alphas; G's count-badge rule covers the count next to it.

## Open questions for the owner

None. The panel button size is decided by R5 and C's `Panel/Header`; the Pages height follows from R5 (decision 2); the match highlight follows the contract; the Dev accent is `DEV_READY` (R10.4).

## Changelog (fix round 2026-10-25, review `reviews/DE-curator-review.md`)

| Review item | Resolution |
|---|---|
| E1 row roles vs C | Fixed. Pages are C `List/Row`, layers C `Tree/Row`, states taken verbatim (layer labels `TEXT_TERTIARY` in every state); the hidden-name collision, row focus, the child-of-selected state and the 6-vs-8 radius are sent to C (§12); E uses radius 8 per the coordinator's R5 ruling. |
| E2 decision 2 and the filter highlight | Fixed. Decision 2 deleted; the row is `FIND_HIGHLIGHT` unchanged (contract row 74, 21:35 sweep); the new filter test asserts `theme::FIND_HIGHLIGHT`. |
| E3 open question 1 | Decided: `Button/Icon` xs 32 (R5 and C `Panel/Header`); the clusters widen by 16 px inside the 40 px rows; B changes `PANEL_ACTION_BUTTON`. |
| E4 tests broken by B's 12/12 | Fixed. Six tests listed as updated with the x-order helper; `hidden_layers_grey_the_type_label_with_the_name` keeps its assertions with the new discriminator; `row_text_split_keeps_a_readable_name_or_drops_the_label` stays a guard because it uses literal widths, not the font; `row_states_use_tree_row_roles` is built on `paint_panel`/`row_texts`. |
| E5 §2 layer-row cites | Fixed at `205c72ef`: row states `:519-530`, chevron `:532-545`, kind icon `:580-586`, name colour `:593-599`, rename `:600-604`, multi-select `:726-733`, drop indicator `:755`/`:760`, type label `:244-250`. Also fixed: header `:362-459`, rename field `:393`, hover pill `:409`, second line `:426-458`, link `:442`. |
| E6 E-03 shows nothing E needs | Fixed: E-03b is the required capture (reviewer rerun exit 0); E-03 optional. |
| E7a file name font | Fixed: `font_large_strong()` 13 (`left.rs:371`) → text-sm 14. |
| E7b scrollbar | Fixed: `left.rs:300`. |
| E7c sublabel link | Fixed: `TEXT_BRAND` → fg-brand-secondary `#F97316` (R8.3). |
| E7d alpha tints | Fixed: named alphas `DEV_TINT_REST`, `DEV_TINT_HOVER`, `TYPE_LABEL_COMPONENT_ALPHA` (new names). |
| E7e pages test | Fixed: cites contract (c) row 310; red only if E lands before B; §1 and §12 say B sets 32. |
| E7f open question 2 | Turned into decision 2 (R5 and plan.md). |
| E7g duplicate `FIX=` | Removed. |
| E7h Dev section count | Fixed: C `Badge/StatusCount` with G's count-badge rule on `DEV_READY` (R10.4) and mono digits. |
| E7i image-job chip | Recorded as unowned (§5, §12, non-goals). |
| Both: AC6 accepted `ui ds a11y` | Fixed: C-1 (contract (f)) and the E-P rows; `ui ds a11y` named as not accepted. |
| Both: R4 5.76 vs 5.73 | Fixed upstream by W1. |
| Both: shared surfaces of E's files | Listed in §5 (I, K, G, J, #1108, D's show-sidebar button, the unowned chip). |
| Coordinator: owner rulings | R8.1, R8.2, R8.3, R8.4, R8.6 applied; R8.5, R8.7 recorded as not applicable; row radius 8. |
| Coordinator: pointer floor | The filter clear button's hit box goes from 20 to 24 (found in this round), with a red test. |
| Owner board 3 (R10.4) | Applied: the Dev accent is `DEV_READY` `#14AE5C`. R10.1 (tab heights), R10.5 (rail dot) and R10.7 (checkbox border) touch no E surface. |
| figma-agent | Not run; no kit fact needed settling, and `design/` was not touched by this round. |
