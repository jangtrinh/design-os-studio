# Project docs workspace: UI mockups (M01 to M13b, 23 screens)

Open **[index.html](index.html)** first: left list of every mockup with its spec section, the selected mockup in an iframe scaled to fit (CSS transform), Prev / Next buttons, Left / Right arrow keys, `#m07`-style deep links (`#m09full` is the tall M09 page), and an "Open full size" link. Works from `file://`, no server, no network. Capture: [png/index.png](png/index.png).

Static HTML/CSS reference for the approved UI spec ([part 1](../ui-spec-ported-from-design-studio.md), [part 2](../ui-spec-part-2-editor-dialogs-and-states.md)). Authority order (ruling R0, [ui-polish-rulings-r1.md](../ui-polish-rulings-r1.md)): Studio's widget kit (`crates/designos-app/src/widgets/kit/*.rs`) and `theme.rs` beat the spec; the specs were corrected to the kit in the same change. Captures are 1440 x 900 headless Chrome.

Shared files: [tokens.css](tokens.css) (the only file with colour literals; every token carries its `theme.rs` name in a comment), [shell.css](shell.css) (layout and components, no colour literals, no shadows). Fonts: Inter and Roboto Mono from `crates/designos-fonts/assets/` (paths relative to this folder).

**Icons.** Phosphor regular (the set Studio vendors, `@phosphor-icons/core` 2.1.1), inline SVG filled paths on a 256 viewBox, 16 px (rail 20, small 10 to 14). The first brief said "Lucide-style"; Phosphor was used instead because the kit's glyphs are Phosphor, so the shapes match the product 1:1. Say so if Lucide was a hard requirement.

## Mockups

| ID | HTML | PNG | Spec section |
|---|---|---|---|
| M01 Home, All folders, open project | [m01-home-open-project.html](m01-home-open-project.html) | [png](png/m01-home-open-project.png) | part 2 §1 |
| M01b Home, inside a project (file grid header) | [m01b-home-project-header.html](m01b-home-project-header.html) | [png](png/m01b-home-project-header.png) | part 2 §1 |
| M02 First open, empty states, search with no result | [m02-first-open-empty-states.html](m02-first-open-empty-states.html) | [png](png/m02-first-open-empty-states.png) | part 2 §2 |
| M03 Reading page, block hover, block menu, Turn into | [m03-reading-block-menu.html](m03-reading-block-menu.html) | [png](png/m03-reading-block-menu.png) | part 1 reading page, embed card, On this page; part 2 §3.1 |
| M04 Slash menu filtered "ta" + no match | [m04-slash-menu-filtered.html](m04-slash-menu-filtered.html) | [png](png/m04-slash-menu-filtered.png) | part 2 §3.2 |
| M04b Slash menu unfiltered, top of the list | [m04b-slash-menu-unfiltered.html](m04b-slash-menu-unfiltered.html) | [png](png/m04b-slash-menu-unfiltered.png) | part 2 §3.2 |
| M04c Slash menu unfiltered, scrolled to the end (ruling T2) | [m04c-slash-menu-scrolled-to-end.html](m04c-slash-menu-scrolled-to-end.html) | [png](png/m04c-slash-menu-scrolled-to-end.png) | part 2 §3.2 |
| M05 Inline toolbar, link popover, existing link | [m05-inline-toolbar-link-popover.html](m05-inline-toolbar-link-popover.html) | [png](png/m05-inline-toolbar-link-popover.png) | part 2 §3.3 |
| M06 Table editing | [m06-table-editing.html](m06-table-editing.html) | [png](png/m06-table-editing.png) | part 2 §4 |
| M06b Table scrolled to the end, add-column + (ruling S4) | [m06b-table-scrolled-to-end.html](m06b-table-scrolled-to-end.html) | [png](png/m06b-table-scrolled-to-end.png) | part 2 §4 |
| M07 Pin frame modal | [m07-pin-frame-modal.html](m07-pin-frame-modal.html) | [png](png/m07-pin-frame-modal.png) | part 2 §5 |
| M08 Suggest from canvas | [m08-suggest-from-canvas.html](m08-suggest-from-canvas.html) | [png](png/m08-suggest-from-canvas.png) | part 2 §5 |
| M09 Embed + diagram states (first 900 px) | [m09-embed-and-diagram-states.html](m09-embed-and-diagram-states.html) | [png](png/m09-embed-and-diagram-states.png) | part 2 §6, §7 |
| M09 full page (all states, 1440 x 2630) | [m09-embed-and-diagram-states-full.html](m09-embed-and-diagram-states-full.html) | [png](png/m09-full.png) | part 2 §6, §7 |
| M10 Conflict banner (saves held) | [m10-conflict-banner-saving.html](m10-conflict-banner-saving.html) | [png](png/m10-conflict-banner-saving.png) | part 2 §8 |
| M10b "Saving…" in the tree header | [m10b-saving-header.html](m10b-saving-header.html) | [png](png/m10b-saving-header.png) | part 2 §8 |
| M10c Save failed banner | [m10c-save-failed.html](m10c-save-failed.html) | [png](png/m10c-save-failed.png) | part 2 §8 |
| M11 Agent caret + write flash | [m11-agent-co-editing.html](m11-agent-co-editing.html) | [png](png/m11-agent-co-editing.png) | part 2 §9 |
| M11b Off-screen "3 changes, Show" chip | [m11b-agent-offscreen-chip.html](m11b-agent-offscreen-chip.html) | [png](png/m11b-agent-offscreen-chip.png) | part 2 §9 |
| M11c "Claude is waiting" held chip | [m11c-agent-held-while-typing.html](m11c-agent-held-while-typing.html) | [png](png/m11c-agent-held-while-typing.png) | part 2 §9 |
| M12 Overview, KPI strip, linked spec, Copy cho AI, toast | [m12-overview-kpi-copy-for-ai.html](m12-overview-kpi-copy-for-ai.html) | [png](png/m12-overview-kpi-copy-for-ai.png) | part 2 §10 |
| M13 Tree interactions (rename, drag, context menu, search) | [m13-tree-interactions.html](m13-tree-interactions.html) | [png](png/m13-tree-interactions.png) | part 2 §12 |
| M13b Delete page dialog | [m13b-delete-page-modal.html](m13b-delete-page-modal.html) | [png](png/m13b-delete-page-modal.png) | part 2 §12 |

23 mockups + index = 24 PNGs. 22 mockup PNGs plus `index.png` are 1440 x 900 (checked with `sips`); `m09-full.png` is 1440 x 2630 because the Rendering and Ready cards are 410 px tall under the spec's 420 px render cap (the first brief said 1600; that does not fit all states). Responsive captures (375/768) are SKIPPED by owner default.

## Round 4 fixes (critique r3, 0 HIGH + 3 MED + 6 LOW, rulings T1 to T4)

Source: [critique-261102-project-docs-mockups-r3.md](../../reports/critique-261102-project-docs-mockups-r3.md). Rulings: [ui-polish-rulings-r2.md](../ui-polish-rulings-r2.md) section "Round 3 rulings" (T1 to T4) over S1 to S4 and [ui-polish-rulings-r1.md](../ui-polish-rulings-r1.md) (R0). Edits are in `shell.css`, 11 mockup HTML files, `index.html`, the new `m04c`, the two spec files and this README. No settled item is open any more: S1 (cap 344) was confirmed in ruling S1 confirmation, phase-02 was edited in `c221d9d5`, and the M04b composite is resolved by T2.

### ID ledger

| ID | Change | Status |
|---|---|---|
| N1 (T1) | Table column width follows the rule: widest cell text + 12 + 12 insets (+ the 1 px divider), clamped 120 to 320; longer text wraps and the row grows; no ellipsis. `shell.css` `.tbl th, .tbl td`: `overflow: hidden; white-space: nowrap; text-overflow: ellipsis` replaced by `white-space: normal; overflow-wrap: anywhere; vertical-align: top` (header cells stay `middle`). All 11 table screens (and m04c): `<colgroup>` 160/150/130/140/150/190 to 120/120/120/120/120/320. The Card row Notes sample "3-D Secure when the issuer asks" became "3-D Secure when the issuer asks. A declined card is never retried." (461.7 px natural width, so it hits the 320 cap and wraps to two lines, as T1 asks of M06b). Spec part 2 §4 states the rule, the sample widths and the top alignment; part 1 table row points to it. | FIXED |
| N2 (T2) | M04b = unfiltered menu at the top of its list, one menu. New `m04c-slash-menu-scrolled-to-end.html` = the same menu scrolled to the end (list offset 192, thumb at the bottom of the band, "Table" highlighted), one menu. The second menu and its "Scrolled to end" caption left M04b; M04c carries the caption. `index.html`: M04b renamed "Slash menu, top of the list", M04c "Slash menu, scrolled to end" added after it. Spec §3.2 heading and text name M04c. | FIXED |
| N3 (T3) | Spec part 2 §5: "count pill = the number of frames the grid lists for that file (ruling T3: all frames in Pin and Embed modes, unpinned frames in Suggest mode); it is not the number of page rows". Mockups unchanged (M07 6, M08 1 / 3 / 4). | FIXED |
| N4 | Spec §4: "thumb length = track x viewport / content = 690 x 690 / content width, 499 for the 954 sample". Mockup thumb stays 499 (690 x 690 / 954 = 499.06). | FIXED |
| N5 | `shell.css` `.ftb .fb.txt { line-height: var(--lh-sm) }`: "Turn into" 14/24/600 to 14/20/600 (spec §3.3 already said 14/20). Button box 92.1 x 32 and toolbar x unchanged. | FIXED |
| N6 | Spec part 1 group-row line: "(`Badge/StatusCount`, ...)" to "(kit `count_badge`, see Count pill below; never "(3)")". No other `Badge/StatusCount` in the spec files (grep). | FIXED |
| N7 (T4) | M04, M04b, M04c now render the shared "Card payments use 3-D Secure when the issuer asks for it." callout, as the other ten Payment methods screens do. It sits straight after the list (like every other screen), so the "/" line follows it; the critique suggested "after the '/' line", but there the callout text would show in the 20 px gap between the two M04 specimens (the M04b artifact again). | FIXED |
| N8 | Spec §3.2: "its left edge is 6 px inside the panel's right edge, kit `scroll_thumb`". Measured M04b/M04c: panel right 719, thumb 713 to 717 (4 x 206.9). The README M04b-02 row in the round-2 ledger says "1 px from the right edge" (inner border edge); this is the kit wording now. | FIXED |
| N9 | "Needs owner" item 2 (phase-02 "Delete to Trash") deleted; items 1 and 3 of that list were settled too (S1 confirmation, T2) and are deleted with it. | FIXED |

