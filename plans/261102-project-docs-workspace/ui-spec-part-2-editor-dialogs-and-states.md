# UI spec part 2 — editor, dialogs, states, agent co-editing, KPI, narrow windows

Covers everything design-studio has no screen for. Same reconciliation rule as [part 1](ui-spec-ported-from-design-studio.md): chrome on Studio constants (`design/principles.md` P3–P5), document content on part 1's type scale. **Authority for chrome (ruling R0, [ui-polish-rulings-r1.md](ui-polish-rulings-r1.md)): Studio's shipped widget kit (`crates/designos-app/src/widgets/kit/*.rs`, `theme.rs`) beats this spec wherever they disagree.** Where design-studio has nothing to copy, the pattern follows Notion (owner chose a "Notion-style" editor, P5) and Studio's existing menus and dialogs. Copy follows `design/ux-writing.md`: English chrome, sentence case, verb + object ≤ 3 words, counts in pills, no ellipsis on labels that only open a dialog. "Copy cho AI" keeps its owner-ruled label (D4).

Mockup for each section: [ui-mockups/](ui-mockups/README.md) (`Mxx` ids below). Captures at 1440 × 900 (owner default: responsive captures skipped; §11 still specifies the behaviour).

Shared chrome values used below (each one is a kit look; the README "Token citations" table gives the source line):
- **Menu / popover**: radius `RADIUS_FRAME` 16, panel padding 8, fill `BG`, 1 px `BORDER_MENU`, no shadow (`menu_panel_look`, `popover_look`); rows 32 tall (`MENU_ITEM_HEIGHT`), highlight pill inset 8 and radius 8, label 12/18 **Medium** (`font_menu`) in `TEXT_SECONDARY`, icon 16 in `ICON_QUATERNARY` at x 16, label at x 40 with an icon and x 16 without; hover `BG_HOVER` + `TEXT_SECONDARY_HOVER` + `ICON_QUATERNARY_HOVER`; shortcut right-aligned 16 from the edge in `TEXT_QUATERNARY`; separator = full-width 1 px `BORDER` with 8 above and below; a menu with a check column (`MenuItem::checked`) puts the check at x 12, the icon box at x 32 and the label at x 60; submenu flush against the parent's right edge, its first row level with the parent row (`SUBMENU_OVERLAP` 0). A menu taller than its cap scrolls: rows clip 8 inside the panel and a 4 px `TEXT_TERTIARY` thumb sits 2 px from the right edge. Section label (slash menu only) 12/18 500 `TEXT_TERTIARY`, 24 tall. Destructive row: label `TEXT_ERROR`, icon `DANGER`.
- **Overlay offsets**: a popover or menu opens `SPACE_FACT` 4 below its anchor (handle, caret line box, selection line box, header button); the selection toolbar sits `SPACE_INLINE` 8 above the selection; a context menu opens with its top-left at the pointer (shifted up only when it would leave the window).
- **Modal** (`kit::modal_surface`, `home/dialog.rs`): scrim `BG_OVERLAY`, frame `BG`, radius 16, **no border**, inset `INSET_MODAL` 24, title 16/24 600 `TEXT`, body 14/20 `TEXT_TERTIARY`, buttons 40 tall (`CONTROL_MD`), radius 8, label 14/20 600, padding 16 and 80 wide at least, right-aligned, gap 8. Confirm dialogs are 440 wide and centred 24 px above the window centre. Esc or Cancel closes; a modal has no header close control.
- **Buttons** (`button_look`): primary `BG_BRAND` + `TEXT_ON_BRAND`; secondary `BG` + 1 px `BORDER_STRONG`, label `TEXT_SECONDARY`, icon `ICON_QUATERNARY`, hover `BG_HOVER` + `TEXT_SECONDARY_HOVER`; destructive `BG_ERROR_SOLID` + `TEXT_WHITE`; label 14/20 600 at every size. Sizes: 24 mini (inside cards), 32 `CONTROL_XS` (editor and banners), 36 `CONTROL_SM` (Home), 40 `CONTROL_MD` (dialogs).
- **Fields** (`field_look`, `search_look`): fill `BG`, 1 px `BORDER_STRONG`, radius 8, text `TEXT`, placeholder `TEXT_PLACEHOLDER`, icon `ICON_QUATERNARY`; focused = 2 px `BORDER_SELECTED` (no separate ring). Dropdown trigger (`dropdown_look`): `BG`, no border at rest, `BORDER` on hover.
- **Icon buttons** (`Hierarchy::Icon`): 24, radius 8, rest `ICON_QUATERNARY`, hover `BG_HOVER` + `ICON_QUATERNARY_HOVER`, pressed or menu open `BG_SELECTED` + `ICON_SECONDARY_HOVER`. Hover-revealed handles and "+" buttons all use it.
- **Checkbox** (`checkbox_look`): 16 box, radius 4, unchecked 1 px `BORDER_STRONG`, checked `BG_BRAND` fill with a `TEXT_ON_BRAND` check. Multi-select check circles use the same look.
- **Segmented** (`segmented_look`): shell `BG_SECONDARY` radius 8, tile inset 2 radius 6; selected tile `BG_SELECTED` + 1 px `BORDER_STRONG` + `TEXT_SECONDARY`, rest `TEXT_QUATERNARY`, labels 12/18 600.
- **Tooltip** (`tooltip_look`): `BG_PRIMARY_SOLID`, no border, radius 8, margin 12 × 8, `TEXT` 12/18.
- **Toast** (`toast_surface`, `shell/toast_layout.rs`): `BG_PRIMARY_ALT`, 1 px `BORDER_MENU`, radius 12, bottom 72, min width 240, 40 tall, an 8 px `ICON_BRAND_PRIMARY` status dot centred 16 from the left, message at x 28 in 14/20 500 `TEXT`, right padding 16. One text run: result first, then ` · ` hint.
- **Focus ring** (`focus_ring`): a 2 px `FOCUS_RING` band, 2 px outside the control (error buttons `FOCUS_RING_ERROR`); inside a table cell, where there is no room outside, the same 2 px band sits inside.

