# Phase 07 — Spec v2 mapping, Overview KPI, retire the old Spec Reader

UI: [part 2](ui-spec-part-2-editor-dialogs-and-states.md) §10. Mockup: ui-mockups/png/m12.

Decisions: P10, P13, D4 kept. Size M · 3 d. Depends on 02.

## Requirements
1. A project can link spec folders (manifest `linked_specs`, action "Link spec folder" in the tree header menu). A linked PRODUCT:OS spec v2 package is read in place and sorted: `intake/` → Overview, `contract/` + `prd/` + `us/` → Spec, `ux/` → Screens/States (as pages), `decisions/` + `review/` → Discovery. Files are never rewritten by this mapping; editing a linked page writes that file in place.
2. Overview top: the 4-metric KPI strip (REQs · US, ACs · SCNs, Screens · Contract rows, Gates approved) from one loaded spec model (D3 metrics; placement per P10). Hidden when no spec is linked.
3. "Copy cho AI" (D4 format: `File: <path>:<start>-<end>`, Mục, Trang reader, Project, Yêu cầu của tôi) on any block.
4. Remove the old reader: `specs_reader_open` branch in `crates/designos-app/src/shell.rs:1863`, `RailItem::Specs` and its hidden rail entry, `ProjectTabKind::Spec` (migrate open Spec tabs to the Project tab), the fake "LIVE CANVAS EMBED" (`specs.rs:3288-3316`), and the rest of `specs.rs` UI. Keep and move any parsing still used (e.g. `parse_markdown_blocks`, table rows) into `designos-docs` or delete it if phase 01 replaces it.
5. Update decision log: D3 placement and D20 role views superseded by P10 (2026-11-02).

## Acceptance
1. Test: the repo's `plans/specs/designos-studio-v2` package maps into the five groups; file hashes unchanged after opening.
2. KPI values equal the old reader's computation on the same package (test with fixed fixture).
3. Copy cho AI output matches D4 format (test).
4. `git grep specs_reader_open` empty; Zero Fake UI gate passes; capture of Overview with KPI at 1440.

## Non-goals
Converting spec v2 to a new format; role views.
