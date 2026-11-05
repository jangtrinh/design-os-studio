# design:os studio — Design Principles (P1–P11)

These 11 binding laws govern every surface, widget, token, and interaction across `design:os studio`. Every change to `crates/designos-app` or `design/` is audited against `P1..P11` and enforced by contrast check C-1 (`token-contract.md` (f)) with the `theme.rs` test `chrome_contrast_pairs`, `ui ds soul check`, `scripts/zero_fake_ui_gate.py`, `scripts/design_os_self_evolve.py` and `scripts/check_spacing_relationships.py`.

The chrome is a neutral **dark** system: one colour hierarchy, one type scale, one spacing scale, one radius scale, hairline borders, one focus ring and one anatomy per widget. Studio keeps its own layout and arrangement, its orange brand and its Purple Ban (owner rulings R1–R7, 2026-10-25). Each law names values by their `theme.rs` constant. Credit: the chrome's colour, type, spacing and radius steps are adapted from the Untitled UI design system (https://www.untitledui.com); this repository ships no Untitled UI file, icon or kit export.

## P1 — Single Token Authority (`crates/designos-app/src/theme.rs` ↔ `design/design.tokens.json`)
All colors, typography scales, spatial increments, corner radii, and motion durations originate in `crates/designos-app/src/theme.rs` and are mirrored into `design/design.tokens.json` (77 DTCG tokens: 48 colours, 8 typography, 12 spacing, 6 radius, 3 durations; all 16 canonical roles mapped). The store keeps its role keys; their values are the dark values, set one token at a time with `ui ds change-token`, and each changelog reason says what the value is for. Inline `Color32` literals in panels or views outside `theme.rs` are forbidden unless explicitly allowlisted for semantic canvas overlays, identity tints or GPU shader debug modes; `scripts/check_chrome_colour_literals.py` counts them per file and the count may only fall.

Values with no store key (for example `TEXT_SECONDARY_HOVER`, `RADIUS_MARK`, `INSET_SEGMENT`, `FONT_SIZE_TEXT_MD`) live in `theme.rs` only: the sealed token store has no add path yet, so `scripts/check_spacing_relationships.py`, `scripts/non_colour_token_parity.py` (`RUST_ONLY`) and the invariant tests in `theme.rs` guard them, and `design_os_self_evolve.py` G01 compares every token that does exist with its constant.

## P2 — Color Restraint, Neutral Dark Hierarchy & Strict Purple Ban
`design:os studio` uses a neutral dark surface hierarchy: `BG` `#0A0A0A` (chrome base, panels, menus, and text fields) < `BG_SECONDARY` `#171717` (cards, hover on `BG`) < `BG_TERTIARY` `#262626` (chips, tracks, selected) < `BG_PRESSED` `#404040` (pressed). Text uses `TEXT` `#FAFAFA`, `TEXT_SECONDARY` `#D4D4D4` and `TEXT_TERTIARY` `#A3A3A3`; icons use `ICON` `#FFFFFF` to `ICON_QUATERNARY` `#737373`. Hairlines are `BORDER` `#262626`; the resting boundary of text fields, selects, secondary buttons, checkboxes and radios, and active outlines, are `BORDER_STRONG` `#404040`. Status colours: error `#EF4444` (`DANGER`, icons) and `#F87171` (`TEXT_ERROR`, text), warning `#EAB308`, success `#22C55E`. Four Studio accents stay:
- `#E36323` Studio Orange (`BG_BRAND`, hover `#F97316`, pressed `#EA580C`, the one primary action per view, live state); focus ring `FOCUS_RING` `#F97316`, except a field in its error state and destructive buttons, which take `FOCUS_RING_ERROR` `#EF4444`
- `#0D99FF` Selection Blue (`info`, canvas bounding boxes, selection handles, snap guides)
- `#06B6D4` Cyan (`component`, reusable components, instances, and component sets)
- `#14AE5C` Dev Mode Emerald (Dev ready badges and Dev overlays)

