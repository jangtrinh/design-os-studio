# E Left panel: file header, Pages list and Layers tree in Untitled UI dark

| Field | Value |
|---|---|
| Issue | #1120 (epic #1118, spec key E) |
| Size | S/M. About two days: 4 owned files (`left.rs`, `left_sep.rs`, `layers.rs`, `filter.rs`), 2 chrome colour sites and 4 derived tints, pages on C's `List/Row` and layers on C's `Tree/Row`, the R5 row and type sizes (Pages rows 24 → 32, the four panel action buttons 24 → 32 through names local to `left.rs`), the Pages/Layers separator's 24 px hit band and focus ring, six layer tests moved off the font-size discriminator, one #1108 test updated. |
| Depends on | A (colour constants, `specs/token-contract.md`, contrast check C-1, colour-literal gate), B (type, spacing, radius and control-size scale; `PAGE_ROW_HEIGHT`; `PANEL_ACTION_BUTTON` and `PAGES_SEARCH_DX` stay frozen, B rule 6), C (widget anatomy and states, `specs/C-widget-kit-untitled-anatomy.md`), L (layout-unchanged gate and baselines), G (the Dev accent role; its value `DEV_READY` `#14AE5C` is R10.4). #1108 (Pages/Layers separator) is merged and its file `panels/left_sep.rs` is E's (plan.md, 22:55); see section 12. |
| Unblocks | M (docs and screenshot refresh). |
| Touches | `crates/designos-app/src/panels/left.rs` (`:27-208` panel, `:210-212` section title, `:214-325` pages list, `:327-349` page drag, `:351-378` page menu, `:380-477` file header); `crates/designos-app/src/panels/left_sep.rs` (`:48-54` hit rect, `:67-198` press, drag and keyboard handling, `:202-237` paint, focus ring and AccessKit node); `crates/designos-app/src/panels/layers.rs` (`:235-250` row text split and type-label colour, `:425-766` rows, drop indicator and scrollbar call, tests `:925-1145`); `crates/designos-app/src/find/filter.rs` (`:50-62` match highlight, `:64-105` layer filter field). Reads only: the left-panel constants in `theme.rs` (`:277-301`, `:318`), owned by A and B. |

