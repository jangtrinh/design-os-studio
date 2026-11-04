# Dev report: Project tab (issue #1524, epic #1522)

Branch `feat/1524-project-tab` (from `origin/feat/1523-docs-model-storage`; main not merged). Read-only phase: tree, reading page, On this page. No push, no PR.
The Skill tool was unavailable in this run; the SKILL.md files (es:feature-dev, es:fable-thinking, es:rust) were read directly.

## NOT verified (read first)

- Idle CPU is "0 %" only to within one `ps` tick: 0:00.19 -> 0:00.20 after 20 s -> 0:00.21 after 40 s with the tab open; the Home baseline shows the same single tick (`evidence-1524/idle-cpu.txt`). `ps` resolves 10 ms, so exactly 0.000 s is not proven.
- Design-os self-evolve gate is 5/9 (G06 to G09 fail). The identical 5/9 result on the base commit `origin/feat/1523-docs-model-storage` (run in a throwaway worktree, removed). So the gate is not green and I did not make it green: its failures are agent-definition warning, evolution DEAD-LOOP, ui_gate_specimen, and mockup HTML audit errors, none in files this phase owns.
- `cargo clippy ... -D warnings` fails on 3 findings in files I did not touch and that are unchanged from base: `agent_permissions.rs:1176` and `:1194` (needless_return, macOS-only cfg block) and `shell/tests/inspector_var_binds.rs:1182` (type_complexity). With those two lints allowed, the whole workspace is clean. CI pins 1.93.1 and runs elsewhere; I could not see why CI passes, so I only report it.
- Not exercised: a real file-watcher event from another program on a live window (the Sim test uses real files and the real watcher thread, not a second process); the `Job::Refresh` on window focus; the 800 px overlay tree and under-1200 px contents hiding (layout unit tests only, no capture; responsive captures skipped by owner default).
- Pins, "Link spec folder", double-click open, the block editor placeholder ("Type / for blocks") are not built (phases 3 and 4 or outside this issue).

## What shipped (all under `crates/designos-app/src/project_tab/`, FACT)

- `ProjectTabKind::Project`; `open_project_tab` focuses an existing tab for the project or creates "<name> · Project". Home gets "Open project" in the project context menu (first row), in the project header (leftmost button), and as a hover button on the project card.
- One docs worker thread per tab (`service.rs`) owns the scanner and the `DocsWatcher`. The UI never parses or serialises a page. Results go through a coalescing inbox plus `request_repaint`; the watcher wake is coalesced through an atomic flag. A focus gain sends a stat refresh.
- Tree (`tree.rs`, `tree_model.rs`): five groups in fixed order, count pills (none on an empty group), placeholders with a hover "+", pins non-interactive, states under their screen, search (title and heading text), collapse, keyboard (arrows, Enter, F2), rename field, drag reorder inside one group (cross-group drop refused), context menu (Open, Rename, Move up/down, Copy page link, Edit source, Delete), header menu and New page menu.
- Reader (`reader.rs`, `reader_layout.rs`, `inline.rs`): headings, paragraphs with marks and links, lists, checklists, callouts, tables (120 to 320 px columns, horizontal scroll bar), code, frame embed card, mermaid card, raw blocks (a comment-only block renders nothing), CSS-style margin collapsing, 200 ms ease-out scroll, scroll-spy, "On this page" (`contents.rs`). A page over 8 MiB shows "This page is larger than 8 MB and is not watched for outside changes."
- Ops (`ops.rs`): first open writes `overview.md` + manifest as real files; rename writes the new file and manifest, retargets links, removes the old file only if unchanged; reorder; new page; delete moves the page into the library trash with a `page.json`, rolling back if the manifest write fails.
- `delete_dialog.rs` (Cancel has the focus; Enter never deletes), `menu.rs` (own context menu: first enabled row hot, arrows, Enter, Esc, outside press closes).

## Acceptance