---

## 1. Opening a project (Home) — M01, M01b
- Home is Studio's existing Home (`home/mod.rs`): sidebar 264, header 64, filter bar, content padding 32. A project is a card in **All folders → Projects**: 260 × 168, radius 12, `BG_SECONDARY`, 1 px `BORDER` (hover 1.5 `BORDER_STRONG`), 2 × 2 preview (92 tall, inset 8), a neutral 22 px tile (`BG_TERTIARY` + frame glyph), name 14/20 600, meta "N files · M folders" 12/18 `TEXT_TERTIARY`, a **+ Folder** button 66 × 24 at the top right (8 px inside the card). The Folders heading sits 28 below the last project row (the 16 row gap plus 12). Card gap `SPACE_BLOCK` 16 (the card grid; file cards use `HOME_CARD_GAP` 24).
- Hover adds a 24 px **Open project** secondary button at the right end of the name row, vertically centred on the name line and 8 px inside the card (it never covers the preview); double-click on the card also opens it.
- Project context menu (`context_entries`, `Target::Project`) gains **Open project** as its first item: Open project · New folder · — · Rename · Move to trash. It opens with its top-left at the pointer.
- Inside a project's file grid (M01b) the header button group gets **Open project** as its first (leftmost) button, then "+ New folder", "Import", "+ FigJam board", "+ Design file" (36 tall, 8 apart).
- Opening focuses an existing Project tab for that project if one is open; otherwise adds a tab at the end of the strip. Tab label "<project name> · Project", tile `TILE_SPEC_BG` / `TILE_SPEC_GLYPH` with the book glyph (ruling Q3: no new token), 14 px, radius `RADIUS_MARK`.
- Cmd+W closes it like any tab; unsaved edits never block close (edits auto-save, §8).