Follow-on values the N1 change forced (all derived, listed so nothing is a surprise): row 1 of the table is 20 px taller (37 to 57), so M06 row handle top 79 to 99, add-row "+" 173 to 193 (M06 and M06b), column handle (Fee column right edge 241 - 8 - 24) left 279 to 209 with its menu; M11b and M11c show the page scrolled by 20 px (`main.col` top 80 to 60) so the last checklist item keeps its 15 px gap above the bottom chip (it would overlap the chip by 5 px otherwise); M06 cut column is Notes (89 of 320 px visible) instead of Limit (109 of 150). Table width 922, content width 954, thumb 499 and the M06b offset 264 are the same as before (5 x 120 + 320 = 920 = 160+150+130+140+150+190).

### Before and after (Playwright, 1440 x 900, 1x, real fonts; before = `a3fb8041` copies, after = this tree)

| ID | Measurement | Before | After |
|---|---|---|---|
| N1 | M06b Notes cell (row 1) scrollWidth / clientWidth | 243 / 190 (ellipsis: "3-D Secure when the i...") | 320 / 320, wraps to 2 lines |
| N1 | Cells with `text-overflow: ellipsis` / overflowing cells, every `td` and `th` on all screens | 264 / 11 (one per table screen, 11 screens) | 0 / 0 (288 cells, 12 screens) |
| N1 | Column widths (6 columns) | 160 / 150 / 130 / 140 / 150 / 190 | 120 / 120 / 120 / 120 / 120 / 320 (natural widest + 24 + 1: 113.5 / 115.6 / 88.0 / 114.9 / 81.0 / 461.7, clamped) |
| N1 | M06 row 1 (y, height) / table width | 241, 37 / 922 | 241, 57 / 922 |
| N2 | `.menu.scr` per capture, M04b / M04c | 2 / (no M04c) | 1 / 1 |
| N2 | Page text visible in the 20 px gap between menus (x 719 to 739) | table text "Set", "2 c", "2 d", "5 c" | no gap: one menu per capture |
| N3 | Spec §5 wording | "count of the rows listed under the file" (head clause 4, parenthesis 6) | "number of frames the grid lists for that file", not page rows |
| N4 | Thumb: spec formula / mockup | 690 x 688 / 954 = 497.6 / 499 | 690 x 690 / 954 = 499.06 / 499 |
| N5 | M05 "Turn into" font-size / line-height / weight, box | 14px / 24px / 600, 92.1 x 32 | 14px / 20px / 600, 92.1 x 32 |
| N6 | `Badge/StatusCount` occurrences in the two spec files | 1 | 0 |
| N7 | `.callout.note` in M04 / M04b / M04c | 0 / 0 / (no M04c) | 1 / 1 / 1 |
| N8 | Slash-menu thumb, panel right minus thumb left (M04b, M04c) | 6 (spec said "6 px from the right edge") | 6 (spec says "left edge 6 px inside the panel's right edge") |

### No ellipsis anywhere (acceptance 2)

`cells.cjs`, every `td` and `th` on every `m*.html`: `scrollWidth <= clientWidth && scrollHeight <= clientHeight`, `getComputedStyle().textOverflow !== 'ellipsis'`. Result: `{"htmlFiles":23,"screensWithTables":12,"cells":288,"ellipsisStyled":0,"overflowing":[]}`. Run against the before copies: 264 cells ellipsis-styled, 11 overflowing (`3-D Secure when the issuer ask` 243 > 190 on each of the 11 table screens).

### Geometry unchanged (acceptance 5)

`geom.cjs` over all `m02` to `m13b`, after: rail `[0, 48]`, tree `[48, 240]`, column `[399, 690]`, contents `[1137, 192]` on the 19 screens that have all four (M03 to M11c, M12, M13b, plus the new M04c); M02 has no contents, M13 has no column. Identical to before, apart from M04c being new.

### Captures and index (acceptance 3, 6)

24 PNGs regenerated from the worktree with headless Chrome (`--headless=new --hide-scrollbars --window-size=1440,900`; `m09-full.png` 1440 x 2630; `index.png` from `index.html#m07`); `sips`: 1440 x 900 for 23 PNGs, 1440 x 2630 for `m09-full`. Changed PNGs: M03, M04, M04b, M04c (new), M06, M06b, M10, M10b, M10c, M11, M11b, M11c, index. M05's PNG is byte-identical (N5 changes no pixel). Each changed screen was opened at 2x (`deviceScaleFactor` 2) and inspected.

Index (`index-test.cjs`, Playwright, 0 page errors): 23 list entries; the list has M04, M04b, M04c, M06, M06b; `#m04b` then Next goes to `#m04c` (m04c-slash-menu-scrolled-to-end.html); ArrowRight from `#m04c` goes to `#m05`; ArrowLeft twice returns to `#m04b`; deep link `#m04c` loads the right file; Prev from `#m04c` goes to `#m04b`; the round-3 checks (`#m06` / `#m06b` / `#m09full`) still pass.

### Gates (acceptance 7), 24 files, before = `a3fb8041` copies, after = this tree

- `ui gate`: exit 1 on all 24 (before: 23 of 23). Errors only `low-contrast` for `#737373` (settled), 42 on the 22 old mockups before and after; M04c adds the same 2. Finding classes per file: no class added or removed. Counts that moved: `off-system-token` +1 on every shell file (the new `var(--lh-sm)` reference of N5, same class; 552 to 553, M05 and M11 553 to 554), M04b `container-nesting-depth` 34 to 17 (one menu instead of two). M04c has the same classes as M04b. Before runs were done from a copy inside the repo tree so the project design system applies (a copy outside the repo shows no `off-system-token` or `token-coverage`).
- `slop-detect`: 213 findings (before 207), same four classes (`cramped-padding` 183, `clipped-overflow-container` 22, `overused-font` 7, `gray-on-color` 1); the +6 are the new M04c (4 + 3) minus one in M04b. No new class.
- axe: 18 of 24 exit 0, 6 exit 1 with the single `color-contrast` of `TEXT_PLACEHOLDER` (index, M01, M01b, M05, M07, M08). Same six as before; M04c exits 0.
- `ui gate` appended to `design/memory.events.*`; both files were restored with `git checkout`.
- Colour literals in `shell.css`, `index.html`, `m*.html`: 0. Shadows in `shell.css`: 0.

### NOT verified in round 4

Motion, drag-ghost offset, screen-reader names, egui text metrics (wrap points in the Notes cell depend on Inter metrics; a different engine may break the line one word earlier or later), kit horizontal-scrollbar thumb formula (no kit source for the table scrollbar; N4 compares spec against mockup only). Responsive captures skipped by owner default.


## Round 3 fixes (critique r2, 1 HIGH + 11 MED + 14 LOW, rulings S1 to S4)

Source: [critique-261102-project-docs-mockups-r2.md](../../reports/critique-261102-project-docs-mockups-r2.md). Rulings: [ui-polish-rulings-r2.md](../ui-polish-rulings-r2.md) (S1 to S4, sample-data rule) over [ui-polish-rulings-r1.md](../ui-polish-rulings-r1.md) (R0 and addendum, still binding). Line numbers are `shell.css` lines after this round. The critique already carries IDs for all 14 LOW findings (M13b-1 ... M07-1); they get the aliases L1 to L14 in the critique's order. Edits were made to both specs in the same change (R0).

### ID ledger (acceptance 1)

