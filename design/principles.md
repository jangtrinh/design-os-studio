# design:os studio — Design Principles (P1–P10)

These 10 binding laws govern every surface, widget, token, and interaction across `design:os studio`. Every change to `crates/designos-app` or `design/` is audited against `P1..P10` and enforced by `ui ds a11y`, `ui ds soul check`, and `scripts/zero_fake_ui_gate.py`.

## P1 — Single Token Authority (`crates/designos-app/src/theme.rs` ↔ `design/design.tokens.json`)
All colors, typography scales, spatial increments, corner radii, and motion durations originate in `crates/designos-app/src/theme.rs` and are mirrored 1:1 into `design/design.tokens.json` (73 DTCG tokens across 5 categories, all 16 canonical roles mapped). Inline `Color32::from_rgb(...)` literals in panels or views outside `theme.rs` are forbidden unless explicitly allowlisted for semantic canvas overlays or GPU shader debug modes.

Spacing, inset, control-height, radius-role and type-role constants (P3, P4, P5, P9, P10) are relationship aliases that live in `theme.rs` only: the sealed token store has no add path yet, so `scripts/check_spacing_relationships.py` and the invariant tests in `theme.rs` guard them, and `design_os_self_evolve.py` G01 compares every token that does exist with its constant. Their values come from the primitives already in `design/design.tokens.json`.

## P2 — Color Restraint, Zinc Dark Hierarchy & Strict Purple Ban
`design:os studio` uses a disciplined 4-step Zinc dark surface hierarchy (`#121215` Canvas < `#18181B` Chrome/Panel < `#1E1E22` Card/Input < `#232329` Hover < `#2E2E36` Active) with exactly four semantic accents:
- `#E36323` Brand Orange (`primary`, focus rings, the one primary CTA per view, live state)
- `#0D99FF` Selection Blue (`info`, canvas bounding boxes, selection handles, snap guides)
- `#06B6D4` Cyan (`component`, reusable components, instances, and component sets)
- `#14AE5C` Dev Mode Emerald (`success`, code inspection, ready badges, live agent status)

**Strict Purple Ban:** Generic AI/Figma violet (`#9747FF`, `#8755E9`, `#A259FF`) is banned across all studio chrome and component badges.

**Selection is a neutral fill.** The selected tool, the active tab and the selected row show a neutral fill (`bg-active`, `#2E2E36`). Accent colour is spent on the one primary action and on live state; canvas selection stays Selection Blue.