1. Tests (sim, `shell/tests/project_tab.rs`; unit tests in the module): tab kind Project and focus-not-duplicate; fixed group order; rename writes file + manifest + retargets links; Delete via menu and dialog moves the page to the library trash and updates the manifest, Enter cancels; drag reorder persists, cross-group drop changes nothing; new project gets `overview.md` as a real file; oversize note; contents click scrolls; project header/menu entries. 56 passed, 0 failed, 1 ignored (fixture generator).
2. Layout at 1440, from the recorded rects of the real window (test `the_regions_are_where_the_layout_puts_them`) and `layout` unit tests: tree x48 w240, column x399 w690, contents x1137 w192; contents hidden under 1200 px. FACT (tests pass). Window was 1600 wide in the sim; the 1440 numbers come from `layout::compute` and the 1440 captures, where the pixels match the mockups (column title x399, contents x1137).
3. Idle CPU: see NOT verified and `evidence-1524/idle-cpu.txt`.
4. External edit keeps scroll: test `an_outside_edit_refreshes_the_open_page_and_keeps_the_scroll_position` (rewrites the file on disk, waits for the watcher, asserts the 7th heading appears and the scroll offset stays within 1 px). Pass.
5. Gates after the last edit (FACT, real results):
   - `cargo test --workspace --lib --locked`: all ok (745, 327, 164, 143, 56, 37, 30, 13, 8, 5 ... passed, 0 failed).
   - `cargo test -p designos-app --bin designos --locked`: 4303 passed, 2 failed = the two known `vecops` flakes (#1579); rerun alone: both pass (4 passed and 1 passed).
   - `cargo clippy --workspace --all-targets --locked -- -D warnings`: 3 pre-existing errors (above); clean with those two lints allowed.
   - `cargo fmt --all --check`: clean.
   - `python3 scripts/zero_fake_ui_gate.py`: 17/17 PASS.
   - `python3 scripts/design_os_self_evolve.py`: 5/9, same as base (above). The script rewrote `design/` memory files; I reverted them.
6. Visual evidence at 1440x900 from the real release app, `plans/reports/evidence-1524/`: `real-01-first-open-overview`, `real-01b-tree-overview`, `real-02-spec-table`, `real-03-tree-context-menu`, `real-04-delete-dialog`, `real-05-search-ca`, and side-by-side sheets `sheet-01..04` (mockup left, real right). No Figma.

## Visible differences vs the mockups (fixed or justified)

Fixed during this phase after the first capture: tree chevrons sat 5 px above their label (box anchored to the row top); body text sat about 3 px high (egui puts a glyph at the top of its line box, CSS in the middle, now shifted by half the extra leading, so H1/description/H2 match the mockup y positions); checklist boxes then lined up with their text; context menu and dialog faded in (now instant); empty groups showed a "0" pill (mockup has none); the first menu row was not hot.

Justified, still different:
1. Text is 1 px lower than the mockup everywhere, including the existing tab bar: egui centres text on its own font metrics (R0, kit chrome wins). Row fills and rects match to the pixel.
2. Tab strip: the capture shows an extra "Untitled" document tab (the screenshot run's placeholder document); the mockup has "Checkout-web". Harness, not UI.
3. Icons: collapse-all uses a down chevron (mockup: fold icon), Move up/down use chevrons (mockup: arrows), Edit source uses the pencil family (mockup: code brackets), state rows use the diamond instance icon (mockup: stacked squares). The shipped icon set has no matching glyph; adding one means editing `icons.rs`, outside this phase.
4. Table: mockup is about 2 px taller (its outer 1 px border sits outside the cell grid, mine inside); the gap under the table follows. Other spacing matches (header 36, 48 px table to H2).
5. Mockup pages show editor chrome (block handles, selected-block box, "Type / for blocks", block menu): Phase 4. Mockup m03 text differs from the fixture (extra bullets), so absolute y below the callout differs; relative gaps match.
6. Context menu opens at the click point (x169 in the capture vs x191 in the mockup, where the pointer was drawn elsewhere).
7. Search capture shows Spec count 2: "Error handling" has the heading "Declined cards", which matches "ca" under the heading rule. Mockup fixture had no such heading.
8. Inline code radius and H2 weight: Semibold stands in for 700 (documented in the code).

## Deviations from the brief (FACT)

- Files outside the owned list that I had to touch: `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt` (two FILE rows for `project_tab/ops.rs` and `project_tab/service.rs`; a repo gate test fails without them, and the rows say the paths come from the docs folder of a library project, never a session folder) and `crates/designos-app/src/shell/tests.rs` (one `mod project_tab;` line). Both are one-line data/registration changes; the controller may want to review the allowlist wording.
- `main.rs`, `agent_document.rs`, `cli.rs` are not owned: the module is declared from `tabs.rs` (`#[path]`), the Project state lives in `TabsState.project_states` instead of a new tab field, and the screenshot hook is the env var `DESIGNOS_PROJECT_TAB_SHOT` (read at start-up in any run, documented in `shot.rs`) instead of a CLI flag.
- Trash: there is no `TrashKind::Page`. A deleted page goes into the library trash folder as a `page-...` entry with `page.md` + `page.json`, which Home cannot list or restore yet. Needs `designos-library` work (follow-up).
- Overview naming drift: the Overview page is titled with the project name ("Checkout"), following spec section 2 and mockup m02, not "About this project" as the brief's acceptance line says. Description: "What this project is and where to start."
- `Cargo.lock` gained one line for the `designos-docs` dependency of `designos-app`.
- Left in the worktree: `target/` (large). Delete the worktree after merge.

## Weakest link

Layout parity is judged by eye plus pixel positions on one fixture, not by an automated image diff. The three scrolled/overflow behaviours (hscroll bars, scroll-spy near the bottom, smooth scroll) are covered by one test each, on a six-section page.

## Assumptions (confidence, what flips it)

- A page the manifest does not list is adopted into the group named by its front matter, else Discovery (high; flips if the epic wants unlisted pages hidden).
- Empty groups show no count pill (medium; mockup m02 shows none).
- The first menu row is hot when a menu opens (medium; mockup m13).

## Unresolved questions

1. Should the Overview page title be the project name (spec, mockups) or "About this project" (brief)?
2. Who adds `TrashKind::Page` so deleted pages can be listed and restored from Home?
3. Is the allowlist wording for the two project_tab files acceptable, or should the file calls move into `designos-docs`?
4. Why do the 3 clippy findings in `agent_permissions.rs` and `inspector_var_binds.rs` not fail CI?

## Fix round (review REQUEST_CHANGES at 9dd0b725)

One commit per finding group on the same branch, nothing pushed. Order: docs crate, library, app.

| finding | fix | test that fails without it |
|---|---|---|
| F1 trashed pages invisible, unrestorable, uncounted | `TrashKind::Page` plus `PageTrash{path, group}` on the `trash.json` item. `Library::trash_page` (hash-checked move under the docs lock, manifest update, rollback) and `Library::restore_page` (next free name on collision, back in its group, Discovery when it had none). `Library::restore` routes pages. Home lists them with a page icon and "Page . deleted ... . from <project>" and "Empty trash" counts them. | `designos-library/tests/trash_pages.rs` (8 tests); `ops_tests` delete makes a restorable item (red before: 0 items); sim `a_deleted_page_shows_in_homes_trash_is_counted_and_restore_puts_it_back` |
| F2 Cmd+S on a Project tab | save, save as, save version and the export shortcuts do nothing while a Project tab is active (`shell.rs`) | `document_save_and_export_shortcuts_do_nothing_on_a_project_tab` (red before: "No file dialog" toast) |
| F3 stale reorder drops a concurrent page | `reorder_pages` edits the manifest read now, inside `DocsStore::update_manifest` retry. Pages not in the user's order stay after the ordered ones; pages deleted meanwhile do not return. | `a_reorder_keeps_a_page_that_was_added_after_the_tree_was_drawn`, `a_reorder_does_not_bring_back_a_page_that_was_deleted_after_the_tree_was_drawn` |
| F4 case-only rename | `DocsStore::rename_page` goes through a temp name when both names are one file; the app writes the title in place, then renames | `a_rename_that_only_changes_the_case_of_the_file_name_works` (app), docs `a_rename_that_only_changes_the_case_works_on_any_file_system` |
| F5 rename/trash without the docs lock | new `DocsStore::{move_page_out, remove_page, rename_page, adopt_file}` check the hash under `.docs.lock`; rename's old-file removal and trash use them | docs `moving_a_page_out_waits_for_the_docs_lock_and_gives_up_with_busy`, `remove_page_checks_the_hash_under_the_lock`; app `a_rename_while_another_program_holds_the_docs_lock_changes_nothing`, `a_delete_while_another_program_holds_the_docs_lock_deletes_nothing` |
| F6 layout cache key | key = page bytes hash + width + pixels per point + design file names + notes; not the snapshot pointer | `a_new_snapshot_that_changed_other_pages_does_not_lay_the_open_page_out_again` (red before), `a_changed_page_a_changed_width_or_a_renamed_design_file_lays_out_again` |
| Fidelity | State rows use `Icon::Frame`, Edit source uses `Icon::Dev`, Move up/down keep chevrons | icon assert in `states_show_under_the_screen_they_belong_to` (red before), `the_page_menu_has_the_mockup_rows_and_icons` (red before) |
| F7 parked tab wakes the window | the worker repaints only when the tab was drawn in the last 2 passes (or never yet); the inbox keeps the latest state and `show` applies it on activation | `a_project_tab_behind_another_tab_does_not_wake_the_window_for_disk_changes_and_catches_up_when_shown` (1 repaint request before), `a_parked_tab_is_told_apart_from_a_visible_one_by_the_pass_it_was_last_drawn_in` |
| F8 toast cap | over the cap the oldest messages go | `messages_over_the_cap_drop_the_oldest_not_the_newest` |
| F9 env var in release | read only by a `--screenshot` run, a debug build, or `DESIGNOS_DEV` set; a `home=trash` key was added for the Home evidence | `only_a_screenshot_run_a_debug_build_or_the_dev_switch_may_use_it`, `home_trash_needs_no_project` |
| F10 tests | idle repaint with a page open (`assert_idle_window_asks_for_no_repaint`), Move up and Move down sim test | added |

Scope notes (FACT):
- Outside the Project tab, as the controller ruled: `designos-docs` (new `store_files.rs`, additive API; `DirLock` and a lock-timeout accessor made crate-visible), `designos-library` (kind and struct in `store.rs`, new `trash_pages.rs`, new dependency on `designos-docs`; docs keeps `designos-library` as a dev-dependency only, and `cargo test -p designos-docs` builds), `home/mod.rs` (trash row icon and line), `shell.rs` (shortcut guard, env gating, `home=trash`), `Cargo.lock` (one line).
- Red-first honesty: F6, F7, F2, the F1 app test and the icon asserts were run red before the fix. The library and docs tests were written first and failed to compile (API absent). F3 and F4 were written together with the fix; their red run is the reviewer's probes (ZZREORDER, ZZCASE), not mine.
- F2 covers shortcuts only, as ruled: the menu-bar File items and Spec tabs are not changed or checked.
- Overview title stays the project name (ruled). The search heading-match cue is a follow-up (ruled).
- Home's empty-trash text still says "Files and projects you delete wait here"; pages now wait there too. Not changed (copy, ux-writing baseline).
- The "after Restore" capture restores through `Library::restore`, the call Home's Restore button makes, not by a GUI click; the click path is the sim test.

Evidence (real release app, 1440x900, `plans/reports/evidence-1524/`): `real-01b-tree-overview` and `real-03-tree-context-menu` (state rows with the frame icon, Edit source with the code icon, first menu row hot), `real-06-home-trash-with-deleted-page` (Home Trash lists "Error handling", "Page . deleted just now . from Checkout", Restore, count 1 in the sidebar), `real-07-tree-after-restore` (the page is back last in Spec and opens). The four sheets were regenerated.

Gates after this round (exit codes):
- `cargo test -p designos-app --bin designos --locked project_tab`: EXIT=0, 72 passed, 0 failed, 1 ignored.
- Full `designos-app` bin run: 4320 passed, 1 failed, the known `vecops` flake (#1579); rerun alone it passes.
- `cargo test -p designos-library --locked`: EXIT=0, 70 passed.
- `cargo test -p designos-docs --locked`: EXIT=0, 99 passed.
- `cargo fmt --all --check`: EXIT=0.
- `python3 scripts/zero_fake_ui_gate.py`: EXIT=0, 17/17.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: EXIT=101 with only the 3 known errors (`agent_permissions.rs:1176`, `:1194`, `inspector_var_binds.rs:1182`); EXIT=0 with those two lints allowed.

Status: DONE_WITH_CONCERNS
Summary: Project tab (tree, reading page, On this page, rename, delete to trash, reorder, first-open Overview, live refresh) is implemented and tested; gates are green except pre-existing clippy findings, the base-identical 5/9 self-evolve gate and the two known vecops flakes.
Concerns/Blockers: two out-of-list one-line edits (fs allowlist, test module registration); idle CPU proven to one ps tick only; trashed pages are not restorable from Home.

## Round 2 (controller rulings)

- Home empty-trash line now reads "Files, projects and pages you delete wait here until you empty the trash." (`home/mod.rs`, and the unused duplicate `home/main_view.rs`). No test pinned the string.
- Menu-bar File commands that act on the tab's placeholder document are greyed, not hidden, while a Project tab is active, in both the logo menu and the File items: Save, Save as, Save .fig copy, Save to library, Show version history, Save to version history, Export selection as PNG, and the two deck PDF exports. Test: `panels::menus::tests::save_and_export_are_greyed_on_a_project_tab_and_enabled_on_a_design_tab` (enabled on a design tab, disabled on a Project tab, both builders).
- A `--menu` preset now stays open over the Project tab that a screenshot run starts on; evidence `evidence-1524/real-08-file-menu-on-project-tab.png` (all seven Save, Export and version-history rows greyed).
- Gates: see the final report of this round (menus 38 passed, project_tab 73 passed, `cargo fmt --all --check` clean).
