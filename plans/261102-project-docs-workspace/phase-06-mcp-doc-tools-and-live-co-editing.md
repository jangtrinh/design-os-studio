# Phase 06 — MCP doc tools + live co-editing

UI: [part 2](ui-spec-part-2-editor-dialogs-and-states.md) §9. Mockups: ui-mockups/png/m11, m11b.

Decisions: P9, P12. Size L · 4–5 d. Read tools depend on 01; write tools and live editing depend on 04.

## Requirements
1. Read tools (tier read): `designos_docs_list {project?}` (groups, pages, pins), `designos_docs_read {page, range?}` (blocks with ids + Markdown). Doc text is returned inside the untrusted-data fence (#1289).
2. Write tools: `designos_docs_write {page, ops:[insert_block|replace_block|delete_block|move_block], after?}`, `designos_docs_create_page {group, title}`, `designos_docs_pin {file_id, node_id, group, parent_screen?}`. Atomic per call; `dry_run` like `apply_batch`.
3. Live: with the Project tab open, each write call appears as one undo step, shows the agent cursor at the block and a write flash (reuse `crates/designos-app/src/agent_presence.rs` colours/timings), and is held while the user is typing in that page (same rule as canvas `live_patch_apply.rs`). Without Studio open, writes go to disk through `designos-docs` with conflict checks.
4. Tool docs regenerate via `gen-tool-docs` and the snapshot test (`crates/designos-mcp/tests/tools_snapshot.rs`).

## Files
`crates/designos-mcp/src/tools/docs.rs` (new), `engine.rs` registration, app side `project_tab/live_docs.rs`; reuse the doc-patch log transport.

## Acceptance
1. MCP tests for each tool incl. conflict, unknown page, untrusted fence.
2. App test: agent write while user types → applied after typing stops; one Cmd+Z removes it.
3. Snapshot updated; tool count in generated docs matches.
4. Capture: agent writing a section into an open page (cursor + flash).

## Non-goals
Agents editing frames from docs (use canvas tools).
