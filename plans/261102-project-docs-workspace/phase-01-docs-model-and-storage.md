# Phase 01 — Docs model + storage crate

UI: none in this phase; it owns the save path behind UI spec part 2 §8 (auto-save 500 ms, conflict detection).

Decisions: P3, P4, P11, P13 ([brainstorm.md](../261102-project-docs-workspace/brainstorm.md)). Size M · 3–4 d.

## Goal
A new crate `crates/designos-docs` that both the app and designos-mcp use to read, write and watch a project's docs. No UI.

## Requirements
1. Location: `<library>/projects/<project-id>/docs/` (project dir from `crates/designos-library/src/store.rs:475`). Pages are `.md` with optional front matter (`title`, `description`, `group`).
2. Manifest `docs/docs-manifest.json`: schema version, groups in fixed order `overview | spec | screens | states | discovery`, page order per group, pins (`{ id, file_id, node_id, name, group: screens|states, parent_screen? }`), linked spec folders. Missing required fields are errors that name the fix; no defaults by omission.
3. Markdown model for v1 blocks: heading 1–3, paragraph (bold, italic, code, link), bullet/numbered list, checklist, callout (`> [!note]` form), table, frame embed `![name](designos:<file-id>#<node-id>)`, fenced `mermaid`, fenced code, and a raw block for anything else. Parse → blocks → serialise is byte-identical for files written by Studio, and preserves unknown Markdown verbatim.
4. Writes go through tmp file + rename; a write refuses when the file's content hash changed since it was read (returns a conflict error).
5. A watcher reports changed pages (path + new hash) off the UI thread; reuse the `DiskWatcher` pattern in `crates/designos-app/src/home/session.rs:38-44`.

## Files
Create `crates/designos-docs/` (`lib.rs`, `model.rs`, `markdown.rs`, `manifest.rs`, `store.rs`, `watch.rs`, tests). Modify workspace `Cargo.toml`. Pick one Markdown crate (comrak or pulldown-cmark + writer) by a 1-hour check of round-trip fidelity on the fixtures; record the choice in the PR.

## Acceptance (test-first)
1. Round-trip fixtures for every v1 block type are byte-identical; a fixture with unknown syntax keeps it.
2. Manifest validation: missing `groups` / bad pin → error naming field and fix.
3. Write conflict: modify file between read and write → `Conflict`, file unchanged.
4. Watcher reports an external edit within 1 s; no polling on the caller's thread.
5. `cargo test -p designos-docs` green; capture attached.

## Non-goals
UI, MCP, spec v2 mapping (phase 07), embeds rendering (phase 03).
