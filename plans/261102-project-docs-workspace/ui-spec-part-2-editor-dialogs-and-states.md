# UI spec part 2 — editor, dialogs, states, agent co-editing, KPI, narrow windows

Covers everything design-studio has no screen for. Same reconciliation rule as [part 1](ui-spec-ported-from-design-studio.md): chrome on Studio constants (`design/principles.md` P3–P5), document content on part 1's type scale. Where design-studio has nothing to copy, the pattern follows Notion (owner chose a "Notion-style" editor, P5) and Studio's existing menus and dialogs. Copy follows `design/ux-writing.md`: English chrome, sentence case, verb + object ≤ 3 words, counts in pills, no ellipsis on labels that only open a dialog. "Copy cho AI" keeps its owner-ruled label (D4).

Mockup for each section: [ui-mockups/](ui-mockups/README.md) (`Mxx` ids below). Captures at 1440 × 900 (owner default: responsive captures skipped; §11 still specifies the behaviour).

Shared chrome values used below:
- Menu / popover: radius `RADIUS_FRAME` 16, inset `INSET_FRAME` 8, bg `BG_MENU`, 1 px `BORDER_MENU`, no shadow (P4); items 32 tall (`CONTROL_XS`), radius 8, 12/18 400, 16 px icon + 8 + label, shortcut right-aligned in `TEXT_TERTIARY`; hover `BG_MENU_HOVER`; section label 12/18 500 `TEXT_TERTIARY`, 24 tall.
- Modal: radius 16, inset `INSET_MODAL` 24, title 16/24 600, body 14/20, buttons 36 tall (`CONTROL_SM`), radius 8, label 14 600; primary = `BG_BRAND`, secondary = `BG_TERTIARY`; buttons right-aligned, gap 8; scrim black 60 %.
- Toast: Studio's existing toast (radius 12, bottom centre), result first, then ` · ` hint.
- Focus ring: 1 px `FOCUS_RING` outside, 2 px offset, on every focusable control.

---

## 1. Opening a project (Home) — M01
- Project tile in Home (FILES & PROJECTS → a project): hover shows an **Open project** button (36 tall, secondary style) at the tile's top right; double-click on the tile also opens it.
- Project context menu gains **Open project** as its first item (above Rename).
- Inside a project's file grid, the header gets an **Open project** secondary button left of "+ Design file".
- Opening focuses an existing Project tab for that project if one is open; otherwise adds a tab at the end of the strip. Tab label "<project name> · Project", tile icon `TileKind::Project` (book icon, 16 px).
- Cmd+W closes it like any tab; unsaved edits never block close (edits auto-save, §8).

## 2. First open and empty states — M02
- A project with no `docs/` folder: on first open Studio creates `docs/overview.md` (title = project name, description "What this project is and where to start.") and the manifest, then shows that page selected. No dialog.
- Empty groups stay visible with a placeholder row in `TEXT_TERTIARY` italic-free 12/18: Spec "No spec pages", Screens "No pinned screens", States "No pinned states", Discovery "No discovery pages". The row is not selectable; its trailing "+" (24 px, visible on hover) does the group's add action.
- Empty page body (new page): one empty paragraph with placeholder "Type / for blocks" in `TEXT_TERTIARY`, visible only while the block is focused and empty.
- Search with no result: "No pages match" 12/18 `TEXT_TERTIARY`, centred under the search field, plus a **Clear search** text button.

## 3. Block editor — M03, M04, M05
The reading page is the editor (P5): click anywhere in content to place the caret. There is no separate edit mode.

### 3.1 Block hover and handle — M03
- Hovering a block shows, 32 px left of the column, two 24 px icon buttons in `ICON_TERTIARY`: **+** (insert block below, opens the slash menu at the new block) and **⋮⋮** (drag handle; click opens the block menu).
- Hovered block gets no background; the handle is the only change. Selected blocks (click handle, or Shift+arrow across blocks) get `BG_SELECTED_AREA` behind the block, radius 8.
- Block menu (popover, anchored to the handle): Turn into ▸ (Text, Heading 1, Heading 2, Heading 3, Bulleted list, Numbered list, Checklist, Callout, Code) · Duplicate (Cmd+D) · Copy cho AI · Copy link to block · section divider · Delete (Del), Delete in `DANGER` text.
- Drag: the block lifts at 80 % opacity following the pointer; a 2 px `TEXT` insertion line shows the drop gap; Esc cancels. Dropping onto an embed card's header is not a target.