Code baseline: `origin/main` `8dfaee93` (v0.7.0 merged, 2026-10-28; read with `git show 8dfaee93:<path>`, no checkout). The spec was written against `205c72ef`; every `file.rs:line` cite in sections 2 to 12 was moved to `8dfaee93` from the `git diff -U0` hunks between the two commits (54 of the 126 cites in this file moved) and each moved cite was checked to point at the same source line at both refs. Cites into `panels/left_sep.rs` (added by #1108 after `205c72ef`) are at `8dfaee93`. Cites in the changelog at the end are history. The census in section 2 was re-run at `8dfaee93` (section 10).

Drift to `8dfaee93` (measured 2026-10-28; 217 app files changed since `649da124`). `panels/layers.rs` and `find/filter.rs` did not change. `panels/left.rs` grew 32 lines: +3 and +12 at the top (the `left_sep` module, and `agent_card_reserve` for the Agents branch, spec I's), #1108's edits in the Pages block (`:78-105`), and +1 after the Layers header; its census sites moved from `:97` and `:289` to `:114` and `:307`. `panels/left_sep.rs` (237 lines) and `shell/tests/pages_layers_separator.rs` (289 lines, `shell/tests.rs:1163`) came with #1108, so they were not in the base this spec was written on; plan.md (22:55) makes the separator file E's. `theme.rs` +8 after `:290` (#1108: `PAGES_SEPARATOR_*`, `LAYERS_MIN_HEIGHT`, `LEFT_DEV_READY_SECTION_STEP`). **Rules the drift touches.** (1) The separator is a surface this spec now owns: its 6 px hit zone, its raw-event press test and its 1 px `BORDER_SELECTED` focus outline are restyled (rule 5, decisions 4 and 5). (2) #1108's tests assert a 6 px recorded rect (`pages_layers_separator.rs:42-60`, which keeps passing) and a `BORDER_SELECTED` outline (`:187-200`, which changes), so one of its six tests changes with E. (3) `PAGE_ROW_HEIGHT` is used by the separator's bounds (`left_sep.rs:35-46`), so rows of 32 change its min, default and max by the same factor, as edge case 2 says. No surface of E was removed. Re-anchor against the HEAD you branch from and report drift before writing code.

Naming follows the plan's contract: a colour is named by the token-contract Rust constant, its Untitled token name and the dark hex. A name the contract does not have is marked "new name" and is a request to A. Widgets are named by spec C's anatomy names.

## 1. Summary and user story

As a designer, I read and work the left panel (file name, Pages, Layers) in Untitled UI dark: neutral row fills, Untitled type at the small end of the scale, C's list-row and tree-row states, Studio orange only for live state (drop targets, canvas hover), and component layers still in cyan. The panel keeps its width (240), its order (header, Pages, optional Dev section, Layers) and every control where it is (R1). Page rows grow to the R5 row height (32, set by B), and the panel's icon buttons grow to button xs 32 (through names local to `left.rs`: `PANEL_ACTION_BUTTON` stays 24 because it also sizes F's inspector buttons at x 209, B rule 6).

## 2. Current state (FACT unless labelled)

- **Panel frame** (`left.rs:27-31`): fill `theme::BG`, right hairline `BORDER`. The Agents/Tools branch (`:32-56`) hands the panel to the agent terminal (spec I) and the Assets branch (`:58-61`) to Assets (spec K); neither is restyled here.
- **File header** (`left.rs:67-76`, `:380-477`): 65 px (`LEFT_HEADER_HEIGHT`, `theme.rs:277`). The file name is `font_large_strong()` 13 px `TEXT` (`left.rs:389`), the project prefix "Project / " `font_body` 11 `TEXT_SECONDARY` (`:413`, `:430`); rename field `BG_SECONDARY` (`:411`); hover pill `BG_HOVER` (`:427`); second line (`:444-476`) in `font_body` `TEXT_SECONDARY` → `TEXT` on hover, and the "Save to library" link in `TEXT_BRAND` (hover `ICON_SELECTED`, `:460`); hide-sidebar button `PANEL_ACTION_BUTTON` 24 (`:68-74`).
- **Pages** (`left.rs:78-105`, `:214-325`): title row 40 (`PANEL_TITLE_ROW_HEIGHT`) with "Pages" in `font_body_strong` 11 `TEXT` (`section_title`, `:210-212`); search and `+` buttons 24 at `tr.top() + 8` (`:82-96`); rows 24 (`PAGE_ROW_HEIGHT` = `CONTROL_DENSE`, `theme.rs:287`), at most 5 visible (`PAGES_MAX_VISIBLE_ROWS`, `:290`; `left.rs:99-101`); selected page `BG_SECONDARY` (`:290-291`, not `BG_SELECTED`), hover `BG_HOVER` (`:292-293`); divider pages as a `BORDER` hairline (`:282-283`); group-label pages in `font_caption_strong` `TEXT_TERTIARY` (`:297-300`); page names `font_body`/`font_body_strong` `TEXT` (`:301-304`); a Ready-for-dev icon in literal `#1BC47D` (`:306-309`); inline rename field `BG_SECONDARY` (`:247-254`); drop line `DROP_INDICATOR` 2 px (`:327-339`); page menu via `popup_menu` (`:351-378`); scrollbar (`:318`, C's `scrollbar`).
- **Dev section** (`left.rs:107-159`, Dev mode only): 52 px block (`:111`); icon and count in literal `#1BC47D` (`:114`, `:122`, `:124-130`), the count as bare `font_caption_strong` text, painted at every value (at zero in `TEXT_TERTIARY`, `left.rs:124-130`); a 24 px "Mark selection ready" button filled with the Dev green at `gamma_multiply(0.16)`, `0.24` on hover (`:133-141`), label in the Dev green or `TEXT_SECONDARY` (`:149`).
- **Find takes the place of Layers** (`left.rs:162-165`): `find::panel` is spec J's.
- **Layers header** (`left.rs:167-204`): 40 px (`LAYER_HEADER_HEIGHT`), "Layers" title (or the Dev tabs, `dev::list::tabs`, spec G), search and collapse-all buttons 24; filter row 32 (`:199-204`): `find/filter.rs:64-105` draws a 28 px field inset 2 px (`:66`), search icon `ICON_SECONDARY` 14 (`:69`), `TextEdit` in `TEXT` with the hint in `TEXT_TERTIARY` (`:83-91`), and a 20×20 clear button (`:70`, `:94-101`).
- **Pages/Layers separator** (`panels/left_sep.rs`, #1108; E's file). The press test is raw: `prepare_height` reads pointer events before the lists lay out and grabs a press whose position is inside `hit_rect_for` (`left_sep.rs:48-54`, `:80-98`); that rect is the full inner width and `PAGES_SEPARATOR_HIT` 6 px tall (`theme.rs:291`), centred on the line at `list_bottom + SPACE_INLINE − BORDER_WIDTH` = list bottom + 7. `paint_and_interact` (`:202-237`) registers the same rect as `pages-layers-separator` (`:207-208`), draws the 1 px `BORDER` line, a 2 px `BG_BRAND` line on hover or drag (`:210-216`), a 1 px `BORDER_SELECTED` outline around the hit rect on keyboard focus (`:218-223`) and an AccessKit `Splitter` node (`:225-236`). Keyboard: arrows step 8 px (`PAGES_SEPARATOR_STEP`), Shift 40, Home, End, Enter resets (`:144-181`). Its bounds use `PAGE_ROW_HEIGHT` (`:35-46`).
- **Layer rows** (`layers.rs:425-766`): rows 32 (`LAYER_ROW_HEIGHT` = `CONTROL_TOOL`, `theme.rs:301`), indent 16, chevron box 24, icon 16, toggles 24. Row states share one pill at the frame inset (`:519-530`): selected `BG_SELECTED`, selected + hover or canvas hover `BG_SELECTED_HOVER`, child of a selected layer `BG_SELECTED_SECONDARY`, canvas hover a 1 px `BORDER_SELECTED` outline, pointer hover a 1 px `BORDER` outline. Chevron colour `ICON_SELECTED`/`ICON`/`ICON_TERTIARY` (`:532-545`). Kind icon `TEXT_COMPONENT` (cyan `#22D3EE`) for component-like layers, else `ICON_SELECTED`/`ICON_TERTIARY` (`:580-586`). Name colour: hidden `TEXT_TERTIARY`, component `TEXT_COMPONENT`, else `TEXT` (`:593-599`). Type label `font_caption` 10 (`:653-675`) coloured by `type_label_color` (`:244-250`): `TEXT_COMPONENT` at `gamma_multiply(0.75)` for visible component-like layers (`:246`), else `TEXT_TERTIARY`. The name elides (`:659`) and the label hides first when the column is short (`split_row_text`, `:235-241`). An image-job progress chip can sit in the row (`:651`, `crate::image_jobs`). Locked rows show only the lock icon; there is no locked name colour. Inline rename: field in `BG` with a `BORDER_SELECTED` outline at `RADIUS_MARK` (`:600-604`). Eye and lock toggles: 14 px icons `ICON_SECONDARY` → `ICON` on hover (`:684-700`). Multi-select is shift/cmd click (`:726-733`) and paints the same selected pill on every selected row. Drop indicator: inside = 2 px `DROP_INDICATOR` outline (`:755`), before/after = 2 px line (`:760`). Filter matches: `find/filter.rs:50-62` paints `FIND_HIGHLIGHT` (`#0D99FF` at 35 %) behind matched characters. Scrollbar call `:766`.

Measured colour census (non-test code, `scripts/rust_literal_scan.clean`; definitions as in spec D section 2):

| File | `theme::` lines | Chrome sites | Site lines | Computed sites |
|---|---|---|---|---|
| `panels/left.rs` | 75 | 2 | 114, 307 | none |
| `panels/layers.rs` | 52 | 0 | | none |
| `find/filter.rs` | 6 | 0 | | none |
| **Total** | 133 | **2** | | 0 |

Derived tints the census does not count (they scale a colour with `gamma_multiply`, not a `Color32` constructor): `left.rs:140` (Dev green at 0.16 and 0.24), `layers.rs:246` (`TEXT_COMPONENT` at 0.75) and the Dev count badge's 22 % (G's count-badge rule, the same alpha as `dev/sections.rs:29`). Rule 1 gives each a named alpha.

Command (run from the repo root, or from the root of a `git archive` of the ref; writes nothing). Its output at `8dfaee93` is pasted in section 10 (exit 0).

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

The plan's line method (`grep -cE 'Color32::from_rgba?(_unmultiplied|_premultiplied)?\(|Color32::from_gray\(' <file>`) gives at `8dfaee93`: `left.rs` 2, `layers.rs` 0, `filter.rs` 0. With tests counted, `layers.rs` has 68 `theme::` lines (51 outside tests).

Gates today: no colour-literal gate (spec A rule 13 adds `scripts/check_chrome_colour_literals.py`, new name). `scripts/check_spacing_relationships.py` ratchets geometry and font literals; `left.rs` and `layers.rs` have entries in `scripts/spacing_baseline.json`. Theme tests pin the control family (`theme.rs:850`) and that selection fills are neutral (`:899`).

## 3. Behaviour rules

1. **Colours from constants only.** The 2 chrome sites go (`left.rs:114`, `:307`); both are the Dev green and take the Dev accent `DEV_READY` `#14AE5C` (R10.4; G's `dev-accent` role). The derived tints get named alphas (new names, requests to A): `DEV_TINT_REST` 0.16 and `DEV_TINT_HOVER` 0.24 (`left.rs:140`), `TYPE_LABEL_COMPONENT_ALPHA` 0.75 (`layers.rs:246`), and `DEV_STATUS_TINT` 0.22 for the Dev count badge (one name shared with G's status pill and count badge).
2. **Role map.** Surfaces and marks E paints directly (row states are in rule 3):

| Element | State | Constant (Untitled role, hex) | Today |
|---|---|---|---|
| Panel fill | default | `BG` (bg-primary `#0A0A0A`, contract row 1) | `BG` `#121215` |
| Panel edge and section hairlines (`left.rs:30`, `:76`, `:158`; the Pages/Layers line is now the separator's, `left_sep.rs:210`), divider pages | default | `BORDER` (border-secondary `#262626`), C `Divider/Hairline` | `BORDER` |
| Section titles "Pages", "Layers" | default | C `Panel/Header` title: `TEXT` `#FAFAFA`, text-sm 14/20 semibold | `TEXT`, 11 strong |
| File name | default | `TEXT`, text-sm 14/20 semibold (`FONT_SIZE_LARGE` 13 → 14, contract (c)) | `TEXT`, 13 strong |
| Project prefix and second line | default, hover | `TEXT_TERTIARY` `#A3A3A3` → `TEXT_SECONDARY` `#D4D4D4` on hover, text-xs 12/18 | `TEXT_SECONDARY` → `TEXT`, 11 |
| "Save to library" link | default, hover | `TEXT_BRAND` (fg-brand-secondary `#F97316`, R8.3) → `TEXT` on hover | `TEXT_BRAND` `#FF9E66` → `ICON_SELECTED` |
| Group-label page | default | `TEXT_TERTIARY` `#A3A3A3`, text-xs 12/18 semibold (C `Section/Header` sublabel role) | `TEXT_TERTIARY`, caption 10 strong |
| Type label ("Auto layout", "Frame") | default; component-like | `TEXT_TERTIARY` `#A3A3A3`, text-xs 12/18; component-like: `TEXT_COMPONENT` at `TYPE_LABEL_COMPONENT_ALPHA`; hidden: `TEXT_TERTIARY` (as today) | `TEXT_TERTIARY` / `TEXT_COMPONENT` × 0.75, caption 10 |
| Component, instance, component-set names and kind icons | any | Studio component cyan stays (controller default): `TEXT_COMPONENT` `#22D3EE` (contract row 30) | unchanged |
| Inline rename (page, layer, file name) | editing | C `Input/Field` Focused: fill `BG`, border 2 px `BORDER_SELECTED` (border-brand `#FB923C`, contract row 23) and **no ring** (C S4: text fields and selects draw none; the border carries focus); radius `RADIUS_CONTROL` 8 | `BG`/`BG_SECONDARY` fill, 1 px `BORDER_SELECTED`, `RADIUS_MARK` 2 (layers) |
| Drop indicator (pages, layers) | dragging | `DROP_INDICATOR` `#E36323` (contract row 67, kept; live state) | same |
| Canvas hover on a layer row | canvas pointer over the node | 1 px `BORDER_SELECTED` outline (border-brand `#FB923C`) | `BORDER_SELECTED` `#E36323` |
| Filter match highlight (`find/filter.rs:58`) | default | `FIND_HIGHLIGHT` unchanged (contract row 74, Studio-only info family; the 21:35 sweep confirms it for J and E) | same |
| Ready-for-dev icon on a page row; Dev section icon | default | `DEV_READY` `#14AE5C` (R10.4; G's `dev-accent` role) | `#1BC47D` literal |
| Dev section count | count > 0; count 0 | C `Badge/StatusCount` with G's count-badge rule: fill `DEV_READY` at `DEV_STATUS_TINT` (22 %), label `TEXT`, mono digits (`UI_MONO_FAMILY`). Zero keeps today's behaviour (R1, plan.md C-R): the count is still painted (`left.rs:124-130` paints "0" in `TEXT_TERTIARY`), now as the gray `Badge/StatusCount` (`UTILITY_NEUTRAL_50` fill, `_200` border, `_700` label). G's Ready for dev tab badge hides at zero, as today (`dev/list.rs:46`): the two counters differ today (this one counts the current page and is always shown, that one counts every ready item) and stay different | bare text, Dev green or `TEXT_TERTIARY` |
| "Mark selection ready" button | can mark; hover; cannot mark | fill `DEV_READY` at `DEV_TINT_REST`, `DEV_TINT_HOVER` on hover; label `DEV_READY`; cannot mark: fill `BG_SECONDARY`, label `TEXT_SECONDARY` | same values, literal green |
| Layer filter field | default, focused | C `Input/Search` at `CONTROL_XS` 32: fill `BG`, 1 px `BORDER_STRONG` `#404040` (E-2: text fields join it), search icon `ICON_QUATERNARY`, typed text `TEXT`, placeholder `TEXT_PLACEHOLDER` `#737373` (R8.1 named exception), clear button glyph `ICON_QUATERNARY` → `ICON_QUATERNARY_HOVER`; focused: border 2 px `BORDER_SELECTED` and **no ring** (C S4) | `ICON_SECONDARY`, `TEXT`, hint `TEXT_TERTIARY`, `ICON_SECONDARY` → `ICON` |
| Pages/Layers separator line | default; hover or drag | 1 px `BORDER` line; 2 px `BG_BRAND` on hover and drag (live state, as #1108 draws it, `left_sep.rs:210-216`) | same |
| Pages/Layers separator focus | keyboard focus | C S4 ring: `FOCUS_RING` 2 px, 2 px gap, around the 6 px core rect (`PAGES_SEPARATOR_HIT`), radius `RADIUS_MARK` 2 + 4; it replaces the 1 px `BORDER_SELECTED` outline around the hit rect (`left_sep.rs:222`) | 1 px `BORDER_SELECTED` |

3. **Widgets and states (spec C).** Pages rows are C `List/Row` xs (cited by C for `left.rs:290-294`); layer rows are C `Tree/Row`. Both take C §3.4's states verbatim, with the owner rulings that override C (R8.2 selected `#262626`; row radius 8 per R5). Section titles are C `Panel/Header` (height 40, title left at 16, actions at the right). The search, `+`, collapse-all and hide-sidebar buttons are C `Button/Icon` xs 32 (R5 "button xs 32"; C's `Panel/Header` names `Button/Icon` xs as its action), sized by `left.rs`-local names, not by `PANEL_ACTION_BUTTON` (rule 5). The layer filter is C `Input/Search`; the page menu is C `Menu/Panel` and `Menu/Item` (painted by C's `widgets/menu.rs`); every hover tip is C `Tooltip/Default`; scrollbars are C's (`panels/mod.rs`).

| Widget | Default | Hover | Selected | Other states |
|---|---|---|---|---|
| `List/Row` (pages) | no fill; label `TEXT_SECONDARY` `#D4D4D4`; glyph `ICON_QUATERNARY` `#737373` | `BG_HOVER` `#171717`; label `TEXT_SECONDARY_HOVER` `#E5E5E5`; glyph `ICON_QUATERNARY_HOVER` `#A3A3A3` | `BG_SELECTED` `#262626` (was `BG_SECONDARY`); label `TEXT_SECONDARY_HOVER`; glyph `ICON_QUATERNARY_HOVER` | Focused: C's ring `FOCUS_RING` 2 px at gap 2 |
| `Tree/Row` (layers) | no fill; label `TEXT_TERTIARY` `#A3A3A3`; glyphs (chevron, kind icon) `ICON_QUATERNARY` | `BG_HOVER`; label and glyphs unchanged (C) | `BG_SELECTED`; label and glyphs unchanged (C) | Selected + hover and canvas hover on a selected row: `BG_SELECTED_HOVER` (`#262626`, contract row 9: the kit keeps the current fill under the pointer); child of a selected layer: `BG_SELECTED_SECONDARY` (`#171717`, row 10); canvas hover: the 1 px `BORDER_SELECTED` outline (decision 1); Focused: none (C says n/a; layer rows take no keyboard focus today) |
| `Button/Icon` xs (panel actions) | no fill; glyph `ICON_QUATERNARY` | `BG_HOVER`; glyph `ICON_QUATERNARY_HOVER` | `BG_SELECTED`; glyph `ICON_SECONDARY_HOVER` (page search while Find is open, `left.rs:90`) | Pressed `BG_PRESSED` with `ICON_QUATERNARY_HOVER` |
| Eye and lock toggles | glyph `ICON_QUATERNARY`; hidden or locked rows show it without hover, as today | glyph `ICON_QUATERNARY_HOVER` | unchanged | n/a |

Hidden layers keep the Tree/Row label colour (`TEXT_TERTIARY`); the eye-off icon carries the state, and a hidden component-like layer drops its cyan for `TEXT_TERTIARY` (name and type label together, as today). Under C's table a hidden plain layer now looks like a visible one except for its icon; that collision is sent to C (section 12). `ICON_QUATERNARY` never sits on `BG_PRESSED` (2.19:1, contract rule).

4. **States.** Rows show Default, Hover, Selected and, for layers, Selected-Hover, Child-of-selected and Canvas-hover; page rows add C's Focused ring. Hidden and locked are content states on top of these. Disabled has no row form today and gets none. The pointer-hover outline on layer rows goes (C's hover fill replaces it, decision 1).
5. **Sizes (R5) and their layout effect.** Width 240, region order and positions stay (R1).

| Element | Constant / code | Today | Target | Δ | Layout effect |
|---|---|---|---|---|---|
| File header | `theme.rs:277` | 65 | 65 | 0 | the name moves to text-sm 14/20 semibold, centred at y + 24 (`left.rs:387`): the 20 px line spans y + 14 to y + 34 inside the 65 px header |
| Section title rows | `PANEL_TITLE_ROW_HEIGHT`, `LAYER_HEADER_HEIGHT` | 40 | 40 | 0 | title type 11 → 14 |
| Panel action buttons (hide-sidebar, search, `+`, collapse-all) | local names in `left.rs` (new, not contract): `LEFT_ACTION_BUTTON` = `CONTROL_XS` 32, `LEFT_ACTION_PITCH` = `LEFT_ACTION_BUTTON` + `SPACE_FACT` = 36 (replaces the frozen `PAGES_SEARCH_DX` 28 = 24 + 4 at `left.rs:89` and `:192`), `LEFT_ACTION_INSET_Y` = (`PANEL_TITLE_ROW_HEIGHT` − `LEFT_ACTION_BUTTON`) / 2 = 4 (replaces the literal 8 at `left.rs:83` and `:181`); `PANEL_ACTION_BUTTON` stays 24 | 24 | 32 | +8 width and height | arithmetic in the table below; titles keep their position and order |
| Page rows | `PAGE_ROW_HEIGHT` (`theme.rs:287`; B sets it to `CONTROL_XS`, contract (c) row 310) | 24 | 32 | +8 per row | the Pages list grows by 8 × visible rows, at most 5: +40 px; Layers starts up to 40 px lower and shows about one fewer row at 1088 px (decision 3); the separator's min, default and max follow (`left_sep.rs:35-46`) |
| Layer rows | `theme.rs:301` | 32 | 32 | 0 | none |
| Filter row and field | `left.rs:200`, `:203`; `filter.rs:66` | row 32, field 28 | row 32, `Input/Search` 32 (`CONTROL_XS`) | 0 | the field fills the row height (vertical inset 2 → 0); horizontal inset 8 stays |
| Filter clear button hit box | `filter.rs:70` | 20×20 | 24×24 (`TARGET_MIN`, P3 floor); glyph 12 unchanged | 0 | none: the box fits the 32 px field; the text column ends 2 px left of it, as today |
| Pages/Layers separator hit band | `hit_rect_for` (`left_sep.rs:48-54`), `PAGES_SEPARATOR_HIT` 6 | 6 tall, full width, recorded as `pages-layers-separator` | `DRAG_DIVIDER_HIT` 24 (spec D's constant, = `TARGET_MIN`) centred on the line, full width, recorded as the new name `pages-layers-separator-band`; `pages-layers-separator` stays the 6 px core | +18 hit | none painted; priorities in rule 8 |
| Row names, prefix, second line, type labels | `font_body` 11, `font_caption` 10 | 10–11 | text-xs 12/18 (`FONT_SIZE_BODY` = `FONT_SIZE_CAPTION` = 12, contract (c) rows 324–325) | 0 | names elide about 8 % earlier at the fixed name column (`layers.rs:659`); the type label hides sooner (`split_row_text`) |
| Dev section | `left.rs:111`, `:157` | 52 / 54 | 52 / 54 (region); count badge `Badge/StatusCount` 22 inside the 28 px title line; button stays 24 | 0 | none; the 24 px button is below C's xs 32, sent to G and C because the 52 px block is a region (R1) |

6. **Radii.** Row pills, inline fields and the filter field take `RADIUS_CONTROL` 8 (R5 "radius md 8", the coordinator's ruling on row and pill radius; C's `Tree/Row` and `List/Row` say radius-sm 6, sent to C). The rename field's `RADIUS_MARK` 2 becomes `RADIUS_CONTROL`. The match highlight keeps `RADIUS_MARK` (it marks characters). Nested shapes stay concentric (P4).
7. **No behaviour change.** Click, double-click, shift/cmd-click, Alt-click on a chevron, drag and drop, rename with Tab, filter and keyboard flow stay. Widget ids recorded through `widgets::record` (`left-dev-ready-section`, `left-dev-mark-ready`, `layers-search`, `layers-collapse-all`, `layers-filter`) stay. Hit boxes change only where R5 or the 24 px floor requires it (panel action buttons, filter clear button).

**Cluster arithmetic** (x from the panel's left edge; the default panel is 240 wide and its inner area 239, `left.rs:30`; the right pad is today's, so every cluster keeps its right edge and grows to the left):

| Cluster | Today (24 px, pitch 28) | After (32 px, pitch 36) | Gap and edge | Overlap check |
|---|---|---|---|---|
| Hide sidebar, file header (`left.rs:68-74`) | x 203..227, y 12..36 (centred on the file-name line, y 24) | x 195..227, y 8..40 (`LEFT_SIDEBAR_BTN_RIGHT_PAD` 12 kept; top = `LEFT_ACTION_Y` 12 − 4) | 12 to the inner edge, as today | the name column `HEADER_NAME_W` = `LEFT_PANEL_WIDTH` − 16 − (12 + 32) − 24 = 156 (was 164): name text 16..172; the name's hit rect (`left.rs:399`) ends at min(172 + 28, 195 − 4) = 191, so it no longer reaches under the button (today it overruns it by 5: 208 against 203); the rename field ends at 8 + 156 + 16 = 180; the sublabel line is far left of x 195 |
| Pages: `+`, search (`left.rs:82-96`) | `+` 207..231, search 179..203, y 8..32 | `+` 199..231, search 163..195, y 4..36 | 4 between the two, 8 to the inner edge | "Pages" at 14 Semibold is 41.7 wide (bundled Inter, advance widths): x 16..57.7, so 105 px stay free left of x 163 |
| Layers: filter, collapse-all (`left.rs:180-197`) | filter 207..231, collapse 179..203 | filter 199..231, collapse 163..195 | as above | "Layers" 45.2 wide: x 16..61.2, 102 px free; with the Dev tabs see edge case 10 |

No button reaches past x 239, none overlaps its neighbour (4 px gap) or a title, and the two-button clusters are 68 wide (were 52). The second button's offset is derived (`LEFT_ACTION_PITCH`), so `PAGES_SEARCH_DX` is not edited (contract appendix: frozen at 28).

8. **Drag-divider hit band (plan.md C-R; the same rule is in D rule 3).** The separator's visible line stays (1 px `BORDER`, 2 px `BG_BRAND` on hover and drag). Its hit band is `DRAG_DIVIDER_HIT` 24 centred on the line: [list bottom − 5, list bottom + 19]. Priorities: (1) the 6 px core (`PAGES_SEPARATOR_HIT`, [list bottom + 4, list bottom + 10]) keeps winning as today; (2) outside the core, a scrollbar track (the Pages list's, 8 px at x 231..239) and every control keep their own hit area: the Layers header buttons start at list bottom + 12, so the band's last 7 px over x 163..231 belong to them; (3) the band wins over list rows and over non-interactive paint: the last 5 px of the last visible Pages row become the divider (the row keeps 27 of its 32 px) and the 8 px gap and the Layers title area are free. Free band: 24 px over x 0..163, 17 px over x 163..231 (above the buttons), 19 px over x 231..239 when the Pages list scrolls (24 when it does not). The raw press test must not rely on `hit.contains(pos)` alone. `widgets::record` is a test-only hook (a no-op in release builds, `widgets/mod.rs:290`), so production code cannot read recorded rects; the press test instead excludes rects computed from the constants that place them: the Pages scrollbar track (`list.right() − 8 .. list.right()`, only when the list scrolls), the Layers header buttons (from `LEFT_ACTION_BUTTON`, `LEFT_ACTION_PITCH` and `LEFT_ACTION_INSET_Y`) and, in Dev mode, the Mark-ready button. The recorded `-band` names are for tests and L only. The tests below fix the outcome.

## 4. Data model / schema changes

None.

## 5. Contracts

**File ownership across D–K.** E owns:

| File | Owner | Shared surface and rule |
|---|---|---|
| `crates/designos-app/src/panels/left.rs` | E | the Agents/Tools branch (`:32-56`) is spec I's surface and the Assets branch (`:58-61`) spec K's: E does not restyle them. The Dev section (`:107-159`), the page-row Ready icon (`:306-309`) and the Dev tabs call (`dev::list::tabs`, `:170-171`) are Dev surfaces: E applies the Dev accent (`DEV_READY`, R10.4) in this file (G cites the same block as `:109-177`). Find takes the panel at `:162-165` (J). #1108's separator is called from this file (`:99-105`) and lives in `left_sep.rs`. The hide-sidebar button pairs with D's show-sidebar button (`shell.rs:1667-1672`): both are `Button/Icon` xs 32, E's through the local `LEFT_ACTION_BUTTON`, D's through `CONTROL_XS`; neither touches `PANEL_ACTION_BUTTON` |
| `crates/designos-app/src/panels/left_sep.rs` | E (plan.md 22:55) | #1108's separator: E restyles the hit band, the focus ring and the colours and updates the test that asserts the `BORDER_SELECTED` outline and extends the one that asserts the 6 px rect (`shell/tests/pages_layers_separator.rs`); the drag, clamp, persistence, keyboard and AccessKit behaviour stay as #1108 ships them. Its band follows the divider rule shared with D |
| `crates/designos-app/src/panels/layers.rs` | E | the image-job progress chip in a row (`:651`, `crate::image_jobs`) is drawn by `image_jobs.rs`, which spec I owns (plan.md 22:20: status chip and inline loading restyle as C `Tag/Chip` and `Loading/Inline`); E only hosts it, and the chip keeps its current look until I lands |
| `crates/designos-app/src/find/filter.rs` | E | `FIND_HIGHLIGHT` is shared with J's `find/panel.rs`; both keep it (contract row 74) |

Not mine: `theme.rs`, `design/*` (A, B); `widgets/*` and `panels/mod.rs` (C; `widgets/picker.rs` is F's); `icons.rs` (C); `tabs.rs`, `shell.rs`, `shell/toast_layout.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); right-panel files (F); `dev/*` including `dev/list.rs` (Ready-for-dev list and tabs drawn in the left panel) and `dev/code_view.rs`, `specs.rs` (G); `terminal.rs`, `agent_*.rs`, `image_jobs.rs` (I); `find/panel.rs`, `find/mod.rs`, `find/select.rs`, `panels/context.rs` (J); `panels/assets.rs`, `panels/check_designs.rs` (K).

**Colour-literal gate.** Spec A rule 13 adds `scripts/check_chrome_colour_literals.py` (new name) with spec D's regex and a falling baseline; until it exists, the section 2 command must print `chrome_sites=0` for the three files.

## 6. File-format mapping

None. The Pages height chosen by #1108 is a preference, not file content (#1108 decides where it persists).

## 7. UI

- Read the in-repo design sources first: `design/principles.md`, `design/soul.md`, `specs/token-contract.md` (A), spec B's scale and spec C's widget kit; they win over generic defaults. The Purple Ban stays (R4). Contributors who use the project's agent kit may also load its `es-designer` skill; it is not required.
- Component, instance and component-set rows stay cyan (controller default; P2 names cyan the component colour).
- Copy is unchanged; Z16 stays green.
- Evidence goes into the PR body (not Figma, R3). Responsive: SKIPPED (owner default).

## 8. Edge cases

1. A file with more than 5 pages: the list scrolls inside 5 × 32 = 160 px (was 120). The scrollbar keeps working (`left.rs:318`, C's scrollbar).
2. With #1108 merged, the user's dragged Pages height is in pixels; after rows grow to 32 a stored height that fitted 5 rows of 24 shows 3.75 rows of 32. #1108's clamps (at least 1 row plus header, at most the content height) still apply; E re-checks the minimum and default with the 32 px row.
3. Long layer names at 12 px: the name elides first, then the type label hides (`split_row_text`); neither overlaps the eye and lock toggles (`layers.rs:590-592`).
4. A selected component row: the selected pill is neutral and the name stays cyan.
5. Canvas hover on a selected row: Selected-Hover fill, not the orange outline (today's order, `layers.rs:519-530`).
6. Hidden and locked together: eye-off and lock both stay visible without hover, as today.
7. Filter on: drag and drop is off (`layers.rs:411-412`, `start_row_drag` refuses while filtering); matched characters keep the `FIND_HIGHLIGHT` mark under the selected pill.
8. Dev mode with Ready rows: the Layers header swaps to the Dev tabs (`left.rs:170-174`, spec G); the Layers rows below keep E's styling.
9. Panel action buttons at 32 px: the arithmetic is in the cluster table under rule 7 (68 px clusters, titles 105 and 102 px clear). At the narrowest panel (`left_panel_width` clamp 200) the clusters sit at x 123..191, the titles end at x 61, and 62 px stay free.
10. Dev mode with Ready rows swaps the Layers title for `dev::list::tabs` (G's file, drawn in E's row). Measured on the bundled Inter at 12 px Semibold (`font_body_strong` after B): "Ready for dev" 80.3, "Layers" 38.8, one-digit count badge 18 wide (`dev/list.rs:46-50`: label + 6 + badge + 14 padding), so the strip runs x 16..191 (two-digit count: 197). Today's strip at 11 px ends at 181 against a collapse-all button at 179 (the text clears it by 5); the cluster now starts at 163, so the strip is 28 px too wide (34 with two digits). The left panel's 240 px cannot hold both at 12 px and 32 px buttons. **Rule (decision 6, shared with G):** the strip is clamped to `cluster_left − PANEL_PADDING_X − SPACE_FACT` = 163 − 16 − 4 = 143 px; when it does not fit, the count badge is dropped first (saves 24), then the "Ready for dev" label elides on a character boundary with an ellipsis (about 72 px of label, "Ready for d…"); a tab never paints under a button. Writer's decision, the owner may reverse it; the alternative is to move the collapse-all button out of the Dev state, which removes a control. Red test below.
11. The separator on a one-page file: Pages is one row (32), `list_h` 32; the band [list bottom − 5, list bottom + 19] overlaps the single row's last 5 px; a press there drags the divider, which clamps to min = max and does nothing (as #1108 does today for a click on the line).

## 9. Non-goals

- The Pages/Layers separator's resize logic, persistence and keyboard handling (#1108: E restyles its band, focus ring and colours only); the agent terminal in the left panel (I); Assets (K); Find and replace (J); the Ready-for-dev list body (`dev/list.rs`, G); the image-job chip (`image_jobs.rs`, spec I: its status chip and inline loading; E only hosts it).
- Canvas overlays, document content and icon art (R6).
- Changing the left panel width (240) or the shell's resize handle (spec D owns it; its 24 px band is D's rule 3).

## 10. Test plan (red first)

Acceptance criteria:

1. The section 2 command (or A's gate) prints `chrome_sites=0` for `panels/left.rs`, `panels/layers.rs` and `find/filter.rs`.
2. `python3 scripts/check_spacing_relationships.py` passes; per-file counts stay at or below `scripts/spacing_baseline.json`.
3. Spec L's layout gate shows the same regions, panel width and control order as the baseline; every size or position change is covered by an entry of the "L allow entries" table in section 10 and by nothing else: the Pages list (+8 per visible page row, at most +40) and what follows from it, the four panel action buttons (+8 × +8 and the cluster offsets), the filter field (28 → 32) and its clear button (20 → 24), and the separator's hit band (6 → 24, recorded under a new name).
4. Every constant in section 3 is a token-contract constant or a new name A has added; `theme::` tests pass with B's updated invariants.
5. `python3 scripts/zero_fake_ui_gate.py` and `python3 scripts/design_os_self_evolve.py` pass.
6. **Contrast.** `ui ds a11y` is not accepted: on this store it measured 0 pairs (false green, plan.md and contract (f)). The check is three parts:
   - (a) The contract's C-1 check (token-contract (f)) exits 0. Run 2026-10-28 from the specs folder against the contract as on disk on 2026-10-28 (23:19) and a scratch `theme.rs` built from `8dfaee93` with the contract's rows applied (C-1.1 to C-1.62 minus the retired C-1.25 and C-1.26): exit 0, "every C-1 row (62/62 on 2026-10-28; rerun against the final contract) pass, 4/4 exception floors held, 0 failure(s)". E relies on C-1.1 (titles, file name, typed text), C-1.5 (`List/Row` label), C-1.7 (`Tree/Row` label, prefix, second line, group labels, type labels), C-1.9 (`Tree/Row` label on a selected row), C-1.10 ("Save to library"), E-1.a (filter placeholder, R8.1 exception floor; C-1.25 is retired), C-1.29 (focus ring) and C-1.36 (`DEV_READY` on `BG`: Dev section icon and the Ready icon on a page row, R10.4).
   - (b) The E-P rows below, in the C-1 row shape, pass the contract's snippet with only the id prefix and the file changed. Run 2026-10-28 from the specs folder: exit 0, 20 rows ok (lowest non-text row E-P.13 3.19:1; lowest text row E-P.9 4.91:1). E-P.19 and E-P.20 are the focus ring (separator, page rows) and the gray zero count.
   - (c) A adds the E-P rows to C-1 and to the `chrome_contrast_pairs` test (spec A section 10). Until then E's PR runs (b).
7. Before/after captures of every command below are in the PR with exit codes.
8. The filter clear button's hit box and the separator's hit band are at least `TARGET_MIN` (red tests below).

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
| E-P.19 | `FOCUS_RING` #F97316 | `BG` #0A0A0A | 3.0 | separator focus ring, page-row focus ring (= C-1.29) |
| E-P.20 | `UTILITY_NEUTRAL_700` #D4D4D4 | `UTILITY_NEUTRAL_50` #171717 | 4.5 | gray zero count (= C-1.24) |

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
| new `panels::left::tests::pages_list_is_five_rows_of_the_row_height` (new name; `left.rs` has no test module at `8dfaee93`; #1108's tests are in `shell/tests/pages_layers_separator.rs`) | the list rect is `min(n, 5) × PAGE_ROW_HEIGHT`, and `PAGE_ROW_HEIGHT` equals `CONTROL_XS` (32, contract (c) row 310, B's change) | red only if E lands before B; it guards B's value afterwards |
| new `find::filter::tests::match_highlight_uses_find_highlight` (new name) | `paint_highlights` fills with `theme::FIND_HIGHLIGHT` | guard (passes today; pins contract row 74) |
| new `find::filter::tests::clear_button_hit_box_meets_the_target_floor` (new name) | the clear button's interact rect is at least `TARGET_MIN` on both axes | yes (20) |
| #1108's separator tests (`shell/tests/pages_layers_separator.rs`) | the drag, clamp, persistence, double-click and slow-frame tests use `PAGE_ROW_HEIGHT` and pass with the 32 px row; one changes: `keyboard_navigation_steps_clamps_and_resets_with_accesskit_and_focus_ring` (`:187-200`) asserts a `BORDER_SELECTED` outline within 2 px of the recorded `pages-layers-separator` rect (now a `FOCUS_RING` ring around the 6 px core, 2 to 4 px outside it); `dragging_the_separator_changes_the_pages_and_layers_split` (`:42-60`) keeps passing because `pages-layers-separator` stays the 6 px core, and gains an assertion that `pages-layers-separator-band` is `DRAG_DIVIDER_HIT` tall; the stale message strings ("4 rows (96 px)", "24 px") are corrected | per #1108 and E |
| new `shell::tests::left_panel_actions_fit_their_rows_and_the_panel_edge` (new name; a new `shell/tests/left_panel_actions.rs` beside the separator tests) | the recorded `sidebar`, `page-add`, `page-search`, `layers-search` and `layers-collapse-all` rects are 32×32; the two-button clusters have a 36 px pitch, a 4 px gap and their right edge 8 px (header: 12 px) from the inner edge; none overlaps the file-name hit rect, a title text rect or the panel edge | yes (24×24, pitch 28) |
| new `dev::list::tests::tabs_never_paint_under_the_layers_actions` (new name; G's file, E's rule 10) | with a two-digit count the strip is at most 143 px wide, the badge is dropped before the label elides, and no tab rect overlaps `layers-search` or `layers-collapse-all` | yes (the strip is 197 wide) |
| new `shell::tests::separator_band_yields_to_controls_and_scrollbars` (new name) | a press 10 px below the line over the Layers search button is a button press; a press 10 px above the line over the Pages scrollbar thumb scrolls; a press 10 px below the line at x 60 drags the divider; a press at the line over a Pages row drags it | yes (6 px band) |

Run: `cargo test -p designos-app --bin designos panels::layers`, `cargo test -p designos-app --bin designos panels::left`, `cargo test -p designos-app --bin designos find::filter`, `cargo test -p designos-app --bin designos theme::`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**L allow entries E needs** (L's schema; `spec: "E"`; the right-anchored clusters grow to the left, so L's derived cumulative shift moves each neighbour, and only the sizes and the pitch are exact entries):

| Target | Entry | Arithmetic |
|---|---|---|
| Page rows (unnamed) | `h_base: 24`, `region: left_panel`, `max_dh: 8` | 24 → 32, up to 5 visible: +40, and everything below shifts by the derived sum |
| `sidebar` (hide-sidebar) | `anchor_x: right`, `anchor_y: centre`, `max_dw: 8`, `max_dh: 8` | 24 → 32 around the same centre line (y 24) |
| `page-add`, `layers-search` | `anchor_x: right`, `anchor_y: centre`, `max_dw: 8`, `max_dh: 8` | 24 → 32 in the 40 px row, right edge fixed |
| `page-search`, `layers-collapse-all` | `anchor_x: right`, `anchor_y: centre`, `max_dw: 8`, `max_dh: 8` | 24 → 32; the right edge moves left by the derived 8 (the `+` grew 8 to the left) and the pitch stays button + 4 |
| Filter clear button (unnamed) | `h_base: 20`, `region: left_panel`, `max_dw: 4`, `max_dh: 4` | 20 → 24 |
| `pages-layers-separator`, `pages-layers-separator-band` | none: the first stays the 6 px core; the second is a new name, reported, not baselined | hit 6 → 24 around the same line |

The filter field is not a recorded rect (`layers-filter` is the 32 px row, unchanged). The file-name hit rect (`file-header`) shrinks on the right by at most 17 px when a name is long (clamp at x 191); L's `max_dw` for it is 0 and its left edge is fixed, so give it `min_dw: -17`, citing R5 (the 32 px button needs the room).

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

No capture was rerun at `8dfaee93`; the recorded runs are from 0.6.x builds. v0.7.0 changed `--select` (it takes `id:`, `fig:`, `name:` or a layer name and exits 2 on a miss) and the screenshot exit paths, so the implementer reruns the list on v0.7.0 (`tools/designos-latest`, an idle machine) and records the exit codes in the PR; E-04 is the one that failed with exit 1 in the earlier runs.

Not capturable from the CLI today: hover, keyboard focus, multi-select (`--select` takes one name), locked rows, the layer filter row (no flag opens it), inline rename and drag indicators. The PR covers them with the paint tests above and a manual window capture, labelled manual.

Census output pasted from the run at `8dfaee93` (the section 2 command, from the root of `git archive 8dfaee93 crates/designos-app/src scripts/rust_literal_scan.py`, 2026-10-28, exit 0):

```text
panels/left.rs theme_lines=75 chrome_sites=2 [114, 307] computed=[]
panels/layers.rs theme_lines=52 chrome_sites=0 [] computed=[]
find/filter.rs theme_lines=6 chrome_sites=0 [] computed=[]
exit 0
```

## 11. Performance budgets

- The layers list stays virtualised (`layers.rs:467-469`: only rows in view are painted); a taller page row does not change the per-frame row count of the layers list.
- No allocation added per row; constants are `const`. No `request_repaint` added; idle CPU stays 0 % (CLAUDE.md section 4). `rows_cache` invalidation rules (`layers.rs:300-320`) are unchanged.

## 12. Dependencies and build order

- A, B, C first; L's baselines before the PR. Then: constants in `left.rs` and `layers.rs`, then `find/filter.rs`, then R5 sizes (page row 32, action buttons 32, type 12/14), the test updates, then evidence.
- **B.** `PAGE_ROW_HEIGHT` → `CONTROL_XS` 32 (contract (c) row 310); B changes neither `PANEL_ACTION_BUTTON` nor `PAGES_SEARCH_DX` (B rule 6: "a surface that needs another size or offset declares a local name in its own file (spec E for the left panel)"), `FONT_SIZE_BODY` = `FONT_SIZE_CAPTION` = 12, `FONT_SIZE_LARGE` 14. B's type change breaks the six layers tests listed in section 10 until E's helper lands, so B and E land together or E's test update lands first.
- **#1108 (Pages/Layers separator) is merged** (`07b961c8`, after `205c72ef`). It edits `panels/left.rs`, adds `panels/left_sep.rs` (E's file since plan.md 22:55), `theme.rs` layout constants, a brand-colour drag highlight and a focus outline. E rebases on it, keeps the `BG_BRAND` highlight (already R4's brand), turns the focus outline into C's S4 ring (`FOCUS_RING`), widens the hit band to 24 px (rule 8) and uses the 32 px row for the Pages minimum and default heights.
- **#1106 (icon-only rail).** No E file; only the panel's x position follows the rail width (handled by D and L).
- **G.** The Dev accent role (value `DEV_READY` `#14AE5C`, R10.4) and the count-badge rule.
- **Sent to C** (open items, not owner questions): `Tree/Row` hidden text equals its default text, so a hidden plain layer is told apart only by its icon; `Tree/Row` Focused is n/a while `List/Row` has a ring; `Tree/Row` has no child-of-selected state (E uses `BG_SELECTED_SECONDARY`, contract row 10); row radius 6 in C vs 8 by R5 (E uses 8); the 24 px "Mark selection ready" button inside the 52 px Dev region.
- **Sent to A (new names):** `DEV_TINT_REST` (0.16), `DEV_TINT_HOVER` (0.24), `TYPE_LABEL_COMPONENT_ALPHA` (0.75), `DEV_STATUS_TINT` (0.22, shared with G); `PAGES_SEARCH_DX` keeps its frozen value 28 but loses its only two readers (`left.rs:89`, `:192`), so the bin target would warn `dead_code` and `clippy -D warnings` would fail: allow it with `#[allow(dead_code)]` (A rule 21's staged pattern) or delete it (the contract freezes the value, not the name); the E-P rows for C-1 and `chrome_contrast_pairs`. E uses spec D's `DRAG_DIVIDER_HIT` (24.0, declared by D, not by A or B). The `left.rs`-local names (`LEFT_ACTION_BUTTON`, `LEFT_ACTION_PITCH`, `LEFT_ACTION_INSET_Y`) are expressions of contract constants, not new values, and need no contract row.
- **Controller:** rule 10 (the Dev tab strip against the 32 px Layers cluster) needs G and E to agree; the owner may prefer another resolution.
- **Open PRs touching E files** (`gh pr list --state open --json files`, 2026-10-28): #1186 `fix/issue-1173-changed-layers-scroll` (`panels/left.rs`: the Agents branch, spec I's surface, near `agent_card_reserve`); stale #770, #768, #765 (`panels/left.rs`, `panels/layers.rs`). Rebase on #1186 if it merges first.

## 13. Owner rulings that apply

- R1 (layout unchanged), R2 (Untitled spacing and type), R3 (no Figma), R4 (Studio orange, dark label, focus ring Orange-500 `#F97316`, Purple Ban), R5 (text-xs 12/18 rows and labels, text-sm 14/20 panel titles, button xs 32, row 32, radius-md 8), R6 (keep icons), R7 (spec-driven).
- R10.4 Dev accent `DEV_READY` `#14AE5C` (Dev section icon, Ready icon, count badge, Mark-ready button).
- R8.1 placeholder `#737373` (filter field); R8.2 selected `#262626` (page and layer rows); R8.3 accent text `#F97316` ("Save to library"); R8.4 field 32 (filter field); R8.6 row and label text stays 12. R8.5 and R8.7 do not apply (no floating chrome and no destructive button in E's files). R9: figma-agent read-only is allowed; not used here.
- Controller defaults: component/instance cyan stays; canvas overlays keep Figma-parity colours. 21:35 sweep: `FIND_HIGHLIGHT` stays info blue.
- Plan.md C-R (22:55): `PANEL_ACTION_BUTTON` stays 24 and `PAGES_SEARCH_DX` stays frozen, E uses local names; drag dividers keep their visible line and get a 24 px hit band, no new exception; where specs disagree on behaviour (Dev count badge at zero) today's behaviour wins; `panels/left_sep.rs` is E's (22:55); text fields and selects join E-2 and draw no ring (C S4).
- 2026-09-22: visual evidence in every PR. Standing: protected `main`. 2026-10-10: responsive SKIPPED by default.

## Decisions recorded by the spec writer (technical; owner may reverse)

1. The pointer-hover outline on layer rows becomes C's hover fill, so pages and layers hover the same way; the canvas-hover outline stays as an outline (it marks a different source of hover).
2. Page rows take the R5 row (32) like layer rows; the five-row cap stays. A five-page list is 40 px taller and pushes Layers down; #1108 lets the user drag the split (R5 names rows 32; plan.md: heights may grow where R5 raises a control).
3. The "Mark selection ready" button keeps its Dev-tinted look with named alphas; G's count-badge rule covers the count next to it.
4. The Pages/Layers separator's hit band is 24 px with the priorities of rule 8: the 6 px core keeps winning, the extension yields to controls and scrollbars and wins over list rows. It reads C-R's "centred on the line" without taking a pixel from a scrollbar or a button.
5. The separator's focus ring is drawn around the 6 px core, not the 24 px band: a 24 px outlined box would read as a panel, and the core is what the user sees highlight on hover.
6. The Dev tab strip yields to the 32 px Layers cluster (edge case 10): badge first, then the label elides. Shared with G; the owner may reverse it.
7. Panel action buttons are 32 through `left.rs`-local names derived from contract constants; B and the contract change nothing.

## Open questions for the owner

None. The panel button size is decided by R5 and C's `Panel/Header`, sized locally (C-R); the Pages height follows from R5 (decision 2); the match highlight follows the contract; the Dev accent is `DEV_READY` (R10.4); the divider hit band is C-R.

## Changelog (fix round 2026-10-25, review `reviews/DE-curator-review.md`)

| Review item | Resolution |
|---|---|
| E1 row roles vs C | Fixed. Pages are C `List/Row`, layers C `Tree/Row`, states taken verbatim (layer labels `TEXT_TERTIARY` in every state); the hidden-name collision, row focus, the child-of-selected state and the 6-vs-8 radius are sent to C (§12); E uses radius 8 per the coordinator's R5 ruling. |
| E2 decision 2 and the filter highlight | Fixed. Decision 2 deleted; the row is `FIND_HIGHLIGHT` unchanged (contract row 74, 21:35 sweep); the new filter test asserts `theme::FIND_HIGHLIGHT`. |
| E3 open question 1 | Decided: `Button/Icon` xs 32 (R5 and C `Panel/Header`); superseded 2026-10-28 by ruling C-R: `PANEL_ACTION_BUTTON` stays 24 and `PAGES_SEARCH_DX` stays frozen; E uses local names in `left.rs` (rows E:124-132). |
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

## Changelog (fix round 2026-10-28, review `reviews/DEFG-curator-review.md`, plan.md C-R)

| Review item | Resolution |
|---|---|
| E1 blocker: panel-button size rests on a B change B refuses | Fixed. The four buttons are sized by `left.rs`-local names (`LEFT_ACTION_BUTTON` = `CONTROL_XS`, `LEFT_ACTION_PITCH` = 36, `LEFT_ACTION_INSET_Y` = 4); `PANEL_ACTION_BUTTON` stays 24 and `PAGES_SEARCH_DX` stays frozen. §1, the Depends row, rule 5, §12 and the old changelog entry E3 are rewritten; the cluster table shows x, width, gap, overlap and the panel edge for the header, Pages and Layers; D is told (its show-sidebar button takes the same 32 px straight from the contract's xs step). The arithmetic found one more collision, the Dev tab strip (edge case 10, decision 6). |
| E2 `panels/left_sep.rs` unowned | Fixed. Listed in Touches, §2, §5 and the tests; cited at `8dfaee93`; its focus becomes C's S4 ring (decision 5). |
| E3 separator hit zone 6 px | Ruled by plan.md C-R: 24 px band, rule 8 (priorities and free widths), three tests, `DRAG_DIVIDER_HIT` requested. |
| E4 focus look contradicts C | Fixed. The rename field and the filter field draw a 2 px `BORDER_SELECTED` border and no ring (C S4). |
| E5 Dev count badge at zero | Fixed to today's behaviour (R1, C-R): the section count is always painted, gray at zero; G's tab badge hides at zero; the two counters differ today. One alpha, `DEV_STATUS_TINT` 0.22, shared with G. |
| E6 image-job chip called unowned | Fixed: spec I owns `image_jobs.rs`; E hosts the chip. |
| E7 AC3 says only the Pages list changes | Fixed. AC3 lists every rule-5 change and points at the "L allow entries" table. |
| Re-anchor to `8dfaee93` | 54 of 126 cites moved and were verified line for line; drift note above; #1108's two tests change with E. |