**Strict Purple Ban:** violet is banned across all studio chrome, component badges, focus rings and syntax colours: generic AI/Figma violet (`#9747FF`, `#8755E9`, `#A259FF`, `#C4A1FF`, `#A855F7`) and the purples `#7F56D9`, `#9E77ED`, `#B692F6`, `#6941C6`. Where another design system would use its purple brand, Studio uses orange.

**Selection is a neutral fill.** The selected tool, the active tab and the selected row show a neutral fill (`BG_SELECTED`, `#262626`). Accent colour is spent on the one primary action and on live state; canvas selection stays Selection Blue.

**Contrast:** every chrome text×surface pair passes WCAG AA 4.5:1; on `BG_PRESSED` only `TEXT` and `TEXT_SECONDARY` qualify. The one named exception is placeholder text: `TEXT_PLACEHOLDER` `#737373` (3.78:1 on `BG_SECONDARY`, 4.18:1 on `BG`; owner ruling R8.1, 2026-10-25), used only in text fields: on their `BG` fill (4.18:1) or in a field inside a `BG_SECONDARY` card (3.78:1). The Inspector's "Mixed" value is not a placeholder: it uses `TEXT_TERTIARY` (controller ruling C-R), so the exception stays placeholder-only. Non-text graphics (icons, scroll thumbs, focus rings, control boundaries) pass 3:1 (WCAG 1.4.11). The second named exception, to that non-text rule, is `BORDER_STRONG` `#404040`, 1 px (1.91:1 on `BG`, 1.73:1 on `BG_SECONDARY`) as the resting boundary of exactly these controls: unchecked checkboxes and radios (owner ruling R10.7, 2026-10-25), text fields, selects and secondary buttons (controller ruling C-F, from R2 and R10.7; the owner may reverse it), and only on `BG` or `BG_SECONDARY`. These two exceptions are the only ones; contrast check C-1 holds each at a floor (rows E-1.a/b, E-2.a/b) and never counts them as passes. Labels on orange are the dark `TEXT_ON_BRAND` `#0A0A0A` (5.73:1 on `#E36323`). White text sits only on `BG_ERROR_SOLID` `#DC2626` and its hover `#B91C1C` (6.47:1); tooltips use `TEXT` on `BG_PRIMARY_SOLID` `#262626`. Status badges take a tinted fill, a border and a text constant per status (a success, warning and error set). The pairs and their minimums are listed once, as contrast check C-1, and a unit test in `theme.rs` asserts them.