### 3.2 Slash menu — M04
- Typing `/` at the start of an empty block, or after a space, opens the menu under the caret; typed characters filter it (case- and accent-insensitive); Up/Down move, Enter inserts, Esc closes and keeps the typed text.
- Width 320, max height 360 with scroll; sections and items (icon, label, a 12/18 `TEXT_TERTIARY` hint on the right):
  - **Basic**: Text · Heading 1 `#` · Heading 2 `##` · Heading 3 `###` · Bulleted list `-` · Numbered list `1.` · Checklist `[]` · Callout `>` · Divider `---` · Code ```` ``` ````
  - **Design**: Frame (opens the pin picker in "Embed" mode, §5) · State (same picker filtered to pinned states) · Diagram (inserts a Mermaid block with a flowchart starter)
  - **Data**: Table (inserts 3 × 3 with a header row)
- No match: one disabled row "No blocks match".

### 3.3 Inline formatting and links — M05
- Selecting text shows a floating toolbar 8 px above the selection: Bold (Cmd+B) · Italic (Cmd+I) · Code (Cmd+E) · Link (Cmd+K) · divider · Turn into ▸. The bar is radius `RADIUS_XL` 12 with inset 4, bg `BG_MENU`, 1 px `BORDER_MENU`; buttons 32 × 32, radius 8 (12 − 4, concentric per P4), active state `BG_SELECTED`.
- Link popover: one field "Paste a link or search pages" (36 tall) + result list of project pages; Enter applies; an existing link shows its URL with **Open** and **Remove link** buttons. External links open in the system browser; the tooltip says "Opens in browser" (ux-writing rule 13).
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
- Click a cell to edit; the focused cell gets a 1 px `FOCUS_RING` inside.
- Hovering the table shows a 24 px **+** at the end of the last row (add row) and the last column (add column).
- Hovering a row shows a 24 px row handle at the left edge outside the table; a column shows a handle above its header. Handle click opens a menu: Insert above / below (or left / right) · Move up / down · Delete row (column), Delete in `DANGER`.
- The first row is the header (Markdown requires one); "Turn into" is not offered for tables.
- Wide tables scroll horizontally inside the 690 column with a visible horizontal scrollbar; the header row does not stick.
- Cell content is inline text only (bold, italic, code, link). Enter in a cell moves down; Shift+Enter is ignored (Markdown cells are single-line).

## 5. Pin frame picker and Suggest from canvas — M07, M08
One modal, two modes.
- **Size**: 880 × 600, centred. Title "Pin frame" (Pin mode), "Embed frame" (Embed mode from `/frame`), "Suggest from canvas" (suggest mode).
- **Left column** (240): search field "Search frames" (36 tall); list of project files (32 rows, file icon + name + frame count pill); a file's pages expand under it (chevron). Selected file `BG_SELECTED`.
- **Right area**: grid of frame tiles, 3 columns, gap 16, tile = thumbnail (fit, on `BG_SECONDARY`, radius 12, 160 tall) + name 12/18 500 + page name 12/18 `TEXT_TERTIARY`. Already-pinned frames show a "Pinned" badge (`BADGE_SM` 22, pill) and are still selectable in Embed mode.
- **Group target** (Pin and Suggest modes): segmented control at the bottom-left "Screens | States"; when States is chosen a dropdown "Under screen" lists pinned screens (required).
- **Selection**: Pin and Embed modes are single-select (click selects, double-click confirms); Suggest mode is multi-select with a check circle on each tile and a count pill in the primary button.
- **Suggest mode content**: top-level frames of all project files that are not pinned yet, sorted by file then canvas order; header line "Frames not pinned yet" with the count pill.
- **Footer**: Cancel (secondary) · primary **Pin frame** / **Embed frame** / **Pin frames** with count pill. Primary disabled until a valid selection.
- **Loading**: thumbnails render lazily; a tile shows a `BG_SECONDARY` block with "Rendering…" until ready. A file that fails to open shows a row error "Can't open this file · Show in Home".

## 6. Embed card states — M09
| State | Body | Header |
|---|---|---|
| Rendering | `BG_SECONDARY` block at the last known aspect ratio (16:10 if unknown), centred "Rendering…" 12/18 `TEXT_TERTIARY` | normal |
| Ready | live render | normal |
| Updated while visible | crossfade 150 ms to the new render; no badge | normal |
| Frame not found | 120 tall, icon + "Frame not found" 14/20 600 + "It was deleted or moved to another file." 12/18 `TEXT_TERTIARY` + **Relink frame** secondary button (opens §5 in Embed mode) | file name kept; open button hidden |
| File not found | same layout, "File not found", text "The design file was deleted or moved out of this project.", button **Relink frame** | — |
| Render failed | same layout, "Can't render this frame", text = the error in one line, button **Try again** | normal |
| Hover | header open button turns `ICON`; card border `BORDER` → stays (no lift, no shadow) | — |

Click on the body or the open button → opens the file tab and frames the node (phase 03). Focus + Enter does the same.

## 7. Diagram (Mermaid) card — M09
- Same card frame. Header: diagram icon + `accTitle` or "Diagram"; right: **Edit diagram** (pencil, 24) toggles a source pane.
- Editing: the card splits vertically — source (Roboto Mono 13/20, `BG_SECONDARY`, min 120 tall, grows with content) above, preview below; preview updates 300 ms after typing stops.
- Syntax error: a 32-tall strip under the source, `DANGER` icon + "Line 4: unexpected token" 12/18; the preview keeps the last good render at 50 % opacity.
- Unsupported type: preview area shows "Diagram type not supported yet · flowchart and sequence work" 12/18 `TEXT_TERTIARY`; source stays visible.

## 8. Saving and conflicts — M10
- Auto-save 500 ms after the last edit. No Save button, no dirty dot on the Project tab.
- While a save is pending, the tree header's second line shows "Saving…" in `TEXT_TERTIARY`; after it lands it returns to "N files". On save failure: a banner (below).
- **External change, page not being edited**: the page reloads in place, scroll kept, no message.
- **External change while the user has unsaved edits on that page** — banner at the top of the reading column, full column width, radius 12, bg `BG_SECONDARY`, 1 px `WARNING` border, inset 12 16: `WARNING` icon + "This page changed outside Studio." + buttons **Load their version** (secondary) and **Keep mine** (primary). Keep mine writes the user's version; the other version is saved next to it as `<page>.conflict-<time>.md` and listed under Discovery.
- **Save failed** (disk full, permission): same banner style with `DANGER` border, "Can't save this page." + error in one line + **Try again**.

## 9. Agent co-editing in docs — M11
Same behaviour as the canvas (P12, `agent_presence.rs` colours and timings).
- **Agent cursor**: a 2 px vertical caret in the agent session's colour at the block it writes, with a name tag above (pill, 22 tall, session colour bg, 12/18 500 white text, e.g. "Claude"). Fades in 200 ms, holds 5 s after the last write, fades out 1.5 s.
- **Write flash**: the written block gets a 1 px outline in the session colour, radius 8, 200 ms attack / 400 ms hold / 300 ms release.
- **Held while typing**: while the user types in the same page, agent writes queue; a chip appears at the bottom centre of the reading column (pill, 28 tall, `BG_TERTIARY`, 12/18): agent dot + "Claude is waiting · 2 changes". It disappears when the queue applies (on 1 s typing pause or when the caret leaves the page).
- **Off-screen writes**: reuse the canvas "N changes → Show" chip (#695) at the bottom centre; **Show** scrolls to the first changed block. The view never moves by itself.
- **Undo**: each agent tool call is one Cmd+Z step; toast after the turn ends: "Claude edited 3 blocks · Cmd+Z to undo".
- **Tree**: a page being written shows the agent dot (8 px, session colour) right of its name.

## 10. Overview, KPI strip, linked spec, Copy cho AI — M12
- **Linked spec**: tree header "…" → **Link spec folder** → system folder picker. A linked package shows a 32-row line in the Overview group "<package> · spec v2" with an unlink action in its context menu (**Unlink folder**). Linked pages carry a small link mark on their icon; editing them writes the original file.
- **KPI strip** (only when a spec is linked): sits under the Overview page's title and description, 24 below the description and before the first heading; it is not part of the Markdown. 4 tiles in one row, gap 16, each tile radius 12, bg `BG_SECONDARY`, inset 16: value 20/30 600 `Roboto Mono` `TEXT` + label 12/18 `TEXT_TERTIARY`. Tiles: "REQs · US" (e.g. `24 · 11`), "ACs · scenarios" (`38 · 52`), "Screens · rows" (`13 · 96`, screens and UI-contract rows), "Gates approved" (`4/5`). A tile is a button: click selects the matching group in the tree. Values come from one loaded spec model (D3).
- **Copy cho AI**: in the block menu (§3.1) and as a 24 px icon button at the right of a heading on hover. Copies the D4 block format; toast "Copied for AI · paste into the agent".

## 11. Narrow windows
Behaviour is specified; captures are skipped by owner default.
- < 1200 px: "On this page" hides; the reading column stays 690 and recentres.
- < 1000 px: reading column shrinks to fill (min 480), 24 px side padding.
- < 800 px: the doc tree collapses to an overlay toggled by the existing left-panel toggle (Cmd+\\); the column takes the full width.

## 12. Tree interactions — M13
- **Context menu** (item): Open · Rename (F2) · Move up · Move down · Copy link to page · Edit source · divider · Delete (`DANGER`). Pinned frame items: Open in canvas · Replace frame · Unpin. Group: New page (Spec, Discovery, Overview) / Pin frame (Screens, States) · Collapse.
- **Rename**: inline field in the row (32 tall, 1 px `FOCUS_RING`), Enter saves, Esc cancels; renaming a page renames its file (slug) and updates links to it.
- **Delete page**: modal "Delete page?" · body "“<title>” moves to the Trash. Links to it will break." · Cancel · **Delete page** (destructive button, `DANGER` bg, focus ring `FOCUS_RING_ERROR`). Unpin needs no confirm (toast "Frame unpinned · Cmd+Z to undo").
- **Drag**: rows drag within their group only; dropping in another group is refused (row snaps back, no message).
- **New page**: "+" in the panel title row → menu: New overview page · New spec page · New discovery page; creates "Untitled" selected in rename mode.
- **Search**: filters by page title and heading text; matches highlight the query in `TEXT` 600 inside the row.

## 13. Accessibility and keyboard (chrome)
- Every icon-only button has a tooltip and an accessible name with the full meaning (ux-writing rule 14).
- Tab order: tab strip → tree header → tree → reading column → "On this page". Arrow keys move within the tree (Left/Right collapse/expand); Enter opens.
- Contrast: body `TEXT_SECONDARY` #D4D4D4 and muted `TEXT_TERTIARY` #A3A3A3 on `BG` #0A0A0A are both above 4.5 : 1; the mockup gate run (README) reports the exact ratios.
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
| Home tile / menu / project header | Open project |
| Tab label | <project> · Project |
| Tree panel title | Documents |
| Groups | Overview · Spec · Screens · States · Discovery |
| Empty groups | No spec pages · No pinned screens · No pinned states · No discovery pages |
| Empty block placeholder | Type / for blocks |
| Search | Search pages · No pages match · Clear search |
| Header menu | Link spec folder · Open in Home · Reveal in Finder |
| New page menu | New overview page · New spec page · New discovery page |
| Picker titles | Pin frame · Embed frame · Suggest from canvas |
| Picker | Search frames · Frames not pinned yet · Under screen · Pinned · Rendering… |
| Picker buttons | Cancel · Pin frame · Embed frame · Pin frames |
| Embed card | Open in canvas · Rendering… · Frame not found · File not found · Can't render this frame · Relink frame · Try again |
| Diagram card | Edit diagram · Diagram type not supported yet · flowchart and sequence work |
| Save | Saving… · This page changed outside Studio. · Load their version · Keep mine · Can't save this page. |
| Agent | <agent> is waiting · <n> changes · <agent> edited <n> blocks · Cmd+Z to undo |
| Delete | Delete page? · “<title>” moves to the Trash. Links to it will break. · Delete page |
| Toasts | Frame unpinned · Cmd+Z to undo · Copied for AI · paste into the agent |
| KPI labels | REQs · US · ACs · scenarios · Screens · rows · Gates approved |

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