## 2. First open and empty states — M02
- A project with no `docs/` folder: on first open Studio creates `docs/overview.md` (title = project name, description "What this project is and where to start.") and the manifest, then shows that page selected. No dialog.
- The tree header's second line "N files" counts the project's design files (the same number as the Home card).
- Empty groups stay visible with a placeholder row in `TEXT_TERTIARY` 12/18: Spec "No spec pages", Screens "No pinned screens", States "No pinned states", Discovery "No discovery pages". The text sits on the item label column (x 44 inside the row: 24 chevron box + 16 icon + 4). The row is not selectable and has no hover fill; hover only reveals the trailing "+" (24 px icon button), which does the group's add action.
- Empty page body (new page): one empty paragraph with placeholder "Type / for blocks" in `TEXT_TERTIARY`, visible only while the block is focused and empty. It starts at the column's left edge (x 399) with the caret drawn over its first letter.
- Search with no result: "No pages match" 12/18 `TEXT_TERTIARY`, centred under the search field, plus a **Clear search** text button (tertiary, 24 tall).

## 3. Block editor — M03, M04, M05
The reading page is the editor (P5): click anywhere in content to place the caret. There is no separate edit mode.

### 3.1 Block hover and handle — M03
- Hovering a block shows two 24 px icon buttons side by side whose right edge is 12 px left of the column (`SPACE_GROUP`; the pair spans 60 to 12 px left of the column): **+** (insert block below, opens the slash menu at the new block) and **⋮⋮** (drag handle; click opens the block menu).
- Hovered block gets no background; the handle is the only change. Selected blocks (click handle, or Shift+arrow across blocks) get `BG_SELECTED_AREA` behind the block (radius 8, 4 px vertical and 8 px horizontal inset around the text); the block's handle shows its pressed look. The selected fill starts 8 px left of the column, so the pressed handle keeps 4 px (`SPACE_FACT`) of clear space to it.
- Block menu (popover, 4 below the handle): Turn into ▸ (Text, Heading 1, Heading 2, Heading 3, Bulleted list, Numbered list, Checklist, Callout, Code; the current type carries the check) · Duplicate (Cmd+D) · Copy cho AI · Copy block link · divider · Delete (Del), Delete in `TEXT_ERROR` with a `DANGER` icon.
- Drag: the block lifts at 80 % opacity following the pointer; a 2 px `TEXT` insertion line shows the drop gap; Esc cancels. Dropping onto an embed card's header is not a target.

