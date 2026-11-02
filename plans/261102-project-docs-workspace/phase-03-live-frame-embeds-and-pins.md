# Phase 03 — Live frame embeds + pinned Screens/States

UI: part 1 "Embed card" + [part 2](ui-spec-part-2-editor-dialogs-and-states.md) §5, §6. Mockups: ui-mockups/png/m03 (card), m07, m08, m09.

Decisions: P7, P11. Size L · 3–4 d. Depends on 02.

## Goal
Embeds and pinned frames show the real frame, stay current, and jump to the canvas on click.

## Requirements
1. Render with `designos_render::render_node` (`crates/designos-render/src/lib.rs:181`) on a worker thread; load the design file read-only (or reuse an open tab's document); cache PNG textures by `(file_id, node_id, content revision)`; at most 2 concurrent renders; only visible cards render.
2. Card per UI spec "Embed card": header "<frame> · <file>", open icon; body fit-width, max 420 px; "Rendering…" while pending.
3. Click (card or tree row) opens the design file's tab (or focuses it) and zooms to the node: add a zoom-to-node helper next to `zoom_to_selection` (`crates/designos-app/src/ops.rs:1986`) that frames a node without changing the selection. Viewport-only: no `App::changed()`, no `TileView::rev` bump.
4. Broken reference (file or node gone) → "Frame not found — Relink" card; Relink opens a picker of frames in project files; never blank.
5. Pins: tree action "Pin frame…" and "Suggest from canvas" (lists top-level frames of project files not yet pinned, multi-select). States are pinned under a screen.
6. The design file changes on disk or in an open tab → affected cards re-render.

## Files
`crates/designos-app/src/project_tab/embed.rs`, `render_worker.rs`, `pin_picker.rs`; `designos-docs` manifest pin API.

## Acceptance
1. Test: changing a pinned frame's fill invalidates and re-renders exactly that card.
2. Test: click → target tab active, view contains the node, document rev unchanged, undo stack unchanged.
3. Test: deleted node → relink card; relink rewrites the reference.
4. Big-file check: VSF-PCP-size project file, scrolling a page with 10 embeds keeps frames < 16 ms (log) and idle CPU 0 %.
5. Captures at 1440 next to `ds-document-live-embeds.jpg`.

## Non-goals
Interactive prototype playback in embeds (rejected R2).