| ID | Change (file and selector or value) | Status |
|---|---|---|
| G1 | Spec part 1 "Count pill" (doc tree section) now states the kit `count_badge` neutral triple: `BADGE_NEUTRAL_BG` fill, 1 px `BADGE_NEUTRAL_BORDER`, `BADGE_NEUTRAL_TEXT` Inter 12/18 500, `BADGE_SM` 22 tall, full radius (`badge.rs:31-33, 75-79`). Mockups unchanged (already kit). Agent-chip count pill stays 16 / `BG_TERTIARY` / mono (spec 2 §9). | FIXED |
| G2 | `.pk-file` gap 8 to 0, `.pk-file .nm` margin-left 4, deleted `.pk-file .ch + .i { margin-left:-4px }`, added `.pk-file .pill` margin-left 8 (l.291-295). Picker rows now follow the doc-tree parent row (chevron 24, icon, 4, label at +44); pages +16. The critique's literal CSS (gap 4) would have left file labels at +48 vs +44 on "All files"; measured instead. | FIXED |
| M05-1 | `.menu { font-size: var(--fs-xs); line-height: var(--lh-xs) }` (l.215): popover rows no longer inherit the 14/24 paragraph type. Spec 2 section 3.3 states it. | FIXED |
| M11-1 | `.agent-tag { font-size: var(--fs-xs); line-height: var(--lh-xs) }` (l.282), height stays `BADGE_MD` 24. Spec 2 section 9 already said 12/18. | FIXED |
| M10-1 | `.banner > .ico { display:grid; place-items:center; height:20px; align-self:center }` (l.274); wrapper `<span class="ico">` in M10 and M10c. | FIXED |
| G3 | M04, M04b, M10, M10b, M10c: appended the shared "Fees and limits" table and "Before release" checklist (byte-identical block to M11b). M04 and M04b captions moved onto the empty `/` line so they do not land on page text. | FIXED |
| M04b-1 | Ruling S1: `.menu.scr` 360 to 344, `.mclip` 344 to 328 (l.455-456); thumb 227.6 to 206.9 (328 squared / 520); scrolled-to-end offset -176 to -192, thumb top 123.4 to 128.1. Spec 2 section 3.2 carries the number. The 344 reading was confirmed in the ruling S1 confirmation. | RULED (S1) |
| M06-1 | M06: `hov` removed from Row handle, Add row (and Add column); only the open Column handle keeps its `on` look. | FIXED |
| M06-2 | Ruling S4: table frame (1 px border, radius 12) is now `.tbl` itself and scrolls with its cells inside `.tbl-scroll` (l.201-204); width 960 to 922; columns 160/150/130/140/150/190 so Limit is cut mid-column (109 of 150 visible) instead of 11 px; thumb 496 to 499 (l.212). Add-column + lives in `.tbl-end` after the last column (out of view at scroll start). New screen M06b shows the scrolled-to-end state with the + 8 px right of the last column. Spec 2 section 4 rewritten. | RULED (S4) |
| M09-1 | M09 and M09 full: "Order confirmed . Checkout-web" (Frame not found) to "Promo banner . Checkout-web"; "Cart . Checkout-mobile" (File not found) to "Cart . Checkout-tablet". Neither name exists in the sample world. | FIXED |
| M08-1 | Ruling S3: spec 2 section 5 wording "frame count pill" to "count of the rows listed under the file (all frames in Pin and Embed modes, unpinned frames in Suggest mode)". Mockups unchanged (M07 6, M08 1 are both right). | RULED (S3) |
| M03-1 | M03 `.handles` left -48 to -52; M04 and M04b -56 to -60 (same hover handle everywhere): pair ends 12 px left of the column, 4 px clear of the selected fill. Spec 2 section 3.1. | FIXED |
| L1 (M13b-1) | M13b `.mfoot` margin-top 20 to 8: modal 192 to 180, buttons top 464 to 452, modal centre 426 = window centre - 24 (`home/dialog.rs:254-265`). Spec 2 section 12 states 440 x 180. | FIXED |
| L2 (M04-1) | `.mi.dis { color: var(--text-secondary); opacity: .5 }` (l.225) = kit `menu_item_look(Disabled)`. | FIXED |
| L3 (M02-1) | `.tbtn` 12/18 to 14/20 600 (l.107). | FIXED |
| L4 (M05-2) | `.ftb .fb.txt { font-weight: var(--w-strong) }` (l.237); spec 2 section 3.3 says "Turn into" is 14/20 600. | FIXED |
| L5 (M12-1) | `.kpi .v { height: var(--lh-xl) }`, `.kpi .v .sep { line-height: 0 }` (l.193-194): all four labels at y224, value rows 30 tall. | FIXED |
| L6 (G4) | `.tabstrip` align-items flex-start; `.tab.home` margin-top 4, `.tab.proj` 3, `.tab.plus` 4, `.tdiv` 8 (l.26-43). All integer y. Spec 1 tab-strip row cites `tabs.rs:983, 1027`. | FIXED |
| L7 (M01-1) | `.pcard .pf .btn { width:66px; padding:0 }` (l.375): "+ Folder" 66 x 24 at card right - 8. Spec 2 section 1. | FIXED |
| L8 (M01-2) | `.pgrid` margin-bottom 12 to 28 (l.365): Folders heading y567 to 583. Spec 2 section 1. | FIXED |
| L9 (M01-3) | `.pcard .po` top 111 to 103: "Open project" centre y327 to 319 = name line centre. Spec 2 section 1 says "centred on the name line". | FIXED |
| L10 (M13b-2) | Ruling S2: M13b "moves to the trash."; spec 2 section 12 and copy deck row. | RULED (S2) |
| L11 (M02-2) | M02 placeholder starts x399 (was 402) with the caret drawn over its first letter (`position:absolute` caret). Spec 2 empty page body line. | FIXED |
| L12 (M13-1) | M13 caption "Left tree is a composite specimen: ..." at x304, y52. | FIXED |
| L13 (G5) | `.r.foc` outline-offset 2 to -2 (l.92) = band drawn inside the row; M13 "Processing" row carries `foc`. | FIXED |
| L14 (M07-1) | `.pk-foot .dd { height: var(--control-xs) }` (l.325): footer dropdown 36 to 32, same row as the 32 segmented control. | FIXED |

BLOCKED: none. Needs owner: none (the three open items were settled; see round 4).

**ID proof** (`id-proof.py` reads the ID column of critique r2 and of the ledger above; red probe = same script on a copy with one ledger row deleted):

```
ledger: critique IDs 26 (HIGH 1, rows with a status 26), missing: none
LOW order in critique: ['M13b-1', 'M04-1', 'M02-1', 'M05-2', 'M12-1', 'G4', 'M01-1', 'M01-2', 'M01-3', 'M13b-2', 'M02-2', 'M13-1', 'G5', 'M07-1']
aliases in ledger   : ['M13b-1', 'M04-1', 'M02-1', 'M05-2', 'M12-1', 'G4', 'M01-1', 'M01-2', 'M01-3', 'M13b-2', 'M02-2', 'M13-1', 'G5', 'M07-1']
alias order matches: True L-numbers: True
status counts (over critique IDs): {'FIXED': 22, 'RULED (S1)': 1, 'RULED (S4)': 1, 'RULED (S3)': 1, 'RULED (S2)': 1}
red probe (M06-2 row deleted): critique IDs 26 (HIGH 1, rows with a status 25), missing: ['M06-2']
```

### Before and after, every HIGH and MED (acceptance 2)