### 3.2 Slash menu — M04, M04b, M04c
- Typing `/` at the start of an empty block, or after a space, opens the menu 4 px under the caret line box; typed characters filter it (case- and accent-insensitive); Up/Down move, Enter inserts, Esc closes and keeps the typed text.
- Width 320, max height 344 with scroll (ruling S1: `MENU_PADDING` 8 + 328 + 8; the 328 is the section label 24 + 9 rows × 32 + half a row, i.e. 10.5 rows counting the label as the first). The cap always cuts the tenth Basic row (Code) at half its height, so the user sees that the list scrolls; the 4 px thumb (thumb length = 328² ÷ 520 = 206.9; its left edge is 6 px inside the panel's right edge, kit `scroll_thumb`) shows more below. M04b shows the list at the top, M04c the same menu scrolled to the end (list offset 192, thumb at the bottom of the band); one menu per capture. Sections and items (icon, label, a 12/18 `Roboto Mono` `TEXT_QUATERNARY` hint on the right, because the hints are Markdown syntax):
  - **Basic**: Text · Heading 1 `#` · Heading 2 `##` · Heading 3 `###` · Bulleted list `-` · Numbered list `1.` · Checklist `[]` · Callout `>` · Divider `---` · Code ```` ``` ````
  - **Design**: Frame (opens the pin picker in "Embed" mode, §5) · State (same picker filtered to pinned states) · Diagram (inserts a Mermaid block with a flowchart starter)
  - **Data**: Table (inserts 3 × 3 with a header row)
- No match: one disabled row "No blocks match" (M04 third specimen).

### 3.3 Inline formatting and links — M05
- Selecting text shows a floating toolbar 8 px above the selection: Bold (Cmd+B) · Italic (Cmd+I) · Code (Cmd+E) · Link (Cmd+K) · divider · Turn into ▸. The bar is radius `RADIUS_XL` 12 with inset 4, fill `BG`, 1 px `BORDER_MENU`; buttons 32 × 32, radius 8 (12 − 4, concentric per P4), tool look (`ICON_QUATERNARY_HOVER`, active `BG_SELECTED` + `ICON`); the **Turn into** text button label is the kit button label, 14/20 600.
- Link popover (4 below the selection's line box): one field "Paste a link or search pages" (36 tall) + a result list of project pages (the page being edited is not listed); each result row is a menu row: 32 tall, label 12/18 500 (`font_menu`), never the 14/24 reading-column type. Enter applies. An existing link shows its URL in the field with **Open** and **Remove link** secondary buttons (32 tall); **Open** carries the external-link glyph and the tooltip "Opens in browser" (ux-writing rule 13; that glyph is reserved for links that leave the app).
- Markdown shortcuts at block start: `# `, `## `, `### `, `- `, `* `, `1. `, `[] `, `> `, ```` ``` ````, `---`. Typing them converts the block; Cmd+Z right after restores the literal characters.

### 3.4 Keyboard map (editor)
| Keys | Action |
|---|---|
| Enter | New block of the same list type; on an empty list item, leave the list |
| Shift+Enter | Line break inside the block |
| Backspace at block start | Merge into previous block (or turn a heading/list into text first) |
| Tab / Shift+Tab | Indent / outdent list item; in a table, next / previous cell |
| Cmd+Z / Shift+Cmd+Z | Undo / redo (typing is grouped per 1 s pause or block change) |
| Cmd+A | Select block text; again: select all blocks |
| Cmd+Shift+Up/Down | Move block up / down |
| Cmd+/ | Open the block menu for the current block |
| Esc | Select the current block (then arrows move between blocks) |

## 4. Table editing — M06
- Click a cell to edit; the focused cell gets the 2 px `FOCUS_RING` band inside.
- Hovering the table shows a 24 px **+** (icon button, rest look: only the control under the pointer takes the hover look) under the table at the reading column's left edge (add row; it does not scroll) and 8 px right of the last column (add column; ruling S4: it belongs to the last column and scrolls with the table, so at scroll start it is out of view).
- Hovering a row shows a 24 px row handle at the left edge outside the table; a column's handle sits inside its header cell at the right end (never above the table border). Handle click opens a menu 4 below it: Insert above / below (or left / right, with directional glyphs) · Move up / down (left / right) · Delete row (column), Delete in `TEXT_ERROR`.
- The first row is the header (Markdown requires one); "Turn into" is not offered for tables.
- A table wider than the 690 column scrolls horizontally inside its own box (no page scroll) with the kit horizontal scrollbar visible under it (track 690 × 11, thumb 7 tall, `VAR_SCROLL_THUMB`, thumb length = track × viewport ÷ content = 690 × 690 ÷ content width, 499 for the 954 sample); the header row does not stick. The table frame (1 px `BORDER`, radius 12) scrolls with its cells, so at scroll start only the left corners are rounded and the first column that does not fit is cut at the box edge well inside the column (M06: Notes shows 89 of 320 px), which reads as scrollable. Scrolled to the end (M06b) the frame closes with its right corners and the add-column **+** sits 8 px right of the last column, flush with the box's right edge. The row handle and add-row **+** stay on the column's left edge and do not scroll.
- **Column width (ruling T1).** A column is as wide as its widest cell text (header included) plus the 12 px insets on both sides (the 1 px divider on its right edge counts inside the column; the last column has none), at least 120 and at most 320. Text that is still longer wraps inside the cell and the row grows; a cell never shows an ellipsis, because hiding cell text hides data. Body cells are top-aligned (8 px above the first line, 20 px lines), header cells centred in their 36 px row; a row handle sits level with the row's first line. Sample: Method, Fee, Settlement, Refund window and Limit are 120 each, Notes is 320 and its Card cell wraps to two lines (row 1 is 56 tall), so the table is 922 wide (6 columns = 920 + the 1 px frame each side).
- Cell content is inline text only (bold, italic, code, link). Enter in a cell moves down; Shift+Enter is ignored (Markdown cells are single-line).

## 5. Pin frame picker and Suggest from canvas — M07, M08
One modal, two modes.
- **Size**: 880 × 600, centred in the window. Head 60 (title 16/24 600 at inset 24), footer 76, 1 px `BORDER` between regions. Title "Pin frame" (Pin mode), "Embed frame" (Embed mode from `/frame`), "Suggest from canvas" (suggest mode).
- **Left column** (240, padding 16 / 24): search field "Search frames" (36 tall), `SPACE_GROUP` 12 below it the list; rows 32 tall with no gap. A file row = 24 chevron box + file icon 16 + `SPACE_FACT` 4 + name (label at x +44, like a doc tree parent row) + count pill = the number of frames the grid lists for that file (ruling T3: all frames in Pin and Embed modes, unpinned frames in Suggest mode); it is not the number of page rows; selected file `BG_SELECTED` (kit row look, weight 600). A file's pages expand under it as plain depth-1 rows (icon and label each 16 further right: page glyph at +40, label at +60) and filter the grid when clicked. Suggest mode adds **All files** as the first row (layers glyph at +24, label at +44 like every other row, no chevron, count = frames not pinned yet).
- **Right area** (padding 16 / 24, `SPACE_GROUP` 12 below an optional header): grid of frame tiles, 3 columns, gap 16, tile = thumbnail (fit inside the area below a 38 px badge band, bottom-aligned, on `BG_SECONDARY`, radius 12, 144 tall) + name 12/18 500 8 below it + page name 12/18 `TEXT_TERTIARY`. Already-pinned frames show a **Pinned** status tag (`status_tag`: neutral tint triple, radius 8, `BADGE_SM` 22) and are still selectable in Embed mode. Selected tile: 2 px `BORDER_SELECTED` outline inside the thumbnail.
- **Group target** (Pin and Suggest modes): kit segmented control "Screens | States" at the bottom left of the footer; when States is chosen, the label "Under screen" and a dropdown listing pinned screens follow it (required).
- **Selection**: Pin and Embed modes are single-select (click selects, double-click confirms); Suggest mode is multi-select with a check circle (kit checkbox look) on each tile. The footer starts with a count pill and "selected" ("3" pill + selected); the primary button never carries a count (ruling Q5).
- **Suggest mode content**: top-level frames of all project files that are not pinned yet, sorted by file then canvas order; header line "Frames not pinned yet" with the count pill. The grid scrolls inside the area.
- **Footer**: Cancel (secondary) · primary **Pin frame** / **Embed frame** / **Pin frames**. Primary disabled until a valid selection.
- **Loading**: thumbnails render lazily; a tile shows a `BG_SECONDARY` block with "Rendering…" until ready. A file that fails to open shows a row error "Can’t open this file · Show in Home".

## 6. Embed card states — M09
Buttons in state panels are secondary, 32 tall (`CONTROL_XS`), label 14/20 600. Error states carry the alert glyph in `ICON_TERTIARY`.
| State | Body | Header |
|---|---|---|
| Rendering | `BG_SECONDARY` block at the last known aspect ratio (16:10 if unknown), centred "Rendering…" 12/18 `TEXT_TERTIARY` | normal |
| Ready | live render (max height 420) | normal |
| Ready, pointer over the card | as Ready; the header open button takes the icon-button hover look (`BG_HOVER` + `ICON_QUATERNARY_HOVER`); the card border does not change (no lift, no shadow) | open button hover |
| Updated while visible | crossfade 150 ms to the new render; no badge | normal |
| Frame not found | 120 tall panel: alert glyph + "Frame not found" 14/20 600 + "It was deleted or moved to another file." 12/18 `TEXT_TERTIARY` + **Relink frame** (opens §5 in Embed mode) | file name kept; open button hidden |
| File not found | same layout, "File not found", text "The design file was deleted or moved out of this project.", button **Relink frame** | file name kept; open button hidden |
| Render failed | same layout, "Can’t render this frame", text = the error in one line, button **Try again** | normal |

Embed header = 36 tall, inset 0 8 0 16 (the 16 px matches the body inset), the title and the file name are one text run "Payment · Checkout-web" (spaces both sides of the dot). Click on the body or the open button → opens the file tab and frames the node (phase 03). Focus + Enter does the same.

## 7. Diagram (Mermaid) card — M09
- Same card frame. Header: diagram icon + `accTitle` or "Diagram"; right: **Edit diagram** (pencil, 24) toggles a source pane.
- Editing: the card splits vertically — source (Roboto Mono 13/20, `BG_SECONDARY`, min 120 tall, grows with content) above, preview below; preview updates 300 ms after typing stops.
- Syntax error: a 32-tall strip under the source, `DANGER` icon + "Line 4: unexpected token" 12/18, the line number in `TEXT_ERROR` in the gutter; the preview keeps the last good render at 50 % opacity.
- Unsupported type: preview area shows "Diagram type not supported yet · flowchart and sequence work" 12/18 `TEXT_TERTIARY`; source stays visible.

## 8. Saving and conflicts — M10, M10b, M10c
- Auto-save 500 ms after the last edit. No Save button, no dirty dot on the Project tab.
- While a save is pending, the tree header's second line shows "Saving…" in `TEXT_TERTIARY` (M10b); after it lands it returns to "N files". On save failure: a banner (below).
- **External change, page not being edited**: the page reloads in place, scroll kept, no message.
- **External change while the user has unsaved edits on that page** (M10): banner at the top of the reading column, full column width, radius 12, bg `BG_SECONDARY`, 1 px `WARNING` border, inset 12 16, 24 (`SPACE_SECTION`) above the page title: `WARNING` icon + "This page changed outside Studio." + buttons **Load their version** (secondary, 32) and **Keep mine** (primary, 32). **Saves are held while the banner is open**: the header keeps "N files" and never shows "Saving…". Keep mine writes the user's version; the other version is saved next to it as `<page>.conflict-<time>.md` and listed under Discovery.
- **Save failed** (M10c; disk full, permission): same banner style with a `DANGER` border and icon, "Can’t save this page." + the error in one line + **Try again** (secondary, 32).

## 9. Agent co-editing in docs — M11, M11b, M11c
Same behaviour as the canvas (P12, `agent_presence.rs` colours and timings). One state per capture.
- **Agent caret** (M11): a 2 px vertical caret in the agent session's colour at the end of the last block it wrote, with a name tag right of it (pill `BADGE_MD` 24, session colour fill, 12/18 500 `AGENT_LABEL_TEXT`, e.g. "Claude"; the tag sits on the caret's line so it never covers text above). Fades in 200 ms, holds 5 s after the last write, fades out 1.5 s. The tag is always shown with the flash.
- **Write flash** (M11): the written block gets a 2 px outline in the session colour at 60 %, radius 8, 200 ms attack / 400 ms hold / 300 ms release. Focus rings stay `FOCUS_RING` bands, so the two never read alike (ruling Q4).
- **Held while typing** (M11c): while the user types in the same page, agent writes queue; a chip appears at the bottom centre of the reading column using the canvas chip anatomy: pill 32 tall in the session colour, text "Claude is waiting" + a mono count pill + "changes". It disappears when the queue applies (on 1 s typing pause or when the caret leaves the page).
- **Off-screen writes** (M11b): the canvas chip from #695 (`canvas/agent_changes_chip.rs`) at the bottom centre of the reading column: pill 32 tall in the session colour, inset 8, a separate count pill (16 tall, `BG_TERTIARY`, `Roboto Mono` 12), "changes", "Show", and a direction caret (down here), 4 between parts, labels 12/18 500 `AGENT_LABEL_TEXT`. The whole chip is the button; it scrolls to the first changed block. The view never moves by itself.
- **Undo**: each agent tool call is one Cmd+Z step; toast after the turn ends: "Claude edited 3 blocks · Cmd+Z to undo".
- **Tree**: a page being written shows the agent dot (8 px, session colour) right of its name.

