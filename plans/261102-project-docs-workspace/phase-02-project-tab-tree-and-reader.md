# Phase 02 — Project tab: doc tree, reading page, "On this page"

UI: [part 1](ui-spec-ported-from-design-studio.md) in full + [part 2](ui-spec-part-2-editor-dialogs-and-states.md) §1, §2, §11, §12, §13, §14, §15. Mockups: ui-mockups/png/m01, m02, m03 (reading parts), m13, m13b.

Decisions: P2, P4, P6 + [UI spec](ui-spec-ported-from-design-studio.md) (copy design-studio 1:1). Size L · 4–5 d. Depends on 01.

## Goal
Open a library project as its own tab and read its docs exactly as design-studio shows a prototype's artifacts.

## Requirements
1. New `ProjectTabKind::Project` (`crates/designos-app/src/tabs.rs:28`) opened from Home: project row/tile "Open project" and context menu; tab title "<project> · Project". Per-tab state (selected page, scroll, tree expansion) isolated like other tabs.
2. Layout, sizes, colours per the UI spec: rail 48 · tree 240 (`LEFT_PANEL_WIDTH`) · reading column 690 · contents 192 (hidden < 1200 px); tree rows 32 (`LAYER_ROW_HEIGHT`).
3. Tree: five groups, pages from phase 01 manifest, pinned frames listed under Screens/States (labels only in this phase; render comes in 03). Search filter, collapse all, drag reorder within a group, F2 rename, Move up/down, Delete page (moves to Studio's trash) — each writes the manifest / file through `designos-docs`.
4. Reader renders all v1 blocks read-only (embeds show the "Rendering…" placeholder card until 03; Mermaid shows its source in a code card until 05).
5. "On this page" lists H2/H3 with scroll-spy; clicking scrolls smoothly.
6. New project with no docs: Overview page titled with the project name (owner ruling 2026-11-04, matches part 2 §2 and mockup m02) created from a template on first open (title + one-line description), written as a real file.
7. External file edits refresh the open page without losing scroll.

## Files
`crates/designos-app/src/project_tab/` (new: `mod.rs`, `tree.rs`, `reader.rs`, `contents.rs`, `layout.rs`), `tabs.rs`, `home/main_view.rs`, `home/mod.rs` (open action), `shell.rs` (route the tab). Load `es-designer` before UI code.

## Acceptance
1. Snapshot/interaction tests: open project → tab kind Project; tree groups in order; rename writes file + manifest.
2. Reader layout test: tree x48 w240, column x399 w690, contents x1137 w192 at 1440 (±1 px).
3. Idle CPU 0 % over 20 s with the tab open (method in CLAUDE.md §4).
4. Captures at 1440: tree + Overview page, a Spec page with a table, next to `ds-document-page.jpg` and `ds-project-context-doc.jpg`.

## Non-goals
Editing (04), live render (03), Mermaid render (05), MCP (06), spec v2 (07).