## P3 — Every gap names a relationship; floors stay floors (spacing scale, `24×24px` pointer floor, same-slot same-size `D12`, `D116`)
**Statement:** Space separates. Every gap names the relationship between the two elements it separates, and each relationship owns one rung of the ladder. An inset (padding inside an element's own frame) comes from the inset family, not the ladder. Control heights and the pointer floor are a separate family that the ladder never re-values. Every ladder and inset value is a step of the spacing scale: 2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48, 64. Control heights are component sizes, not spacing steps: button and input 32, 36, 40, 44, 48, badge 22, 24, 28, plus `TARGET_MIN` 24. Tabs keep today's heights by owner ruling R10.1 (file tabs 26 in the 32 bar, session tabs and the Inspector segmented strip 24). The 4px grid of P3 v1 becomes this spacing scale (R5), which allows the 2px and 6px steps. A drag divider (left-panel resize edge, Pages/Layers separator, ruler strips) keeps its visible 1px line and takes a 24px hit band centred on it (controller ruling C-R), so no pointer target falls under `TARGET_MIN` and no exception is added.

**How:** Ladder rungs are constants in `theme.rs`:

| Rung | Constant | px | Relationship it names |
|---|---|---|---|
| 1 | `SPACE_FACT` | 4 | Parts of one fact: icon and label, prefix and value, a label and the field it names |
| 2 | `SPACE_ROW` | 4 | Stacked rows in one group (filled rows separate themselves) |
| 3 | `SPACE_INLINE` | 8 | Peers side by side: field columns, buttons in a bar, chips, tabs |
| 4 | `SPACE_GROUP` | 12 | Group to group inside a section |
| 5 | `SPACE_BLOCK` | 16 | Block to block in one region: cards in a grid, Home tiles |
| 6 | `SPACE_SECTION` | 24 | Section to section in a panel |
| 7 | `SPACE_REGION` | 48 | Region to region: Home header to grid, Home section to section |

- Insets: `INSET_SEGMENT` 2, `INSET_CONTROL` 8, `INSET_FRAME` 8, `INSET_BUTTON` 12, `INSET_PANEL` 16, `INSET_POPOVER` 16, `INSET_MODAL` 24.
- Controls (button and input sizes): `CONTROL_XS` 32 for editor buttons, rows and fields, `CONTROL_SM` 36 and `CONTROL_MD` 40 for Home and dialogs; badges `BADGE_SM` 22, `BADGE_MD` 24, `BADGE_LG` 28. `TARGET_MIN` 24 is the `WCAG 2.5.8 AA` hit-box floor. `CONTROL_DENSE` 24 and `CONTROL_STANDARD` 28 are legacy names that retire site by site, except `CONTROL_DENSE` as the base of `RIGHT_TABS_H` (R10.1).
- One payer per boundary: the element that starts a new relationship pays the gap; a container's padding and a child's trailing space never add up, and the last child pays nothing. In egui the payer is whoever calls `add_space`, offsets a `Rect` or sets `item_spacing`.

**Yields when:** It yields to the floors (`TARGET_MIN`, the control family) and to measured canvas geometry such as ruler strips and selection handles.

**Test:** `python3 scripts/check_spacing_relationships.py` fails when any file's literal offsets or insets outside `theme.rs` rise above `scripts/spacing_baseline.json`; a fixture proves it can fail. Unit tests in `theme.rs` assert that every ladder and inset constant is a step of the spacing scale, that every control height is a button or input size {32, 36, 40, 44, 48}, a badge size {22, 24, 28} or `TARGET_MIN`, that the ladder never inverts, and that `RIGHT_TABS_H` stays 24 (R10.1).

## P4 — Surface discipline: hairlines, no shadows and concentric radii (`D92`), dedicated `36px` Inspector Footer
**Statement:** Adjacent dark surfaces are separated by a fill step and a crisp `1px` `BORDER` `#262626` hairline, never by a drop shadow: every shadow colour is alpha 0. A rounded shape inside a rounded frame is concentric: inner radius = outer radius − inset, with no optical offset. There are five radius roles on the radius scale: `RADIUS_FRAME` 16 for menus, dropdowns and popovers (R8.5; items inside at radius 8 and inset 8) and for modals, `RADIUS_XL` 12 for cards, the command bar, toasts and nested bars and panels that hold controls at inset 4, `RADIUS_CONTROL` 8 for buttons, fields, tabs, menu items and rows at every size, `RADIUS_SEGMENT` 6 for the tile inside a segmented control and `RADIUS_MARK` 2 for tiny marks. A pill's radius is its height ÷ 2 (`radius_full`). Status and zoom telemetry live in a dedicated `36px` right-inspector footer bar.

**How:** `RADIUS_FRAME` 16 − `INSET_FRAME` 8 = `RADIUS_CONTROL` 8, `RADIUS_XL` 12 − 4 = `RADIUS_CONTROL` 8, and `RADIUS_CONTROL` 8 − `INSET_SEGMENT` 2 = `RADIUS_SEGMENT` 6. egui `CornerRadius` is a `u8`; derive inner radii with `theme::concentric(outer, inset)`, never as literals. When a frame strokes `StrokeKind::Inside`, measure the inset from the stroke's inner edge. Pills are used for pill badges, count pills (`Badge/StatusCount`), filter chips, toggles and avatars; buttons, including Home and dialog buttons, use `RADIUS_CONTROL`, and colour and modern badges use `RADIUS_SEGMENT` 6 (`RADIUS_CONTROL` 8 at the large size).

**Yields when:** It yields to canvas overlays, which draw at document scale, and to document content.

**Test:** Unit tests in `theme.rs` assert `RADIUS_FRAME - INSET_FRAME == RADIUS_CONTROL`, `RADIUS_CONTROL - INSET_SEGMENT == RADIUS_SEGMENT`, that every radius role is a step of the radius scale, and the `concentric` and `radius_full` helpers. The ratchet counts literal radii and detected non-concentric nested pairs per file and may only fall.

## P5 — Type roles are steps; numbers in mono; counts in pills (`D6`, `D85`)
**Statement:** Text is set by role, not by size, on one type scale in `Inter`. Editor panels use the small end: 12/18 for rows, labels, captions, counts, chips and values (`FONT_SIZE_CAPTION`, `FONT_SIZE_BODY`) and 14/20 for panel titles (`FONT_SIZE_LARGE`). Home and dialogs use 14/20 for body text; modal titles alone take 16/24 (`FONT_SIZE_TEXT_MD`). Button labels at every size, the editor's 32px buttons included, are 14 Semibold (owner ruling R8.6); rows, labels and tabs stay 12. 20/30 sets page titles and the Home hero (`FONT_SIZE_TITLE`). A 9px size exists for canvas overlays only (rulers, canvas pills). Weights are Regular 400 (body), Medium 500 (menus, labels, captions) and Semibold 600 (titles, button labels, strong text); Bold 700 belongs to Display styles, which chrome does not use. All numeric coordinates (`X`, `Y`, `W`, `H`, rotation, opacity), hex colour codes, token paths and FPS/DirtyLog counters render in `Roboto Mono` (`theme::font_mono()`, `D6`), bundled with the app for tabular scannability. The mono rule is scoped to numeric field values, hex codes, token paths, code views and the terminal, pending owner question OQ-1 (the alternative is Inter with tabular figures); the ontology gate pins the monospace sites that exist today (`scripts/check_ds_ontology_parity.py`, P7), so a new one fails it. Item counts render in separate pill badges (`Badge/StatusCount`, `D85`), never concatenated into label strings like `"Layers (12)"` and never put in parentheses (`design/ux-writing.md` rule 3, gate `Z16`).

**How:** `theme.rs` holds the role sizes `FONT_SIZE_CAPTION`, `FONT_SIZE_BODY`, `FONT_SIZE_LARGE`, `FONT_SIZE_TITLE` and `FONT_SIZE_CANVAS`, the scale steps `FONT_SIZE_TEXT_XS` … `FONT_SIZE_TEXT_XL` with their line heights `LINE_TEXT_XS` … `LINE_TEXT_XL`, `font_caption*()` and `font_body*()` for chrome and `font_canvas*()` for overlays.

**Yields when:** It yields to canvas overlays, where 9px is the declared exception, and to document text, which follows the document.

**Test:** A unit test in `theme.rs` asserts that each role size and line height is one of the scale's text styles (12/18, 14/20, 16/24, 20/30), that the weights are 400, 500 and 600, and that the canvas size is the only one below the chrome floor. The ratchet counts font-size literals outside `theme.rs`.

## P6 — Zero Fake UI (`scripts/zero_fake_ui_gate.py` `Z01..Z14`) & Home Data Truth
Every visible button, tab, menu item, filter pill, and search input must execute real stateful logic verified by `scripts/zero_fake_ui_gate.py` (`Z01..Z14`). In the Home workspace (`crates/designos-app/src/home/mod.rs`), each navigation section (`Recents`, `Drafts`, `Starred`, `Templates`, `Design Systems`, `Trash`, `Archive`) queries its own truthful dataset and renders a dedicated empty state when empty — never silently falling back to `Recents`.

## P7 — Terminal-First Per-File Agent Isolation
Every open document owns an isolated PTY agent session inside the `400px` resizable `Panel/AgentTerminal` (`crates/designos-app/src/terminal.rs`), rendered with the Orca Ghostty dark palette (terminal content; the chrome around it follows P2), procedural pixel-aligned box-drawing glyphs (`U+2500..U+257F`), bracketed paste (`\x1b[200~..\x1b[201~`), 3-tier live status indicator dots (`Running`, `Idle`, `Exited`), and automatic canvas single-key shortcut suppression whenever the terminal or any text input holds focus.

## P8 — Figma C++/WASM 60fps Performance Floor
The viewport engine preserves a locked 60fps (`16.6ms` frame budget) on 100k+ node scenes via flat Structure-of-Arrays `SceneIndex` spatial indexing, incremental `DirtyLog` O(k) GPU instance buffer synchronization, lazy per-page component instance expansion, and off-main-thread background image decoding.

## P9 — Closer means related
**Statement:** An element sits nearer to what it belongs to than to what it merely follows: a label is nearer its field than the field above; a heading is nearer its content than the previous section; a group's distance to its neighbours is at least twice its own inner gap.

**How:** Inspector sublabels sit `SPACE_GROUP` 12 above and `SPACE_FACT` 4 below, not midway. Section titles sit closer to their first row than to the hairline above. Home sections get `SPACE_REGION` between them and `SPACE_BLOCK` between cards. List rows with gap 0 and inset hover pills count as one group: the pills, not the gaps, separate them.

**Yields when:** It yields to the floors and to the control family. If a label cannot move without breaking a 24px target, the target wins.

**Test:** Unit tests in `theme.rs` assert `SPACE_GROUP` ≥ 2 × `SPACE_FACT` (a label sits at least twice as far from the field above as from the field it names), `SPACE_BLOCK` ≥ 2 × `SPACE_INLINE` and `SPACE_REGION` ≥ 2 × `SPACE_BLOCK`.

## P10 — A child inherits from its parent
**Statement:** Spacing, radius and style flow downward. A child's inset is no larger than its parent's, a child's gap is no larger than its parent's, and a child's radius is its parent's radius minus the inset (P4). One panel has one content inset: every title, search field, list and card in it starts on the same edge.

**How:** The inset family is ordered `INSET_MODAL` 24 ≥ `INSET_POPOVER` 16 = `INSET_PANEL` 16 ≥ `INSET_BUTTON` 12 ≥ `INSET_FRAME` 8 = `INSET_CONTROL` 8 ≥ `INSET_SEGMENT` 2. A surface takes its inset from its kind, never from its file: Assets, Variables and Find use `INSET_PANEL` for content and `INSET_FRAME` for hover pills.

**Yields when:** It yields to canvas content and to user documents.

**Test:** A unit test in `theme.rs` asserts the inset order. The ratchet's per-file baseline stops a surface from growing a new local padding value.

## P11 — Name the action, say less (`design/ux-writing.md`)
**Statement:** UI copy follows the Studio UX writing standard, adopted as-is from VSF PCP by owner ruling 2026-10-21. It helps the user understand the content, choose an action and know its result.

**Always:**
- Action labels (buttons, menu items) are verb + object, at most three words (rule 1).
- Microcopy is about eight words, one idea per sentence, and keeps any consequence or recovery step (rule 5).
- Errors name the blocked operation and a recovery; success is announced only after it happens (rule 6).
- Accessible names keep the full meaning the compact label drops (rule 14).

**Never:**
- Explanatory subcopy that restates a nearby title, label or the UI itself (rule 2).
- A count in parentheses or inside the action phrase: use the count pill (rule 3, P5).
- A coined status word where the domain already has one (rule 7).

**Yields when:** It yields to quoted user data, canonical product and file names, and to the safety text of a destructive action.

**Test:** `python3 scripts/zero_fake_ui_gate.py` invariant `Z16` rejects a new action label over three words or a count in it (parentheses, digits, a formatted `.len()`) at the registered label helpers, against `scripts/ux_writing_baseline.json`, which only shrinks (the writer refuses to add or raise an entry). Everything else in the standard is checked in review.