## 10. Overview, KPI strip, linked spec, Copy cho AI — M12
- **Linked spec**: tree header "…" → **Link spec folder** → system folder picker. A linked package shows a 32-row line in the Overview group "<package> · spec v2" (e.g. "checkout-spec · spec v2") with an unlink action in its context menu (**Unlink folder**). Linked pages carry a small link mark on their icon; editing them writes the original file.
- **KPI strip** (only when a spec is linked): sits under the Overview page's title and description, 24 below the description and before the first heading; it is not part of the Markdown. 4 tiles in one row, gap 16, each tile radius 12, bg `BG_SECONDARY`, inset 16, content top-aligned: value row exactly 30 tall, 20/30 600 `Roboto Mono` `TEXT` (the "·" separator in `Inter` `TEXT_TERTIARY` with 4 px each side, not a mono space) + label 12/18 `TEXT_TERTIARY`. Tiles: "REQs · US" (e.g. `24 · 11`), "ACs · scenarios" (`38 · 52`), "Screens · rows" (spec screens and UI-contract rows, e.g. `3 · 24`), "Gates approved" (`4/5`) with a third line naming the open gate ("Open: G5 Release"). A tile is a button: click selects the matching group in the tree, so the numbers come from the same loaded spec model as the tree (D3).
- **Copy cho AI**: in the block menu (§3.1) and as a 24 px icon button 8 px right of a heading on hover (tooltip "Copy cho AI" 4 px to its right, so it never covers the heading above). Copies the D4 block format; toast "Copied for AI · paste into the agent".