## P3 — Every gap names a relationship; floors stay floors (4px grid, `24×24px` pointer floor, same-slot same-size `D12`, `D116`)
**Statement:** Space separates. Every gap names the relationship between the two elements it separates, and each relationship owns one rung of the ladder. An inset (padding inside an element's own frame) comes from the inset family, not the ladder. Control heights and the pointer floor are a separate family that the ladder never re-values. All ladder and inset values sit on the 4px grid; there are no 2px or 6px half-steps.

**How:** Ladder rungs are constants in `theme.rs`, each equal to a primitive spacing token:

| Rung | Constant | px | Relationship it names |
|---|---|---|---|
| 1 | `SPACE_FACT` | 4 | Parts of one fact: icon and label, prefix and value, a label and the field it names |
| 2 | `SPACE_ROW` | 4 | Stacked rows in one group (filled 24px rows separate themselves) |
| 3 | `SPACE_INLINE` | 8 | Peers side by side: field columns, buttons in a bar, chips, tabs |
| 4 | `SPACE_GROUP` | 12 | Group to group inside a section |
| 5 | `SPACE_BLOCK` | 16 | Block to block in one region: cards in a grid, Home tiles |
| 6 | `SPACE_SECTION` | 24 | Section to section in a panel |
| 7 | `SPACE_REGION` | 48 | Region to region: Home header to grid, Home section to section |

- Insets: `INSET_CONTROL` 8, `INSET_BUTTON` 12, `INSET_FRAME` 8, `INSET_PANEL` 16, `INSET_POPOVER` 16, `INSET_MODAL` 24, `INSET_SEGMENT` 2.
- Controls (R10): `CONTROL_DENSE` 24, `CONTROL_STANDARD` 28, `CONTROL_TOOL` 32, `CONTROL_ROW` 40. Segmented tab strips are 24px, not 28. `TARGET_MIN` 24 is the `WCAG 2.5.8 AA` hit-box floor.
- One payer per boundary: the element that starts a new relationship pays the gap; a container's padding and a child's trailing space never add up, and the last child pays nothing. In egui the payer is whoever calls `add_space`, offsets a `Rect` or sets `item_spacing`.

**Yields when:** It yields to the floors (`TARGET_MIN`, the control family) and to measured canvas geometry such as ruler strips and selection handles.

**Test:** `python3 scripts/check_spacing_relationships.py` fails when any file's literal offsets or insets outside `theme.rs` rise above `scripts/spacing_baseline.json`; a fixture proves it can fail. Unit tests in `theme.rs` assert that every ladder, inset and control constant is on the 4px grid, that the ladder never inverts, and that the control heights are the R10 family.

## P4 — Surface discipline: hairlines and concentric radii (`D92`), dedicated `36px` Inspector Footer
**Statement:** Adjacent dark surfaces are separated by crisp `1px` `#27272A` (`colors.border`) hairlines rather than muddy drop shadows. A rounded shape inside a rounded frame is concentric: inner radius = outer radius − inset, with no optical offset. There are four radius roles: `RADIUS_FRAME` 14 for floating chrome (menus, palettes, popovers, modals, toasts, toolbars), `RADIUS_CONTROL` 6 for controls, `RADIUS_SEGMENT` 4 for the tile inside a segmented control and `RADIUS_MARK` 2 for tiny marks. A pill's radius is its height ÷ 2 (`radius_full`). Status and zoom telemetry live in a dedicated `36px` right-inspector footer bar.

**How:** `RADIUS_FRAME` 14 − `INSET_FRAME` 8 = `RADIUS_CONTROL` 6, and `RADIUS_CONTROL` 6 − `INSET_SEGMENT` 2 = `RADIUS_SEGMENT` 4. egui `CornerRadius` is a `u8`; derive inner radii with `theme::concentric(outer, inset)`, never as literals. When a frame strokes `StrokeKind::Inside`, measure the inset from the stroke's inner edge. Pills are used for chips, badges, status marks, filter chips and dialog and Home buttons; fields, tool buttons and inspector rows stay rectangular at `RADIUS_CONTROL`.

**Yields when:** It yields to canvas overlays, which draw at document scale, and to document content.

**Test:** Unit tests in `theme.rs` assert `RADIUS_FRAME - INSET_FRAME == RADIUS_CONTROL`, `RADIUS_CONTROL - INSET_SEGMENT == RADIUS_SEGMENT`, and the `concentric` and `radius_full` helpers. The ratchet counts literal radii and detected non-concentric nested pairs per file and may only fall.

## P5 — Type roles are steps; numbers in mono; counts in pills (`D6`, `D85`)
**Statement:** Text is set by role, not by size. UI chrome uses `Inter` in four roles: caption 10px (sublabels, rail labels, counts, chips), body 11px (body, controls, menu rows, values), heading 13px (panel and modal titles) and title 20px (page titles, the home hero). A 9px size exists for canvas overlays only (rulers, canvas pills). Weights are 450 (body), 500 (menus and captions) and 550 (strong text); 600 is retired. Multi-line text uses line height 16 (body) and 14 (caption). All numeric coordinates (`X`, `Y`, `W`, `H`, rotation, opacity), hex colour codes, token paths and FPS/DirtyLog counters render in `Roboto Mono` (`theme::font_mono()`, `D6`), bundled with the app for tabular scannability. Item counts render in separate pill badges (`Badge/StatusCount`, `D85`), never concatenated into label strings like `"Layers (12)"`.

**How:** `theme.rs` holds `FONT_SIZE_CAPTION`, `FONT_SIZE_BODY`, `FONT_SIZE_LARGE`, `FONT_SIZE_TITLE` and `FONT_SIZE_CANVAS`, with `font_caption*()` for chrome and `font_canvas*()` for overlays.

**Yields when:** It yields to canvas overlays, where 9px is the declared exception, and to document text, which follows the document.

**Test:** A unit test in `theme.rs` asserts that the role sizes keep their order and that the canvas size is the only one below the chrome floor. The ratchet counts font-size literals outside `theme.rs`.

## P6 — Zero Fake UI (`scripts/zero_fake_ui_gate.py` `Z01..Z14`) & Home Data Truth
Every visible button, tab, menu item, filter pill, and search input must execute real stateful logic verified by `scripts/zero_fake_ui_gate.py` (`Z01..Z14`). In the Home workspace (`crates/designos-app/src/home/mod.rs`), each navigation section (`Recents`, `Drafts`, `Starred`, `Templates`, `Design Systems`, `Trash`, `Archive`) queries its own truthful dataset and renders a dedicated empty state when empty — never silently falling back to `Recents`.

## P7 — Terminal-First Per-File Agent Isolation
Every open document owns an isolated PTY agent session inside the `400px` resizable `Panel/AgentTerminal` (`crates/designos-app/src/terminal.rs`), rendered with the Orca Ghostty dark palette, procedural pixel-aligned box-drawing glyphs (`U+2500..U+257F`), bracketed paste (`\x1b[200~..\x1b[201~`), 3-tier live status indicator dots (`Running`, `Idle`, `Exited`), and automatic canvas single-key shortcut suppression whenever the terminal or any text input holds focus.

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