Headless Chrome 1440 x 900, `getBoundingClientRect()` after fonts loaded (`metrics.cjs` for round 3, the critique's `measure.cjs` method). Before = `b91395f3` files, after = this tree.

| ID | Measurement | Before | After |
|---|---|---|---|
| G1 | M03 `.g .pill` | 22 x 22, `#171717`, 1 px `#404040`, Inter 12/18/500, r11 (mockup); spec text "BG_TERTIARY mono 12, 16 tall" | same mockup; spec text = kit triple, 22 tall |
| G2 | M07 file row icon x / label x | 332 / 356 | 328 / 348 |
| G2 | M07 page row icon x / label x | 344 / 364 | 344 / 364 (= file icon +16, label +16) |
| G2 | M08 "All files" icon x / label x | 328 / 352 | 328 / 348 (same column as file rows) |
| M05-1 | M05 result rows | 14px/24px/500 x 3 | 12px/18px/500 x 3, height 32 |
| M11-1 | M11 `.agent-tag` | 14px/24px/500, 60.4 x 24 | 12px/18px/500, 56.3 x 24 |
| M10-1 | M10 and M10c icon box centre y / text centre / buttons centre | 106.5 / 109 / 109 | 109 / 109 / 109 |
| G3 | h2 count, tables, checklists, column bottom (M04 / M04b / M10 / M10b / M10c) | 1, 0, 0; 390 / 390 / 520 / 438 / 520 | 3, 1, 1; 805 / 805 / 935 / 853 / 935 (matches "On this page": 3 headings) |
| M04b-1 | menu height / clip / partial rows / thumb | 360 / 344 / none / 227.6 | 344 / 328 / "Code: 16/32 visible" / 206.9 |
| M06-1 | M06 revealed controls | Row handle, Add row, Add column all `hov` (bg `#171717`, fg `#A3A3A3`) | all rest (bg transparent, fg `#737373`); only Column handle `on` |
| M06-2 | M06 table / viewport right / cut column / thumb | 400..1360 (960) / 1089 / Limit 980..1100 cut 11 px; Notes never visible / 496 | 399..1321 (922) / 1089 / Limit 980..1130 shows 109 of 150, cut mid-column / 499 |
| M06-2 | Add-column + | x1097 (8 px right of the viewport) | M06: x1329 (8 px right of the last column, clipped); M06b: x1065 = last column right 1057 + 8 |
| M09-1 | Frame not found / File not found card titles | "Order confirmed . Checkout-web" / "Cart . Checkout-mobile" | "Promo banner . Checkout-web" / "Cart . Checkout-tablet" |
| M08-1 | spec wording | "frame count pill" | "count of the rows listed under the file" (mockups unchanged: M07 pill 6, M08 pill 1) |
| M03-1 | handle pair x / gap to selected fill / gap to column | 343..391 / 0 / 8 | 339..387 / 4 / 12 |

LOW re-measures (`verify.cjs`): L1 modal y336 h180 centre 426, buttons top 452; L2 colour `rgb(212,212,212)` opacity .5; L3 14px/20px/600 on a 24 tall button; L4 "Turn into" 14px/24px/600; L5 four value rows 30, four labels y224; L6 tab y 4 / 3 / 4, divider y8; L7 +Folder 66 x 24 at x482 (card right 556 - 8 - 66); L8 Folders heading y583 (last card bottom 555 + 28); L9 Open project centre 319 = name centre 319; L11 caret x399, placeholder x399, column x399; L13 `outline: 2px solid rgb(249,115,22)`, offset -2px on row x56 w223; L14 footer seg 32, dd 32, buttons 40.

### Spec and mockups agree (acceptance 3)

| Value | Spec (changed text) | Mockup |
|---|---|---|
| Count pill | spec 1 doc tree "Count pill" | 22 tall, `BADGE_NEUTRAL_*`, Inter 12/18 500 (M03 etc.) |
| Slash cap | spec 2 section 3.2: 344 = 8 + 328 + 8, thumb 206.9, end offset 192 | `.menu.scr` 344, `.mclip` 328, thumb 206.9, margin-top -192 |
| Wide table | spec 2 section 4 (S4) | M06 start state, M06b end state |
| Per-file pill | spec 2 section 5 (S3) | M07 6, M08 1 |
| Trash wording | spec 2 section 12 and copy deck (S2) | M13b "moves to the trash." |
| Agent tag | spec 2 section 9 12/18 500 | `.agent-tag` 12px/18px/500 |
| Link popover rows | spec 2 section 3.3 12/18 500, 32 tall | M05 result rows 12px/18px/500, 32 |
| Handle pair | spec 2 section 3.1: ends 12 left of the column | M03 339..387, M04 / M04b same |
| Picker rows | spec 2 section 5: label +44, pages +16 | M07 / M08 table above |
| Delete dialog | spec 2 section 12: 440 x 180, 8 before buttons | M13b 440 x 180, buttons top 452 |
| Home card | spec 2 section 1: + Folder 66 x 24, Open project on the name line, heading 28 below | M01 table above |
| Tab strip | spec 1: Home y4, project tabs y3, + tab y4 | all 20 shell screens |
| KPI / placeholder | spec 2 sections 10 and 2 | M12 / M02 |

### Sample world (acceptance 4) and geometry (acceptance 5)

`verify.cjs` over all 22 mockups: the doc tree lists the same 18 labels and pills 1,4,3,2,2 on M03, M04, M04b, M05, M06, M06b, M07, M08, M09 x 2, M10, M10c, M11, M11b, M11c, M13b; deliberate variants only: M02 (empty tree), M10b ("Saving..." instead of "2 files"), M12 (linked package row, pill 2), M13 (rename row). Page titles: H1 = tab = selected tree row on every Payment methods screen (M03 to M06b, M10 to M11c), "Payment flow" on M09, "Checkout" on M02, M07, M08, M12, "Cart rules" on M13b. Frame and file names: the Frame not found card names a frame outside the inventory ("Promo banner"), the File not found card a file outside the project ("Checkout-tablet"); "Payment . Checkout-mobile" (Can't render) is in the inventory.

Layout geometry, every screen that has the region (M02 to M13b; Home screens have no tree or column): rail `[0, 48]`, tree `[48, 240]`, column `[399, 690]`, contents `[1137, 192]` (absent only on M02 and M13 as before). Unchanged from round 2.

### Captures and index (acceptance 6, 7)

All 23 PNGs regenerated with headless Chrome from the worktree (`--headless=new --hide-scrollbars --window-size=1440,900`; `m09-full.png` 1440 x 2630; `index.png` from `index.html#m07`); `sips` reports 1440 x 900 for 21 mockups plus index and 1440 x 2630 for m09-full. Changed screens were inspected at 2x and 3x crops: M06, M06b, M04, M04b, M10, M13, M13b, M02, M09, M01, M07, M05, M11 (full 2x frames for M06, M06b, M04, M04b, M13; the final PNGs of M01 inspected directly).

Index (`index-test.cjs`, Playwright, 0 page errors): list has 22 entries incl. "M06b Table scrolled to the end"; `#m06` loads m06; Next goes to `#m06b` (m06b-table-scrolled-to-end.html); Prev back to `#m06`; ArrowRight to `#m06b` then `#m07`; ArrowLeft twice back to `#m06`; deep links `#m06b` and `#m09full` load the right file.

### Gates (acceptance 8), 23 files, before = `b91395f3` copies, after = this tree

- `ui gate`: exit 1 on all, as before. Errors: only `low-contrast` for `#737373` (kit token, settled): 40 before (22 files), 42 after (23 files; the 2 extra are M06b). Per-file finding-class counts (errors and warnings excluding `off-system-token`): 0 of 22 existing files changed; M06b shows the same classes as every other shell screen. A first M06b draft used a third wrapper (`.tbl-box`) and tripped `container-nesting-depth`; the table frame is now the `<table>` itself.
- `slop-detect`: 207 findings (before 208 on 22 files), same four classes (`cramped-padding` 180, `clipped-overflow-container` 19, `overused-font` 7, `gray-on-color` 1), no new class.
- axe: 17 of 23 files exit 0; 6 exit 1 with the single settled `color-contrast` of `TEXT_PLACEHOLDER` (`.ph` on M05, M07, M08; `.hs-search > span` on M01, M01b; the index iframe). Same six as before; no new rule.
- Running `ui gate` appended to `design/memory.events.*`; both files were restored with `git checkout`.

### NOT verified in round 3

Motion, drag-ghost offset, tree-row keyboard focus beyond the one specimen drawn, screen-reader names, egui text metrics; responsive captures skipped by owner default.

## Round 2 fixes (critique r1, 102 IDs)

Source: [critique-261102-project-docs-mockups-r1.md](../../reports/critique-261102-project-docs-mockups-r1.md). Rulings applied: [ui-polish-rulings-r1.md](../ui-polish-rulings-r1.md) (R0, Q1 to Q7, sample-world and copy rules). Line numbers are `shell.css` lines. "Spec 1" = part 1, "Spec 2" = part 2.

**Proof that every ID is accounted for** (`id-diff.py` parses the ID column of the critique and of the table below, red-probed by deleting one row from a copy):

```
critique IDs: 102 (HIGH 22, MED 50, LOW 30); README rows: 102
missing from README: none
extra in README: none
status counts: {'FIXED': 98, 'RULED-OUT': 4}
exit code: 0
red probe (README copy without the M09-06 row):
  critique IDs: 102 (HIGH 22, MED 50, LOW 30); README rows: 101
  missing from README: ['M09-06']
  extra in README: none
  status counts: {'FIXED': 97, 'RULED-OUT': 4}
  exit code: 1
```

| ID | What changed (file, selector, value) | Status |
|---|---|---|
| G01 | `shell.css` `.btn.sec` (l.250): `background: var(--bg)`, `border-color: var(--border-strong)`, `color: var(--text-secondary)`, icon `var(--icon-quaternary)` (l.251). New tokens `--icon-quaternary`, `--text-secondary-hover`. Spec 2 "Buttons". | FIXED |
| G02 | `.btn.dng` (l.252): `var(--bg-error-solid)` #DC2626 + `var(--text-white)`; ring `.btn.dng.foc` uses `--focus-ring-error`. Spec 2 "Buttons"; old ambiguity 4 deleted. | FIXED |
| G03 | `.scrim` (l.257) `var(--bg-overlay)` rgba(38,38,38,.702); `.modal` (l.258) no border, `var(--bg)`, radius `--radius-frame` 16. Spec 2 "Modal". | FIXED |
| G04 | `.tip` (l.229): `var(--bg-primary-solid)`, no border, radius 8, padding 8 12, 12/18 400. Spec 2 "Tooltip". | FIXED |
| G05 | `.toast` (l.263-264): `var(--bg-primary-alt)`, 1 px `--border-menu`, radius 12, 40 tall, bottom 72, min width 240, 8 px `--icon-brand-primary` dot (`::before`, 16 from the outer edge), text 14/20 500 at x 28, right 16; hint is one text run "Copied for AI · paste into the agent". Spec 2 "Toast" names `toast_surface`. | FIXED |
| G06 | `.field` / `.field.foc` / `.field .ph` (l.238-242): `--bg` + 1 px `--border-strong`, focus 2 px `--border-selected`, placeholder `--text-placeholder`; Home search `.hs-search`; picker search and dropdown (`dropdown_look`: no border at rest). Spec 2 "Fields". | FIXED |
| G07 | `.chk .box` (l.143-144): 16 px, radius `--space-fact` 4, 1 px `--border-strong`; `.on` = `--bg-brand` + `--text-on-brand`; picker check circles reuse it (ruling Q1). Spec 1 checklist row, spec 2 "Checkbox". | FIXED |
| G08 | `.mi` (l.213-224): rows 32, pill inset 8, label 12/18 500 `--text-secondary`, icon `--icon-quaternary`, hover `.hl` = `--bg-hover` + `--text-secondary-hover` + `--icon-quaternary-hover`; shortcut `--text-quaternary`; menu panel `.menu` radius 16, padding 8 incl. 1 px `--border-menu`. Spec 2 "Menu / popover". | FIXED |
| G09 | Rows `.r` / `.r.sel` / `.r.hov` (l.77-91): selected `--bg-selected` + `--text-secondary-hover`, weight 600; tabs `.tab.sel` (l.30-31); icon buttons `.ibtn` (l.59-61); tools `.rail .tile.sel` (l.47); segmented `.seg`. Spec 1 States bullet and spec 2 "Segmented"/"Icon buttons". | FIXED |
| G10 | `.mi.dng` (l.223-224): label `--text-error` #F87171, icon `--danger`; Home "Move to trash" has no red (context_entries). Spec 2 §3.1 and §4. | FIXED |
| G11 | `.tree-title .acts { gap: 8px }` (l.65): centres 32 apart (203/235/267), "…" right edge aligned with the title-row "+" right edge (both 8 from the panel edge). | FIXED |
| G12 | `.g { padding-right: 4px }` (l.71): count pill sits 4 px inside the rounded row. | FIXED |
| G13 | Block menu top = handle bottom + 4 (measured 258 to 262), submenu flush (607 = 607), slash menu top = line box bottom + 4 (390 to 394), link popover = line top + 28 (line box + 4), selection toolbar bottom = selection top - 8 (222 vs 230), context menus open at the pointer. Spec 2 "Overlay offsets". | FIXED |
| G14 | One icon-button look `.ibtn` for block handle, "+", table add buttons, column/row handles, tree actions (l.59-61); pressed/open = `--bg-selected` + `--icon-secondary-hover`. Spec 2 "Icon buttons". | FIXED |
| G15 | "Copy block link", "Copy page link" (generator menus M03/M13); "Open in canvas" and embed open button use the frame glyph; tree "Open" uses the reader glyph; the external-link glyph survives only on the link popover "Open" (tooltip "Opens in browser"). Spec 2 §3.1, §3.3, §12, §15. | FIXED |
| G16 | `.embed-h .tx { white-space: pre }` (l.161): "Payment · Checkout-web" is one run with spaces on both sides of the dot; toast is one run. Spec 1 embed header. | FIXED |
| G17 | Tab strip from `tabs.rs`: Home tab 72x24 at x 8, divider, 240x26 project tabs, 14 px kind tile (`.tab .tile.spec` = `TILE_SPEC_*`, `.design` = `TILE_DESIGN_*`, radius 2, 11 px glyph), 18 px close plate, separator between inactive tabs, "+" 24 (`shell.css` l.25-41). Spec 1 layout table. | FIXED |
| G18 | Real rail from `panels/rail.rs` (menu, separator at 48, File/Agents/Assets/Tools, separator, Variables/Plugins, help; File selected) (`.rail` l.45-49). Critique said "add only the Project tile": read as the Project tab tile, the rail gets no new item. Spec 1 layout table. | FIXED |
| G19 | Ruling Q4: flash outline 2 px at `--agent-1-60` (AGENT_1 at 60 %), radius 8; tag always drawn with it; focus rings stay 2 px `--focus-ring` bands (the kit ring is 2 px, wider than the 1 px the ruling text assumes). Spec 2 §9. | FIXED |
| M01-01 | M01 rebuilt as the real Home (`home/mod.rs`): sidebar 264, header 64, buttons 36 radius 8 14/600, filter row, hairline at y 142, project cards 260x168 (`.pcard`), "+ Folder" 24 px mini (`.btn.t24`), neutral 22 px meta tile; version text "v0.7.5". Spec 2 §1. | FIXED |
| M01-02 | Project card grid gap 16 (`.pgrid`, l.359) = source; file-card grid gap 24 = `HOME_CARD_GAP` (`theme.rs:963`, `--home-card-gap`). Ruling R0 (theme.rs beats P3). Principle/product mismatch listed under "Needs owner". | RULED-OUT (R0) |
| M01-03 | "Open project" is a 24 px secondary button at the right end of the name row (never over the preview); hover state described in spec 2 §1. | FIXED |
| M01-04 | Context menu top-left at the pointer (320, 350) inside the Checkout card; first item "Open project". | FIXED |
| M01-05 | New screen `m01b-home-project-header.html`: inside a project, "Open project" is the leftmost header button, then "+ New folder", "Import", "+ FigJam board", "+ Design file". Spec 2 §1. | FIXED |
| M01-06 | One project list in M01: sidebar and grid both show Checkout, Onboarding, Design system, Marketing site. | FIXED |
| M01-07 | Home project tiles use the neutral tile (`BG_TERTIARY` + frame glyph, as in source); the Project tab uses `TILE_SPEC_BG/GLYPH` (ruling Q3, no new token). Spec 1 layout table. | FIXED |
| M01-08 | Previews vary per card (different frame counts, mobile thumbnail on Checkout); card title 14/600 (`.pcard`). | FIXED |
| M01-09 | Product measures the same 32 as the mockup; principle P3 text left alone. Recorded under "Needs owner" for the principles owner. | RULED-OUT (critique: leave) |
| M02-01 | `.r.ph { padding-left: 44px }` (l.93): placeholder text sits on the label column. Spec 2 §2. | FIXED |
| M02-02 | M02 adds a search-no-result specimen ("No pages match" + **Clear search**). Spec 2 §2. | FIXED |
| M02-03 | `.r.ph.hov { background: none }`, hover only reveals the "+" (l.94-96). Spec 2 §2. | FIXED |
| M02-04 | Header keeps "2 files"; spec 2 §2 now states N = the project's design files (same number as the Home card). | FIXED |
| M03-01 | Checklist uses the kit checkbox (G07). | FIXED |
| M03-02 | `.selblk` (l.151): padding 4 8, margin 12 -8 -4, radius 8, `--bg-selected-area`. | FIXED |
| M03-03 | Pressed drag handle = `.ibtn.on` (`--bg-selected`), same as every icon button (G14). | FIXED |
| M03-04 | "Turn into" submenu uses the kit check column (check at x 12, icon box 32, label 60; `.mi.hasck`, `.ckc`); current type (Text) checked. Kit style instead of the critique's right-aligned check (R0). Spec 2 §3.1 and "Menu". | FIXED |
| M03-05 | `.embed-h { padding: 0 8px 0 16px }` (l.159) = body inset 16. Spec 1 embed header, spec 2 §6. | FIXED |
| M03-06 | G16, G10, G15 applied in M03. | FIXED |
| M03-07 | Critique itself says a menu shot needs no more prose; M12 carries the reading page. No change. | RULED-OUT (critique: no action) |
| M04-01 | M04 third specimen: disabled row "No blocks match" (`aria-disabled`). Spec 2 §3.2. | FIXED |
| M04-02 | Slash menu opens 4 below the line box (G13). | FIXED |
| M04b-01 | `.mi .hint` (l.216): Roboto Mono 12/18 `--text-quaternary`; "```" is a monospace backtick run. Spec 2 §3.2. | FIXED |
| M04b-02 | Max height cuts a row (`.menu.scr`; value 360 in round 2, 344 since round 3 ruling S1), 4 px `--text-tertiary` thumb (`.mscroll`, l.227) 1 px from the right edge. Spec 2 "Menu". | FIXED |
| M04b-03 | `.mclip { height: 344px; overflow: hidden }` (l.449): rows clip at the 8 px inset, not under the radius. | FIXED |
| M04b-04 | Captions "Top" / "Scrolled to end" under the two menus. | FIXED |
| M05-01 | Link field = kit field (G06). | FIXED |
| M05-02 | M05 adds the existing-link specimen: URL + **Open** + **Remove link**, tooltip "Opens in browser". Spec 2 §3.3. | FIXED |
| M05-03 | Suggestion list excludes the page being edited (first row is another page). Spec 2 §3.3. | FIXED |
| M05-04 | Toolbar "Turn into" uses the right chevron like the block menu. Spec 2 §3.3. | FIXED |
| M06-01 | One page model "Payment methods": Accepted methods / Fees and limits / Before release on every Payment methods screen (M03, M04, M04b, M05, M06, M10, M10b, M10c, M11, M11b, M11c) in "On this page". | FIXED |
| M06-02 | Column handle sits inside its header cell at the right end; row handle outside the table at the left. Spec 2 §4. | FIXED |
| M06-03 | Handles and both "+" use `.ibtn` (G14). | FIXED |
| M06-04 | Directional glyphs for Insert left / Insert right (Move left/right keep arrows). | FIXED |
| M07-01 | Pinned frames carry the **Pinned** status tag; the modal pins an unpinned frame. Sample world per rulings. | FIXED |
| M07-02 | Left list: file rows (count pill), page rows as plain depth-1 rows without chevron; grid shows the selected file's six frames with the page name under each tile. Spec 2 §5. | FIXED |
| M07-03 | `.pk-left` rows gap 0; field to list 12 (`SPACE_GROUP`). | FIXED |
| M07-04 | Both picker columns use padding-top 16. | FIXED |
| M07-05 | Tile name margin-top 8 (`SPACE_INLINE`). | FIXED |
| M07-06 | Render is fit inside the 144 px thumbnail below a 38 px badge band, bottom-aligned. | FIXED |
| M07-07 | `status_tag` look: neutral triple (`--badge-neutral-*`), radius 8, `--badge-sm` 22 (l.306). Spec 2 §5. | FIXED |
| M07-08 | No close control in the header; spec 2 "Modal" states Esc or Cancel closes. | FIXED |
| M07-09 | Distinct frame and page names (Cart/Payment/Order confirmed, "Cart flow", "Payment states", "Confirmation"); slop-detect no longer reports repeated container text. | FIXED |
| M08-01 | Inventory fixed: Checkout-web unpinned = Empty cart; Checkout-mobile = Cart, Payment, Order confirmed (count pill 4 = 1 + 3). | FIXED |
| M08-02 | Mobile frames authored natively at 390x844 (`.fr.mob`), no wrapped lines, no clipped receipt line. | FIXED |
| M08-03 | Each thumbnail is truthful: empty cart, mobile cart, mobile payment, mobile confirmation. | FIXED |
| M08-04 | Same column top and gap-0 rows as M07. | FIXED |
| M08-05 | Ruling Q5: footer "3" count pill + "selected" at the left, button reads "Pin frames" with no pill. Spec 2 §5. | FIXED |
| M08-06 | "All files" row (layers glyph, count 4) documented in spec 2 §5 and in the copy deck. | FIXED |
| M09-01 | "Payment flow" added to Spec (4 pages) and selected on M09; H1 = tree label (title-check script). | FIXED |
| M09-02 | State-panel buttons `.btn.xs` 32 tall, 14/20 600. Spec 2 §6. | FIXED |
| M09-03 | "File not found" and Ready + pointer-over (open button in `.ibtn.hov` look) added to M09 / M09 full. Spec 2 §6 table. | FIXED |
| M09-04 | Header inset, dot spacing and glyph fixed (M03-05, G15, G16). | FIXED |
| M09-05 | Alert glyph in `--icon-tertiary` on the three error states. Spec 2 §6. | FIXED |
| M09-06 | Diagram SVG centre = card centre (744 = 744, measured). | FIXED |
| M09-07 | Real document headings ("Screens", "Flow diagram") replace the meta heading. | FIXED |
| M09-08 | Typographic apostrophe in "Can’t render this frame", "Can’t save this page."; spec 2 §15 updated. | FIXED |
| M10-01 | "Saving…" moved to its own screen (M10b); M10 keeps "N files" and spec 2 §8 says saves are held while the banner is open. | FIXED |
| M10-02 | New `m10c-save-failed.html`: `--danger` border and icon, "Can’t save this page." + error line + **Try again**. Spec 2 §8. | FIXED |
| M10-03 | Banner to title gap 24 named `SPACE_SECTION` in spec 2 §8. | FIXED |
| M10-04 | "3-D Secure" is plain text in M03/M05/M06/M10/M11 (callout and paragraph). | FIXED |
| M11-01 | Agent caret at the end of "Pay later (planned)", the block the flash marks; name tag right of the caret. | FIXED |
| M11-02 | Ruling Q4 (see G19): 2 px flash at 60 % + name tag; focus ring is a separate 2 px `--focus-ring` band. | FIXED |
| M11-03 | One state per capture: M11 caret + flash, M11b off-screen chip, new `m11c-agent-held-while-typing.html` held chip. Spec 2 §9. | FIXED |
| M11b-01 | `.chip` (l.266-268) follows `agent_changes_chip.rs`: 32 px pill in `--agent-1`, inset 8, count pill 16 tall `--bg-tertiary` mono, "changes", "Show", caret, labels 12/18 500 `--agent-label-text`. | FIXED |
| M11b-02 | The whole chip is one `<button class="chip">`. Spec 2 §9. | FIXED |
| M12-01 | Tooltip and toast per G04, G05; the tooltip opens 4 px to the right of the sparkle button so it no longer touches the KPI strip. | FIXED |
| M12-02 | Ruling Q6: Screens tile `3 · 24` matches the 3 pinned screens in the tree. | FIXED |
| M12-03 | `.kpi .v .sep` (l.193): Inter `--text-tertiary` dot with 4 px margins. Spec 2 §10. | FIXED |
| M12-04 | Fourth tile sublabel "Open: G5 Release". Spec 2 §10. | FIXED |
| M12-05 | Linked package named "checkout-spec · spec v2". | FIXED |
| M12-06 | Stale ambiguity 8 replaced (KPI strip sits under the description, tiles content-aligned). | FIXED |
| M13-01 | `.r.ghost` (l.100-101): opaque `--bg-selected` row, 80 % opacity only on its contents. | FIXED |
| M13-02 | Context menu top-left at the pointer inside the "Checkout research" row. Spec 2 "Overlay offsets". | FIXED |
| M13-03 | "Open" in the tree menu uses the reader glyph (G15). | FIXED |
| M13-04 | Rename field = kit field look: `--bg` + 2 px `--border-selected`, 24 tall, 4 px inset (`.r.ren .rf`). Spec 2 §12. | FIXED |
| M13-05 | Caption "Search active: filtered by “ca”, the open page stays selected" explains the selected fill. | FIXED |
| M13b-01 | Delete button = kit destructive (G02). | FIXED |
| M13b-02 | Initial focus ring on **Cancel**; Delete gets the ring only when tabbed to. Spec 2 §12. | FIXED |
| M13b-03 | Two body lines, one sentence each: "“Cart rules” moves to the trash." (round 3, ruling S2) / "Links to it will break." | FIXED |
| M13b-04 | Modal is centred 24 px above the window centre because `home/dialog.rs` (W 440, PAD 24) does it; the picker (M07/M08) stays exactly centred. The critique asked for exact centre; the kit wins. | RULED-OUT (R0, dialog.rs) |
| IDX-01 | `index.html` has an "M09 full" entry (`m09full`, 1440 x 2630, per-entry height in `fit()`), plus entries for the five new screens. | FIXED |
| IDX-02 | Index Prev / Next / Open full size use the kit secondary look (`.btn`), 2 px focus outlines. | FIXED |

### G01 to G10 values with kit source lines (acceptance 2)

Each value was read from the Rust source in this worktree (`crates/designos-app/src/`), not inferred from screenshots. Mockup CSS lines are in `shell.css`.

| ID | Kit source | Values copied | Mockup CSS |
|---|---|---|---|
| G01 secondary button | `widgets/kit/button.rs:68-75` (`secondary`), `:31-45` (`button_look`) | fill `BG`, 1 px `BORDER_STRONG`, label `TEXT_SECONDARY`, icon `ICON_QUATERNARY`; hover `BG_HOVER` + `TEXT_SECONDARY_HOVER` + `ICON_QUATERNARY_HOVER`; pressed `BG_PRESSED` | `.btn.sec` l.250-251 |
| G02 destructive button | `button.rs:40` | `BG_ERROR_SOLID` + `TEXT_WHITE`; ring `FOCUS_RING_ERROR` (`button.rs:48-56`) | `.btn.dng` l.252, `.btn.dng.foc` l.254 |
| G03 modal | `widgets/kit/surface.rs:37-39` (`modal_look`), `:66-69` (`modal_surface`), `theme.rs:152` (`BG_OVERLAY`) | scrim `BG_OVERLAY`, frame `BG`, no border, radius `MENU_RADIUS` 16 (`surface.rs:12`) | `.scrim` l.257, `.modal` l.258 |
| G04 tooltip | `surface.rs:52-54` (`tooltip_look`), `:82-88` (`tooltip_frame`) | `BG_PRIMARY_SOLID`, no border, radius `RADIUS_CONTROL` 8, margin 12 x 8 (`INSET_BUTTON` x `INSET_FRAME`) | `.tip` l.229 |
| G05 toast | `surface.rs:47-49` (`toast_look`), `:77-80` (`toast_surface`), `shell/toast_layout.rs:14` (`PAD_LEFT` 28), `:27` (`DOT_INSET` 16) | `BG_PRIMARY_ALT`, 1 px `BORDER_MENU`, radius `RADIUS_XL` 12, dot `ICON_BRAND_PRIMARY` | `.toast` l.263-264 |
| G06 fields | `widgets/kit/field.rs:17-28` (`field_look`), `:34-36` (`search_look`), `:44-52` (`dropdown_look`) | `BG` + `BORDER_STRONG`, focus 2 px `BORDER_SELECTED`, placeholder `TEXT_PLACEHOLDER`; dropdown no border at rest | `.field` l.238-242 |
| G07 checkbox | `widgets/kit/choice.rs:46-52` (`checkbox_look`) | 16 box, radius 4, `BORDER_STRONG`; selected `BG_BRAND` + `TEXT_ON_BRAND` | `.chk .box` l.143-144 |
| G08 menus | `surface.rs:23-25` (`menu_panel_look`), `widgets/kit/rows.rs:47-54` (`menu_item_look`), `theme.rs:343` (`font_menu`), `theme.rs:668` (`MENU_ITEM_HEIGHT`), `:697` (`SUBMENU_OVERLAP`) | panel `BG` + `BORDER_MENU` radius 16, rows 32, label `TEXT_SECONDARY` 12/18 500, icon `ICON_QUATERNARY`, hover `BG_HOVER` + `TEXT_SECONDARY_HOVER` + `ICON_QUATERNARY_HOVER` | `.menu` l.212, `.mi` l.213-224 |
| G09 rows, tabs, segmented | `rows.rs:11-21` (`row_look`), `widgets/kit/tabs.rs:42-50` (`segmented_look`), `:53-62` (`tab_look`), `button.rs:44-45` (Icon, Tool) | selected row `BG_SELECTED` + `TEXT_SECONDARY_HOVER`; tab selected `BG_SELECTED` + `TEXT_SECONDARY`; segmented selected `BG_SELECTED` + 1 px `BORDER_STRONG` | `.r.sel` l.89, `.tab.sel` l.30, `.ibtn` l.59-61 |
| G10 danger text | `theme.rs:100` (`TEXT_ERROR`), `DANGER` icon | label `TEXT_ERROR`, icon `DANGER` | `.mi.dng` l.223-224 |
| Dialog (M13b) | `home/dialog.rs:76-77` (`W` 440, `PAD` 24), `theme.rs:477` (`INSET_MODAL`) | 440 wide, padding 24, buttons 40 tall, Cancel focused, centred 24 px above the window centre | `.modal`, `.btn.md` l.247 |
| Focus ring | `widgets/kit/focus.rs:24-40` | 2 px band, 2 px outside, radius + 4 | `.btn.foc` l.253, `.r.foc` l.91 |

### Token citations

citations: 61 mismatch: 0 not a theme.rs Color32 const: 0 (script `token-citations.py`: every `--name` in `tokens.css` whose comment names a `theme.rs` constant is looked up in `crates/designos-app/src/theme.rs`; the hex must equal the constant's hex).

| CSS variable | theme.rs constant | Source line | Value |
|---|---|---|---|
| `--bg` | `BG` | theme.rs:22 | `#0A0A0A` |
| `--bg-secondary` | `BG_SECONDARY` | theme.rs:24 | `#171717` |
| `--bg-tertiary` | `BG_TERTIARY` | theme.rs:25 | `#262626` |
| `--bg-hover` | `BG_HOVER` | theme.rs:26 | `#171717` |
| `--bg-pressed` | `BG_PRESSED` | theme.rs:27 | `#404040` |
| `--bg-secondary-hover` | `BG_SECONDARY_HOVER` | theme.rs:88 | `#262626` |
| `--bg-selected` | `BG_SELECTED` | theme.rs:31 | `#262626` |
| `--bg-text-selection` | `BG_TEXT_SELECTION` | theme.rs:35 | `#431407` |
| `--bg-brand` | `BG_BRAND` | theme.rs:36 | `#E36323` |
| `--bg-brand-hover` | `BG_BRAND_HOVER` | theme.rs:37 | `#F97316` |
| `--bg-brand-pressed` | `BG_BRAND_PRESSED` | theme.rs:38 | `#EA580C` |
| `--bg-error-solid` | `BG_ERROR_SOLID` | theme.rs:110 | `#DC2626` |
| `--bg-primary-alt` | `BG_PRIMARY_ALT` | theme.rs:122 | `#171717` |
| `--bg-primary-solid` | `BG_PRIMARY_SOLID` | theme.rs:124 | `#262626` |
| `--bg-overlay` | `BG_OVERLAY` | theme.rs:152 | `rgba(38, 38, 38, 0.702)` |
| `--bg-selected-area` | `BG_SELECTED_AREA` | theme.rs:43 | `rgba(53, 130, 255, 0.2)` |
| `--border` | `BORDER` | theme.rs:44 | `#262626` |
| `--border-menu` | `BORDER_MENU` | theme.rs:47 | `#262626` |
| `--border-strong` | `BORDER_STRONG` | theme.rs:68 | `#404040` |
| `--border-selected` | `BORDER_SELECTED` | theme.rs:48 | `#FB923C` |
| `--border-translucent` | `BORDER_TRANSLUCENT` | theme.rs:49 | `rgba(255, 255, 255, 0.102)` |
| `--text` | `TEXT` | theme.rs:50 | `#FAFAFA` |
| `--text-secondary` | `TEXT_SECONDARY` | theme.rs:51 | `#D4D4D4` |
| `--text-secondary-hover` | `TEXT_SECONDARY_HOVER` | theme.rs:90 | `#E5E5E5` |
| `--text-tertiary` | `TEXT_TERTIARY` | theme.rs:52 | `#A3A3A3` |
| `--text-quaternary` | `TEXT_QUATERNARY` | theme.rs:94 | `#A3A3A3` |
| `--text-placeholder` | `TEXT_PLACEHOLDER` | theme.rs:96 | `#737373` |
| `--text-error` | `TEXT_ERROR` | theme.rs:100 | `#F87171` |
| `--text-on-brand` | `TEXT_ON_BRAND` | theme.rs:106 | `#0A0A0A` |
| `--text-white` | `TEXT_WHITE` | theme.rs:112 | `#FFFFFF` |
| `--icon` | `ICON` | theme.rs:56 | `#FFFFFF` |
| `--icon-secondary` | `ICON_SECONDARY` | theme.rs:57 | `#D4D4D4` |
| `--icon-secondary-hover` | `ICON_SECONDARY_HOVER` | theme.rs:116 | `#E5E5E5` |
| `--icon-tertiary` | `ICON_TERTIARY` | theme.rs:58 | `#A3A3A3` |
| `--icon-quaternary` | `ICON_QUATERNARY` | theme.rs:98 | `#737373` |
| `--icon-quaternary-hover` | `ICON_QUATERNARY_HOVER` | theme.rs:118 | `#A3A3A3` |
| `--icon-brand-primary` | `ICON_BRAND_PRIMARY` | theme.rs:114 | `#F97316` |
| `--danger` | `DANGER` | theme.rs:60 | `#EF4444` |
| `--info` | `INFO` | theme.rs:61 | `#0D99FF` |
| `--success` | `SUCCESS` | theme.rs:62 | `#22C55E` |
| `--warning` | `WARNING` | theme.rs:63 | `#EAB308` |
| `--focus-ring` | `FOCUS_RING` | theme.rs:108 | `#F97316` |
| `--focus-ring-error` | `FOCUS_RING_ERROR` | theme.rs:234 | `#EF4444` |
| `--badge-neutral-bg` | `BADGE_NEUTRAL_BG` | theme.rs:128 | `#171717` |
| `--badge-neutral-border` | `BADGE_NEUTRAL_BORDER` | theme.rs:130 | `#404040` |
| `--badge-neutral-text` | `BADGE_NEUTRAL_TEXT` | theme.rs:132 | `#D4D4D4` |
| `--agent-1` | `AGENT_1` | theme.rs:255 | `#FFA629` |
| `--agent-label-text` | `AGENT_LABEL_TEXT` | theme.rs:260 | `#0A0A0A` |
| `--home-thumb-bg` | `HOME_THUMB_BG` | theme.rs:966 | `#262626` |
| `--tile-spec-bg` | `TILE_SPEC_BG` | theme.rs:176 | `#431407` |
| `--tile-spec-glyph` | `TILE_SPEC_GLYPH` | theme.rs:178 | `#FB923C` |
| `--tile-design-bg` | `TILE_DESIGN_BG` | theme.rs:164 | `#172554` |
| `--tile-design-glyph` | `TILE_DESIGN_GLYPH` | theme.rs:166 | `#60A5FA` |
| `--scroll-thumb` | `VAR_SCROLL_THUMB` | theme.rs:988 | `#737373` |
| `--sample-sheet` | `READER_LIGHT_DOC_SHEET` | theme.rs:206 | `#FFFFFF` |
| `--sample-page` | `READER_LIGHT_BG_PAGE` | theme.rs:204 | `#FAFAFA` |
| `--sample-surface` | `READER_LIGHT_SURFACE_2` | theme.rs:210 | `#F5F5F5` |
| `--sample-border` | `READER_LIGHT_BORDER` | theme.rs:216 | `#E5E5E5` |
| `--sample-text` | `READER_LIGHT_TEXT` | theme.rs:220 | `#171717` |
| `--sample-muted` | `READER_LIGHT_TEXT_SUBTLE` | theme.rs:224 | `#525252` |
| `--sample-ok` | `READER_LIGHT_DOT_OK` | theme.rs:226 | `#16A34A` |

Not colour constants (sizes and type), same file: `INSET_MODAL` :477, `BADGE_MD` :496, `MENU_ITEM_HEIGHT` :668, `SUBMENU_OVERLAP` :697, `HOME_CARD_GAP` :963, font roles `font_menu` :343.

## Consistent sample world (acceptance 3)

One project "Checkout": files Checkout-web (6 frames: Cart, Payment, Order confirmed, Card declined, Processing, Empty cart) and Checkout-mobile (3 frames: Cart, Payment, Order confirmed). Pinned: Screens = Cart, Payment, Order confirmed; States under Payment = Card declined, Processing. Script `sample-world-check.mjs` reads the doc tree of every screen; `title-check.mjs` compares each page H1 with the selected tree row.

```
m01-home-open-project.html                       (no doc tree)
m01b-home-project-header.html                    (no doc tree)
19 screens with a doc tree; distinct tree label sets: 3
1 screens
  Overview 1 / Checkout / Spec / No spec pages / Screens / No pinned screens / States / No pinned states / Discovery / No discovery pages
  head/tab variants: Checkout 2 files|Home Checkout · Project Checkout-web
17 screens
  Overview 1 / Checkout / Spec 4 / Payment methods / Payment flow / Cart rules / Error handling / Screens 3 / Cart / Payment / Order confirmed / States 2 / Payment / Card declined / Processing / Discovery 2 / Checkout research / Competitor flows
  head/tab variants: Checkout 2 files|Home Checkout · Project Checkout-web || Checkout Saving…|Home Checkout · Project Checkout-web
1 screens
  Overview 2 / Checkout / checkout-spec · spec v2 / Spec 4 / Payment methods / Payment flow / Cart rules / Error handling / Screens 3 / Cart / Payment / Order confirmed / States 2 / Payment / Card declined / Processing / Discovery 2 / Checkout research / Competitor flows
  head/tab variants: Checkout 2 files|Home Checkout · Project Checkout-web
18 pages with H1; 0 mismatches
```

Doc pages are identical on every screen: Overview (Checkout), Spec (Payment methods, Payment flow, Cart rules, Error handling), Discovery (Checkout research, Competitor flows). Deliberate variants: M02 shows the empty tree of a new project, M12 adds the linked package row, M10b swaps "2 files" for "Saving…". The Home sidebar and grid list the same four projects (Checkout, Onboarding, Design system, Marketing site). M06 is a scrolled capture (the H1 is above the fold, kept as a visually hidden `<h1>`); M13 shows only the tree.

## Layout geometry, re-measured (acceptance 5)

Headless Chrome, `getBoundingClientRect()` after fonts loaded (`measure-geom.mjs` over all 21 mockups (round 3 re-measure: 22 mockups, section "Round 3 fixes"); `geom-m03.mjs` for the table). Rail 48, tree x48 w240, column x399 w690, contents x1137 w192 are unchanged on every screen that has the region:

```
rail
  x/w = null -> 2 files m01-,m01b
  x/w = 0/48 -> 19 files 
tree
  x/w = null -> 2 files m01-,m01b
  x/w = 48/240 -> 19 files 
col
  x/w = null -> 3 files m01-,m01b,m13-
  x/w = 399/690 -> 18 files 
otp
  x/w = null -> 4 files m01-,m01b,m02-,m13-
  x/w = 1137/192 -> 17 files 
tabstrip
  x/w = 0/1440 -> 21 files
```

(`null` = region not on that screen: M01/M01b are Home and have no rail or tree; M13 has no reading column; M02 has fewer than 2 headings so no "On this page".)

M03 detail:

| Region | selector | x | y | w | h |
|---|---|---|---|---|---|
| tab strip | `.tabstrip` | 0 | 0 | 1440 | 32 |
| rail | `.rail` | 0 | 32 | 48 | 868 |
| tree | `.tree` | 48 | 32 | 240 | 868 |
| tree header | `.tree-head` | 48 | 32 | 239 | 65 |
| Documents row | `.tree-title` | 48 | 97 | 239 | 40 |
| group row (first) | `.tree .g` | 56 | 137 | 223 | 32 |
| item row (first) | `.tree .r` | 56 | 169 | 223 | 32 |
| reading column | `.col` | 399 | 80 | 690 | 1281 |
| On this page | `.otp` | 1137 | 80 | 192 | 104 |
| otp item (first) | `.otp a` | 1137 | 106 | 192 | 26 |
| embed header | `.embed-h` | 400 | 858 | 688 | 36 |
| Home tab | `.tab.home` | 8 | 3.5 | 72 | 24 |
| Project tab | `.tab.proj` | 92 | 2.5 | 240 | 26 |

Column top y80 = 32 tab strip + 48 `SPACE_REGION`. Main region x288 w1152; 399 = 288 + (1152 - 930) / 2. Tree rows x56 w223 = 8 inset each side of the 239 px panel plus 1 px border.

## Copy deck proof (acceptance 4)

`copy-deck-check.py` reads §15 of part 2 and `texts.json` (every visible text node and `aria-label` / `title` / `placeholder` of the 21 mockups, from `dump-text.mjs`, sample frame renders excluded). A string passes if it is in §15 (exact, per-cell, or a template such as `<project> · Project`), or is on the explicit existing-Studio-chrome list (`studio-existing.json`: Home sidebar, rail and tab names that exist today), or on the sample-content list (`sample-content.json`: page titles, table cells, file and frame names, error lines, captions that annotate the mockup, counts). Red-probe: without the two lists it reports 162 unclassified strings and exits 1.

```
deck strings: 170 | unique visible strings: 308
in deck: 127 | sample: 122 | existing Studio chrome: 59
UNCLASSIFIED: 0
--- red probe (no allow-lists): deck strings: 170 | unique visible strings: 308 | in deck: 127 | sample: 19 | existing Studio chrome: 0 | UNCLASSIFIED: 162
```

Sample-content list is reviewed by eye: it holds no product chrome (all chrome copy is in §15). New §15 rows added this round: slash menu sections and items, slash hints, shortcuts, selection toolbar, table menus, accessible names, KPI sublabel, embed state text, embed header template.

## Colour literal proof

```
grep -nE '#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\(|oklch\(' shell.css index.html m*.html   -> 0 matches
tokens.css: 27 distinct #RRGGBB values, every one present in theme.rs (script check: missing = [])
box-shadow / drop-shadow / text-shadow in shell.css, index.html, m*.html            -> 0 matches
purple / violet / indigo in tokens.css, shell.css                                    -> 0 matches
emoji probe: rg '[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]' (red-probe on "✓" exit 0 = can go red)  -> none
```

Alpha tokens that are derived values the specs name: `--border-translucent` (BORDER_TRANSLUCENT), `--bg-selected-area` (BG_SELECTED_AREA), `--bg-overlay` (BG_OVERLAY), `--bg-tertiary-60` (spec: embed header BG_TERTIARY at 60 %), `--agent-1-60` (spec: AGENT_1 at 60 % for the write flash). Sample frame renders use `READER_LIGHT_*` values from theme.rs.

## Gates (acceptance 7)

Commands run per file, 22 files (21 mockups + `index.html`), from `ui-mockups/`, logs in the scratch folder `gates/`:

| Gate | Command | Result |
|---|---|---|
| ui gate | `ui gate <file>` | exit 1 on all 22. Causes: (1) `token-coverage` 18 to 22 % (floor 80 %): the gate grades `var(--x)` names against the project's design:os DS, the mockups use their own `tokens.css` names; every value behind them is a theme.rs value (script above). Same as round 1, not a pass. (2) `a11y low-contrast`, 2 per mockup (1 on M01, M01b; 0 on index): text `#737373` on `#0A0A0A` 4.18:1 and on `#262626` 3.19:1. The elements are SVG icons (and the placeholder spans) painted with `ICON_QUATERNARY` / `TEXT_PLACEHOLDER`, mandated by ruling R0. Icons are non-text (WCAG 1.4.11, 3:1 floor: 4.18 and 3.19 pass); the kit's own test table exempts both (`widgets/kit/contrast_table.rs:58` placeholder, `:108-112` icon floor 3.0). New versus round 1, because round 1 drew icons and placeholders in `TEXT_TERTIARY` / `ICON_TERTIARY` against the kit. layout, content: clean on all 22. Remaining warnings only: `off-system-token` (~520 per file, same names repeated), `container-nesting-depth` (menus and sample frame renders), `z-index-off-ladder` (15, 25, 31), `radius-sprawl`, `font-scale-sprawl` (8 sizes now; round 1: 9), `shape-assembled-illustration` (rail tiles), `nested-cards` (sample frame fields in the embed card), `cramped-padding` x8 (height-centred rows), `overused-font` x1 (Inter on M12). |
| ui gate fixes this round | | `--page-h` declared in `:root`; `.fcard .bd` renamed `.fcard .fbd` (collided with the frame render `.fr .bd`: blue text on white, 2.54:1, found by axe); sample-frame font sizes folded onto 8 distinct values and literal 12 / 13 px declarations raised to 14 px in the mobile render (`tiny-body-text`); `.mini i.g` text colour; border 4 px to 2 px on the empty-cart bag; step labels varied per frame (`repeated-container-text`); `<h1>` for M06; `aria-disabled` on the disabled menu row. |
| slop-detect | `~/.claude/skills/es-designer/scripts/vendor/slop-detect/slop-detect detect <file>` | exit 2 on all 22, 208 findings, same classes as round 1: `cramped-padding` x181 (fixed `height` rows with flex centring, e.g. `.mi`, `.r`, `.tab`: false positive, inset is the 8 px padding), `clipped-overflow-container` x19 (`html/body/.app overflow: clip` is the 1440 x 900 artboard crop), `overused-font` x7 (Inter is Studio's chrome font, P5), `gray-on-color` x1 (M03 selected-block tint `BG_SELECTED_AREA`, `#D4D4D4` on the blended fill is about 11:1). `repeated-container-text`: 0 (round 1: 2). |
| a11y-audit (axe, rendered) | `a11y-audit file://<abs>/<file> --tags wcag2a,wcag2aa,wcag21aa` | 16 of 22 files exit 0 (0 violations). 6 files exit 1 with one `color-contrast` rule each (`.ph` on M05, M07, M08; `.hs-search > span` on M01, M01b; the same span inside the index iframe): placeholder text `TEXT_PLACEHOLDER` `#737373` on `#0A0A0A`, 4.17:1, below 4.5:1. Ruling R0 / critique G06 prescribe that colour; the kit exempts placeholders at 3.0:1. NEW vs round 1 (axe was clean). Listed under "Needs owner"; not hidden. |
| DS drift | script | 61 `theme.rs` constants cited, 0 mismatches; 27 distinct hex values, 0 missing from theme.rs. |
| probe-render.mjs, build-evidence lint, taste-duel | | NOT RUN: fixed 1440 x 900 artboards, responsive checks skipped by owner default, no builder loop (reference mockups, not product code). |

Index behaviour (`index-test.mjs`, Playwright, 0 page errors): list has 21 entries; clicking `#m07` loads M07 in the iframe; Next goes to M08, Prev back to M07; ArrowRight goes to M08, ArrowLeft twice to M06; deep links `#m11c` and `#m09full` load the right file; M09 full is scaled to 0.305 to fit (iframe 1440 x 2630); "Open full size" points at the selected file.

## Decisions and ambiguities (choice made)

1. **Fonts.** `tokens.css` loads Inter and Roboto Mono from `crates/designos-fonts/assets/` with paths relative to this folder; the mockups need a checkout that contains those files.
2. **Icons.** Phosphor regular, see the top of this file.
3. **Agent name tag text.** `AGENT_LABEL_TEXT` `#0A0A0A` on `AGENT_1` (white on it is under 2:1); the spec text "white" was wrong. Spec 2 §9 says `AGENT_LABEL_TEXT`.
4. **Focus ring.** The kit draws a 2 px band 2 px outside the control (`focus.rs:24`); ruling Q4 says "1 px `FOCUS_RING`". Kit used (R0). Inside table cells and full-width tree rows the band is drawn inside because there is no room outside.
5. **Dialog position.** `home/dialog.rs` centres confirm dialogs 24 px above the window centre; M13b follows it (M13b-04). The picker (M07, M08) is exactly centred.
6. **Check column in "Turn into".** Kit check column (check x 12, icon x 32, label x 60), not a right-aligned check.
7. **Agent tag position.** Right of the caret on the caret's line, 24 tall (`BADGE_MD`), so it never covers text above.
8. **Held chip.** M11c uses the canvas chip anatomy with the text "Claude is waiting [2] changes".
9. **KPI.** Strip sits under the title and description, tiles content-top-aligned; fourth tile gets a third line "Open: G5 Release". Package name "checkout-spec · spec v2".
10. **Slash menu filter "ta".** Substring match on labels gives State (Design) and Table (Data); Table highlighted. M04b shows the unfiltered menu twice (top, scrolled to end) because the 344 px cap cannot show all rows (round 3, ruling S1: it cuts the tenth Basic row at half height).
11. **Table.** The hide-scrollbars capture flag removes native bars, so the horizontal scrollbar is drawn (track 11 px = `VAR_SCROLL_H`, thumb `VAR_SCROLL_THUMB`, thumb length 690 x 688 / content width; round 3: content 954 = 922 table + 32 add-column strip, thumb 499; ruling S4). Column handle inside its header cell, row handle outside the table, cell focus 2 px inside.
12. **Picker sizes.** Head 60, footer 76, left column padding 16 / 24, right column padding 16 / 24, tile thumbnail 144, name 8 below, 3 columns gap 16. Spec 2 §5 carries these values now.
13. **M03 selection.** The block with the open menu shows `BG_SELECTED_AREA` and its handle button is pressed.
14. **M12.** Tooltip opens 4 px to the right of the "Copy cho AI" button (so it does not touch the KPI strip above); toast follows the kit toast.
15. **M13.** The search state cannot coexist with rename / drag / context menu in one list, so a second specimen panel shows the same tree in search state for "ca" with a caption. The context menu opens at the pointer inside the "Checkout research" row.
16. **Values outside spec and theme.rs** (derived or sample only): 5 / 10 px link mark on linked-spec icons; 7 px scrollbar thumb inside the 11 px track; 20 px check circle and caret height; 38 px picker badge band; 560 px specimen height; 922 px table width (920 + 1 px frame each side; round 3); sizes inside the sample frame renders (scaled by CSS transform).
17. **Sample content.** Strings not in §15 are sample content: page titles, list items, table cells, file and frame names, error lines ("Out of memory while drawing 1 of 41 layers.", "The disk is full."), Home menu extras, captions that annotate a mockup ("Top", "Scrolled to end", "Existing link").

## Needs owner

1. **Placeholder contrast.** Kit `TEXT_PLACEHOLDER` `#737373` is 4.18:1 on `BG`, below WCAG AA 4.5:1 for text; axe reports it on 6 files. Kept (R0, G06, kit test table exempts it at 3.0). Options: keep and exempt placeholders in the a11y gate, or lighten the token (for example `#8A8A8A`, 5.5:1) in `theme.rs` and the mockups. Conservative choice made: kit value.
2. **Icon contrast.** `ICON_QUATERNARY` `#737373` is 3.19:1 on `BG_TERTIARY`/`BG_SELECTED` and 4.18:1 on `BG`: passes the 3:1 non-text floor, trips `ui gate`'s text rule. Kept.
3. **M01-02 / M01-09.** `HOME_CARD_GAP` is 24 and P3 says 16 for Home tiles; Home header to grid measures 32 and P9 says 48. The mockup copies the product. The principles text needs an owner call.
4. **Lucide vs Phosphor** (see top).
5. **Focus ring width** (ruling Q4 wording says 1 px; kit is 2 px).
6. **Q7** (principles contrast note) stays out of scope as ruled.

## NOT verified

- Rendering in egui. These mockups define pixel intent only; egui text metrics (Inter at opsz 14 via the bundled variable font) can differ by a pixel.
- `ui gate` is not a pass (token-coverage and the two icon/placeholder contrast hits above); `slop-detect` exits 2 on design-intent findings; axe exits 1 on 6 files (placeholder).
- Hover, focus and motion states beyond the ones drawn (spec §14 durations are not animated here).
- 2x captures were inspected by eye (crops of tab strip, tree header, menus, table, picker header and footer, KPI strip, toast tooltip, agent flash and chip, delete dialog, Home cards, M08 tiles); sub-pixel text hinting is not asserted.
- Home screens (M01, M01b) are drawn from the Home source code, not from a live Studio capture.

## Rebuild

The HTML was generated by throw-away scripts in the scratch folder (not part of the deliverable). Edit the HTML/CSS here directly, or capture again with:

```
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars --window-size=1440,900 --screenshot=png/<name>.png file://$PWD/<name>.html
```

(`m09-embed-and-diagram-states-full.html` needs `--window-size=1440,2630`; `index.html#m07` is the index capture.)