## 11. Narrow windows
Behaviour is specified; captures are skipped by owner default.
- < 1200 px: "On this page" hides; the reading column stays 690 and recentres.
- < 1000 px: reading column shrinks to fill (min 480), 24 px side padding.
- < 800 px: the doc tree collapses to an overlay toggled by the existing left-panel toggle (Cmd+\\); the column takes the full width.

## 12. Tree interactions — M13, M13b
- **Context menu** (item): Open · Rename (F2) · Move up · Move down · Copy page link · Edit source · divider · Delete (`TEXT_ERROR`). Pinned frame items: Open in canvas · Replace frame · Unpin. Group: New page (Spec, Discovery, Overview) / Pin frame (Screens, States) · Collapse. "Open" uses the reader glyph; "Open in canvas" uses the frame glyph. The external-link glyph is never used for in-app opens.
- **Rename**: the row keeps its icon; the kit field look (`BG`, 2 px `BORDER_SELECTED`) sits over the label inside the row (4 px inset top and bottom, 24 tall, right edge 8 inside the row). Enter saves, Esc cancels; renaming a page renames its file (slug) and updates links to it.
- **Delete page** (M13b): the kit confirm dialog (§ shared Modal): title "Delete page?", two body lines "“<title>” moves to the trash." (Studio's own trash, ruling S2; lowercase in sentences, "Trash" only as the Home section label) and "Links to it will break." (20 tall lines, 8 between, 8 before the buttons, so the dialog is 440 × 180 as `home/dialog.rs` sizes it), **Cancel** (secondary) and **Delete page** (destructive). **Initial focus is on Cancel** (ring on Cancel; Enter cancels), the ring moves to Delete only when tabbed there. Unpin needs no confirm (toast "Frame unpinned · Cmd+Z to undo").
- **Drag**: rows drag within their group only; dropping in another group is refused (row snaps back, no message). The dragged row dims to 60 %; the ghost follows the pointer (+12, +8) as an opaque `BG_SELECTED` row with 80 % contents; the drop gap shows a 2 px `TEXT` line.
- **New page**: "+" in the panel title row → menu: New overview page · New spec page · New discovery page; creates "Untitled" selected in rename mode.
- **Search**: filters by page title and heading text; matches highlight the query in `TEXT` 600 inside the row; the open page keeps its selected fill.

## 13. Accessibility and keyboard (chrome)
- Every icon-only button has a tooltip and an accessible name with the full meaning (ux-writing rule 14).
- Tab order: tab strip → tree header → tree → reading column → "On this page". Arrow keys move within the tree (Left/Right collapse/expand); Enter opens.
- Contrast: body `TEXT_SECONDARY` #D4D4D4 and muted `TEXT_TERTIARY` #A3A3A3 on `BG` #0A0A0A are both above 4.5 : 1; `TEXT_PLACEHOLDER` #737373 is 4.18 : 1 (placeholder text only); `TEXT_ERROR` on `BG` 7.16 : 1; the mockup gate run (README) reports the checked pairs.
- Reduced motion: no smooth scroll, no crossfade, no flash animation (static outline for 1 s instead).

## 14. Motion
| Event | Duration | Easing |
|---|---|---|
| Menu / popover open | 120 ms fade + 4 px rise | ease-out |
| Smooth scroll to heading | 200 ms | ease-out |
| Embed re-render crossfade | 150 ms | linear |
| Agent cursor fade in / out | 200 ms / 1.5 s | ease-out |
| Write flash | 200 / 400 / 300 ms | ease-in-out |
No bounce, no layout transitions (P8 frame budget).

## 15. Copy deck
| Where | String |
|---|---|
| Home project card / menu / project header | Open project |
| Home project card | + Folder · N files · M folders |
| Tab label | <project> · Project |
| Tree panel title | Documents |
| Groups | Overview · Spec · Screens · States · Discovery |
| Empty groups | No spec pages · No pinned screens · No pinned states · No discovery pages |
| Empty block placeholder | Type / for blocks |
| Search | Search pages · No pages match · Clear search |
| Header menu | Link spec folder · Open in Home · Reveal in Finder |
| New page menu | New overview page · New spec page · New discovery page |
| Block menu | Turn into · Duplicate · Copy cho AI · Copy block link · Delete |
| Tree menu | Open · Rename · Move up · Move down · Copy page link · Edit source · Delete |
| Link popover | Paste a link or search pages · Open · Remove link · Opens in browser |
| Picker titles | Pin frame · Embed frame · Suggest from canvas |
| Picker | Search frames · All files · Frames not pinned yet · Under screen · Screens · States · Pinned · selected · Rendering… |
| Picker buttons | Cancel · Pin frame · Embed frame · Pin frames |
| Embed card | Open in canvas · Rendering… · Frame not found · File not found · Can’t render this frame · Relink frame · Try again |
| Embed state text | It was deleted or moved to another file. · The design file was deleted or moved out of this project. |
| Diagram card | Edit diagram · Diagram type not supported yet · flowchart and sequence work |
| Save | Saving… · This page changed outside Studio. · Load their version · Keep mine · Can’t save this page. |
| Agent | <agent> is waiting · changes · Show · <agent> edited <n> blocks · Cmd+Z to undo |
| Delete | Delete page? · “<title>” moves to the trash. · Links to it will break. · Delete page |
| Toasts | Frame unpinned · Cmd+Z to undo · Copied for AI · paste into the agent |
| KPI labels | REQs · US · ACs · scenarios · Screens · rows · Gates approved |
| Slash menu | Basic · Design · Data · Text · Heading 1 · Heading 2 · Heading 3 · Bulleted list · Numbered list · Checklist · Callout · Divider · Code · Frame · State · Diagram · Table · No blocks match |
| Slash hints (Roboto Mono) | # · ## · ### · - · 1. · [] · > · --- · ``` |
| Shortcuts | Cmd+D · Del · F2 |
| Selection toolbar | Bold · Italic · Code · Link · Turn into |
| Table menus | Insert left · Insert right · Move left · Move right · Delete column · Delete row · Insert above · Insert below |
| Accessible names (icon buttons) | Add · Add column · Add row · Column handle · Row handle · Drag or open block menu · Insert block below · Search pages · Collapse all · New page · Project menu · Documents · On this page · Claude is writing · Open in canvas · Pin frame · Suggest from canvas · Edit diagram · Copy cho AI · Delete page? |
| KPI sublabel | Open: G5 Release |
| Embed header | <title> · <file> |

## 16. What each phase owns
| Section | Phase / issue |
|---|---|
| §1, §2, §11, §12, §13 | 02 · #1524 |
| §3, §4, §8 | 04 · #1526 (§8 save path: 01 · #1523) |
| §5, §6 | 03 · #1525 |
| §7 | 05 · #1527 |
| §9 | 06 · #1528 |
| §10 | 07 · #1529 |
| §14, §15 | every UI phase |
