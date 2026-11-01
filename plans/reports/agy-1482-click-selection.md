# Worker Report — Issue #1482: Canvas Click Selection (Outside-In & Matching Layers)

- **Branch**: `jangtrinh/agy-1482-click-selection` (commit `012bdf57`)
- **Draft PR**: https://github.com/jangtrinh/design-os-studio/pull/1489
- **Base commit**: `9db71effd5f447a0e6718d713505c4ca641b2a4b` (`origin/main`)
- **Spec**: `/Users/jangtrinh/Products/design-os-studio/plans/reports/researcher-261031-2235-figma-click-selection-re.md` (§3 R1–R12, §4 Fixture T, §5 T1–T17)
- **Out of scope (per brief)**: R13 / T18 / T19 (multi-matching hover dashed boxes and blue handle dots)

---

## 0. Re-Anchor at HEAD (`9db71effd5`) & Complete Hit-Test Caller Inventory

### Cited Studio lines vs `HEAD` (`9db71effd5`)

| Spec Citation | Actual Location at Base `HEAD` | Drift Notes |
|---|---|---|
| `crates/designos-app/src/canvas/hit.rs:26-46` | `hit.rs:26-46` (`is_selected_or_ancestor`, `can_enter`) | Exact match. Base `can_enter` unconditionally entered `Page`, `Section`, `Frame`, `Component`, `ComponentSet`, and `Table` (except one-shape components), and only gated `Group`, `Instance`, `BooleanOp`, and `Vector` on `is_selected_or_ancestor(doc, id, entered)`. |
| `crates/designos-app/src/canvas/hit.rs:181` | `hit.rs:181` (`drill_down_select_with_index`) | Exact match. Base `drill_down_select_with_index` only handled `Group`, `Instance`, and `BooleanOp` and ignored nested `Frame`, `Component`, and `ComponentSet`. |
| `crates/designos-app/src/canvas/hit.rs:193-197` | `hit.rs:193-197` (`marquee_select_with_index`) | Exact match. Base `marquee_select_with_index` recursed into `Page` and `Section`, plus `Frame`/`Component`/`ComponentSet` only when `parent == Some(page_id)` (missing frames direct inside a `Section`). |
| `entered()` (`crates/designos-app/src/canvas/interact.rs`) | `interact.rs:60-62` (`fn entered(app: &App) -> &[NodeId] { &app.selection }`) | Exact match. |
| `MatchCriteria::Properties` menu wiring | `crates/designos-app/src/panels/menus.rs:429` (`same_nodes`), `crates/designos-app/src/panels/context.rs:437` (`select_submenu`) | Exact match. Both menus had 6 criteria (`Properties`, `Fill`, `Stroke`, `Effect`, `TextProperties`, `Font`) and `Action::SelectMatching` in `find/select.rs` delegated to `select_same(app, MatchCriteria::Properties)`. |
| Hover outline call (`crates/designos-app/src/canvas/draw.rs`) | `draw.rs:40` (`if let Some(hid) = app.hovered { if !app.selection.contains(&hid) { draw_hover_outline(...); } }`) | Exact match. |

### Every Caller of `hit_test` / `hit_test_with_index` in `crates/designos-app/src/`

| Caller Location | Function Called (Before -> After) | Classification |
|---|---|---|
| `crates/designos-app/src/canvas/interact.rs:88` (hover update at start of `update`) | `hit::hit_test_with_index` -> `hit::hit_test_with_index` (`deep = ctrl_down`, `entered(app)`, plus Shift matching-layer hover resolution) | **Selection** |
| `crates/designos-app/src/canvas/interact.rs:584` (Move tool primary press) | `hit::hit_test_with_index` -> `hit::hit_test_with_index` (`deep = m.command`, `entered(app)`, plus Shift matching-layer click resolution) | **Selection** |
| `crates/designos-app/src/canvas/interact.rs:640` (Move tool double-click) | `hit::drill_down_select_with_index` / `hit::hit_test_with_index` | **Selection** |
| `crates/designos-app/src/canvas/interact.rs:765` (Canvas right-click context menu target) | `hit::hit_test_with_index` (`deep = false`, `entered(app)`) + `hit::hit_all_layers_with_index` for `Cmd+Right-click` and `Select layer >` | **Selection** |
| `crates/designos-app/src/canvas/interact.rs:1012` (Move tool click-release inside selected group/frame) | `hit::hit_test_with_index` (`deep = m.command`, `entered(app)`) | **Selection** |
| `crates/designos-app/src/comments/pins.rs:34` (`Tool::Comment` pin placement) | `hit::hit_test_with_index` -> `hit::hit_test_with_index_through_nested_frames` | **Non-selection** (Ruling 6: keeps through-nested-frames behavior so pins attach to nested layers) |
| `crates/designos-app/src/canvas/vector.rs:351` (Vector/Pen tool hover target) | `hit::hit_test` -> `hit::hit_test_through_nested_frames` | **Non-selection** (Ruling 6: keeps through-nested-frames behavior) |
| `crates/designos-app/src/panels/properties.rs` & `crates/designos-app/src/canvas/draw.rs` | No direct `hit_test` calls (`draw.rs` consumes `app.hovered` / `app.layer_hovered`; `draw.rs` and `properties.rs` inspect `app.selection`) | **Non-selection** (Ruling 6: unchanged) |

---

## 1. Summary of Changes (R1–R12)

| Rule | Summary | Files & Symbols |
|---|---|---|
| **R1** | Outside-in click selection: top-level `Frame`, `Component`, and `ComponentSet` nodes (parent is `Page` or `Section`, excluding single-shape components) are click-through; nested frames (`List`, `Btn2`, etc.), groups, instances, and boolean operations are opaque (`can_enter_for_selection` stops at them unless selected/ancestor-selected). Double-click (`drill_down_select`) enters one level per double-click (`A -> List -> Btn2 -> Label`). | `crates/designos-app/src/canvas/hit.rs` (`can_enter_for_selection`, `hit_test_with_index_mode`, `drill_down_select_with_index`) |
| **R2** | Deep-select (`Cmd+click` / `Ctrl+click`, `Cmd+Shift+click` / `Ctrl+Shift+click`) bypasses container boundaries (`deep = m.command`) and selects or toggles the deepest leaf layer at the cursor (`Label`). | `crates/designos-app/src/canvas/interact.rs` (Move tool click & release handler) |
| **R3** | Marquee drag selection: plain marquee enters `Page`, `Section`, and top-level `Frame`/`Component`/`ComponentSet` (`marquee_select_with_index`); `Cmd+drag` / `Ctrl+drag` deep marquee (`marquee_select_deep_with_index`) selects leaf layers at any depth whose world bounds intersect the marquee rect. | `crates/designos-app/src/canvas/hit.rs` (`marquee_select_with_index`, `marquee_select_deep_with_index`), `crates/designos-app/src/canvas/interact.rs` |
| **R4** | Shift-click and Shift-marquee matching-layer selection: when `Shift` (without `Cmd`/`Ctrl`) is held and the current selection has a valid matching scope, clicking or marquee-dragging over a matching layer in another container (`B/List/Btn2`) selects the matching layer directly instead of its outer container (`B`). A Shift-marquee that touches another container without hitting any matching layer leaves the selection unchanged. | `crates/designos-app/src/canvas/interact.rs`, `crates/designos-doc/src/find.rs` (`find_matching_layers`) |
| **R5** | Hover target (`app.hovered`) uses the same selection-aware hit test as click (`ctrl_down` for deep hover; `Shift` matching candidate resolution when holding Shift over a matching layer in another container). | `crates/designos-app/src/canvas/interact.rs` (hover update at start of `update`) |
| **R6** | Hover outline (`canvas::draw::draw_hover_outline`) is suppressed when `app.ui.hide_hover_highlight` is true; added `Preferences > Highlight layers on hover` (`Action::ToggleHoverHighlight`, checked by default). | `crates/designos-app/src/app.rs` (`UiState::hide_hover_highlight`), `crates/designos-app/src/canvas/draw.rs`, `crates/designos-app/src/panels/menus.rs` |
| **R7** | Layers panel hover (`app.layer_hovered`) draws the hover outline on the canvas even when `app.ui.hide_hover_highlight` is enabled (`let hover_id = app.layer_hovered.or(if app.ui.hide_hover_highlight { None } else { app.hovered })`). | `crates/designos-app/src/app.rs` (`App::layer_hovered`), `crates/designos-app/src/panels/layers.rs`, `crates/designos-app/src/canvas/draw.rs` |
| **R8** | `Enter` (`select_children`) selects direct children of every selected container (`List -> [Btn1, Btn2, Avatar]`); `Shift+Enter` (`select_parent`) selects the parent (`[Btn1, Btn2, Avatar] -> [List]`); `Backslash` (`\`) also selects the parent; `Tab` / `Shift+Tab` (`cycle_sibling`) cycles siblings in layer-list order (`Btn1 -> Btn2 -> Avatar -> Btn1` and reverse); `Escape` clears selection (`escape_clears_selection`). | `crates/designos-app/src/ops.rs` (`select_children`, `select_parent`, `cycle_sibling`), `crates/designos-app/src/shell.rs` |
| **R9** | `Select all with same …` (`MatchCriteria`) extended with `MatchCriteria::Instance` (all instances of the same component or component set across the page, plus the main component if in the initial selection) and `MatchCriteria::Variant` (all instances on the page bound to the exact same variant component). Added to both `Main menu > Edit > Select all with same` and `Right-click > Select > Select all with same`, enabled via `can_select_same_instance` / `can_select_same_variant`. | `crates/designos-doc/src/find.rs` (`MatchCriteria::{Instance, Variant}`, `can_select_same_instance`, `can_select_same_variant`), `crates/designos-app/src/panels/menus.rs`, `crates/designos-app/src/panels/context.rs` |
| **R10** | `Select matching layers` (`Opt+Cmd+A` / `Ctrl+Alt+A`, `Edit > Select matching layers`, `Right-click > Select > Select matching layers`): matches layers across sibling top-level containers, inside a section (`ScopeKind::Section`), or across instances of the same component/component set (`ScopeKind::ComponentFamily`) by relative path, same-named sibling index, and normalized geometry within `5 px` tolerance. Shows scoped empty toast (`"No matching layers to select"` / `"No matching layers to select in this section"`). | `crates/designos-doc/src/find.rs` (`find_matching_layers`, `matching_scope_of`, `MatchingScopeKind`), `crates/designos-app/src/find/select.rs` (`select_matching`) |
| **R11** | `Cmd+Right-click` / `Ctrl+Right-click` opens a flat layer-picker context menu (`ctx_menu_layer_pick = true`) listing `hit_all_layers_with_index` at the cursor in paint order (topmost leaf first: `Label, Btn2, List, A`). Clicking a row selects that layer. | `crates/designos-app/src/canvas/hit.rs` (`hit_all_layers_with_index`), `crates/designos-app/src/panels/context.rs` (`layer_picker_nodes`), `crates/designos-app/src/canvas/interact.rs` |
| **R12** | Plain right-click context menu includes `Select layer >` (`select_layer_submenu`) listing every hit layer at the cursor in outside-in order (`A, List, Btn2, Label`). Hovering a row in either the flat picker or `Select layer >` sets `app.layer_hovered` so the canvas previews that layer's hover outline. | `crates/designos-app/src/panels/context.rs` (`select_layer_submenu`), `crates/designos-app/src/widgets/menu.rs` (`MenuItem::hover_node`, `MenuOut::hovered_node`) |

---

## 2. Default Decisions (Ruling 2 Named Helpers)

Every open question from §8 of the research report is isolated in a dedicated named helper with a doc comment ending `"to be confirmed against Figma"`:

1. **`transparent_container_selects_self_on_empty_area`** (`crates/designos-app/src/canvas/hit.rs:53`) — Clicking empty space inside a transparent container selects the container itself when no child is hit.
2. **`double_click_drills_into_text_edits_immediately`** (`crates/designos-app/src/canvas/interact.rs:64`) — Returns `false` so double-clicking a container drills down to select the child `Text` node first; a subsequent double-click on the already-selected `Text` node enters inline text editing.
3. **`deep_marquee_selects_leaf_layers`** (`crates/designos-app/src/canvas/hit.rs:61`) — `Cmd+drag` / `Ctrl+drag` marquee selects leaf layers (`children.is_empty()`) at any depth whose world bounds intersect the marquee rect.
4. **`matching_hierarchy_position`** (`crates/designos-doc/src/find.rs:665`) — Matching layers require identical relative container path, same-named sibling index, and normalized relative geometry within `5 px` tolerance (`MATCHING_POS_TOLERANCE = 5.0`).
5. **`escape_clears_selection`** (`crates/designos-app/src/shell.rs:956`) — Pressing `Escape` in the Move tool clears the current selection (mirrors Figma rather than walking up to the parent).
6. **`select_layer_submenu_lists_parents_first`** (`crates/designos-app/src/panels/context.rs:429`) — `Right-click > Select layer >` orders layers outside-in (parents first: `A, List, Btn2, Label`), while `Cmd+Right-click` orders layers topmost-leaf-first (`Label, Btn2, List, A`).

---

## 3. Red → Green Run of T1–T17 on Fixture T

- **Red log**: `plans/reports/agy-1482-click-selection-evidence/red-t1-t17.log` (`5 passed; 12 failed`)
- **Green log**: `plans/reports/agy-1482-click-selection-evidence/green-t1-t17.log` (`21 passed; 0 failed`)

### Why 5 tests already passed on untouched `main` (`red-t1-t17.log`)

| Test | Base Status | Reason it passed on `main` |
|---|---|---|
| `t2_click_group_child_with_nothing_selected_selects_group` | PASS on base | Base `can_enter` already treated `NodeKind::Group` as opaque unless selected or ancestor-selected (`is_selected_or_ancestor`). |
| `t5_click_sibling_branch_when_deeply_selected_selects_direct_child_of_shared_ancestor` | PASS on base | In Fixture T, `Avatar` is a leaf `Ellipse` inside `A/List`, so clicking `Avatar` when `A/List/Btn1/Label` was selected returned `Avatar` both under base's always-enter-frames rule and under outside-in shared-ancestor rules. |
| `t8_enter_selects_children_and_shift_enter_selects_parent` | PASS on base | `ops::select_children` (`Enter`), `ops::select_parent` (`Shift+Enter`), and `ops::select_all` (`Cmd+A`) inside a selected container were already implemented for a single container in `ops.rs`. |
| `t9_cmd_click_deep_selects_innermost_layer` | PASS on base | Base `hit_test` drilled through all frames to the innermost leaf (`Label`), which coincided with `Cmd+click`'s deep-select target. |
| `t13_shift_click_on_non_matching_layer_uses_normal_outside_in_hit` | PASS on base | `B/List/Avatar` is a leaf `Ellipse`, so Shift-clicking `B/List/Avatar` with `A/List/Btn2` selected added `B/List/Avatar` to the selection on base. |

All 12 remaining tests (`t1`, `t3`, `t4`, `t6`, `t7`, `t10`, `t11`, `t12`, `t14`, `t15`, `t16`, `t17`) failed on untouched `main` and pass on the final commit (`21 passed; 0 failed` in `green-t1-t17.log`, covering T1–T17 plus Ruling 6 regression test `comment_pin_inside_nested_frame_attaches_to_nested_layer`, `hover_highlight_preference_toggles_canvas_hover_outline`, `select_all_with_same_text_properties_and_variant`, and `select_layer_submenu_includes_locked_layers_and_can_select_them`).

---

## 4. Ruling 6 Non-Selection Caller Audit & Regression Test

Non-selection callers of `hit::hit_test` / `hit::hit_test_with_index` continue to drill through nested frames as before via `hit_test_through_nested_frames` / `hit_test_with_index_through_nested_frames`:

| Caller | Purpose | Behavior |
|---|---|---|
| `crates/designos-app/src/comments/pins.rs:34` | Attaching a comment pin to a layer under the cursor | Calls `hit::hit_test_with_index_through_nested_frames` so pins attach to nested frames/children inside top-level frames, unchanged from `main`. |
| `crates/designos-app/src/canvas/vector.rs:351` | Pen/Vector vertex hover hit-testing | Calls `hit::hit_test_through_nested_frames`, unchanged from `main`. |
| `crates/designos-app/src/panels/properties.rs` | Inspector eyedropper / target lookup | Unchanged. |
| `crates/designos-app/src/canvas/draw.rs` | Frame/selection/hover overlays | Reads `app.selection`, `app.hovered`, and `app.layer_hovered` (updated for R6/R7). |

- **Added Ruling 6 regression test**: `shell::tests::click_selection::comment_pin_inside_nested_frame_attaches_to_nested_layer` (`crates/designos-app/src/shell/tests/click_selection.rs:495`) — clicks inside nested frame `A/List/Btn2` with `Tool::Comment` active and verifies `composer.anchor.node == Some(t.a.btn2)`.

---

## 5. Ruling 7 Audit of Existing Tests

Checked all 5 named test modules plus the full `designos` binary test suite (`4,007` tests):

| Test Module / Test | Status | Old Expectation vs New Expectation |
|---|---|---|
| `shell/tests/effect_stop_binds.rs` (16 tests) | Unchanged (16/16 pass) | Uses `select_only` or top-level shapes; no nested-frame click-through dependency. |
| `shell/tests/vector.rs` (32 tests) | Unchanged (32/32 pass) | Vector tool uses `hit_test_through_nested_frames` or top-level paths; all 32 pass unchanged. |
| `shell/tests/fast_drag_transform.rs` (18 tests) | Unchanged (18/18 pass) | Clicks direct children of top-level frames or pre-selected nodes; all 18 pass unchanged. |
| `shell/tests/comments.rs` (16 tests) | Unchanged (16/16 pass) | Comment pins use `hit_test_with_index_through_nested_frames` (Ruling 6); all 16 pass unchanged. |
| `shell/tests/canvas_label_rename.rs` (17 tests) | Unchanged (17/17 pass) | Tests top-level frame label clicks/double-clicks; all 17 pass unchanged. |
| `shell::tests::find::ctrl_alt_a_selects_matching_layers` (`crates/designos-app/src/shell/tests/find.rs:233`) | **Updated** | **Old**: Duplicated `Stat Tile A` inside the same `Dashboard` frame and asserted `Ctrl+Alt+A` selected the sibling duplicate (`Ctrl+Alt+A` previously aliased `MatchCriteria::Properties`).<br>**New**: Keeps the in-frame sibling duplicate assertion on `Action::SelectSame(MatchCriteria::Properties)`, and duplicates the top-level `Dashboard` frame (`Dashboard Copy`) to assert `Ctrl+Alt+A` selects `Stat Tile A` across both top-level frames (`R10`). |

---

## 6. Proof & Headless CLI Capture Note

- **Terminal proof logs** (under `plans/reports/agy-1482-click-selection-evidence/`):
  - `red-t1-t17.log` — Red run of `click_selection` against base behavior (`5 passed; 12 failed`).
  - `green-t1-t17.log` — Green run of `click_selection` on final implementation (`21 passed; 0 failed`).
  - `workspace-lib-tests.log` — `cargo test --workspace --locked --lib` (`0 failed` across all workspace crates, including 2 new `designos-doc` unit tests).
  - `designos-bin-tests.log` — `cargo test -p designos-app --bin designos --locked`.
  - `clippy.log` — `cargo clippy --workspace --locked`.
  - `zero-fake-ui-gate.log` — `python3 scripts/zero_fake_ui_gate.py` (`17/17 PASS`).
  - `check-spacing-relationships.log` — `python3 scripts/check_spacing_relationships.py` (`PASS`, `purple-band hits: 0`).
  - `design-os-self-evolve.log` — `python3 scripts/design_os_self_evolve.py` (`9/9 PASS`).
- **Headless CLI selection/hover outline capture**: **SKIPPED** — `--screenshot` in `designos-app/src/cli.rs` launches a live native `eframe` window (`eframe::run_native`), and `brief-common-argon.txt` strictly forbids opening any window on `--fixture` or synthetic `.designos` files and only permits live window capture on an APFS clone of the Untitled UI kit when `HIDIdleTime >= 300s`. Selection and hover outline geometry and toggle state are verified headlessly in `shell::tests::click_selection::t7_hover_highlights_outermost_unentered_and_cmd_hover_highlights_leaf` and `t8_preference_highlight_layers_on_hover_and_layers_panel_hover_override`.

---

## 7. Verification Gates & Test Counts vs `main`

| Gate | Command | Result |
|---|---|---|
| Rustfmt | `cargo fmt --all -- --check` | **PASS** (0 diffs) |
| Clippy | `cargo clippy --workspace --locked` | **PASS** (0 new warnings; only `main`'s pre-existing `Cargo.toml` duplicate bin target notice) |
| Binary unit tests (`designos`) | `cargo test -p designos-app --bin designos --locked` | **PASS** (`4,007 passed; 0 failed; 13 ignored` vs `3,986 passed` on `main`, **+21 new tests** in `shell::tests::click_selection`) |
| Workspace library tests | `cargo test --workspace --locked --lib` | **PASS** (`0 failed` across all crates; `designos-doc` `169 passed` vs `167` on `main`, **+2 new unit tests**) |
| Zero Fake UI Gate | `python3 scripts/zero_fake_ui_gate.py` | **PASS** (`17/17 invariants satisfied`) |
| Spacing & Purple Ban | `python3 scripts/check_spacing_relationships.py` | **PASS** (`purple-band hits: 0`) |
| Self-Evolution Gate | `python3 scripts/design_os_self_evolve.py` | **PASS** (`9/9 gates passed`) |


---

## Round 2

### 1. What Could Not Be Verified

- Live Figma Desktop side-by-side manual interaction could not be run in this headless agent session; all Figma parity rules and edge cases were verified via deterministic `Sim` integration tests, `designos-doc` unit tests, mutation testing (M1–M15), and release-mode benchmarking on an APFS clone of the Untitled UI kit (`"Landing pages"`, 67,717 nodes).
- During the full 4,065-test `cargo test -p designos-app --bin designos --locked` run with `S` placed under macOS's `/var/folders/.../T` (`$TMPDIR`), 1 test (`agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root`) hit the 104-byte Unix domain socket path limit (`130 bytes`) because `/var/folders/...` is 49 bytes longer than `/tmp`, and 3 wall-clock/budgeted tests (`agent_mention::tests::search_is_budgeted_on_a_huge_document`, `shell::tests::close_human_dirty::library_file_read_only_close_save_is_invalid_arguments_not_path_required`, `shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required`) timed out under 16-thread CPU contention (`r2-test-bin-designos.log`). Re-running those modules with `S="$(mktemp -d /tmp/x.XXXXXX)"` passed 14/14 (`0 failed`, `r2-test-bin-designos-reruns.log`).

### 2. Round 2 Rulings Addressed

1. **Nested selection definition & `Shift + drag` marquee toggle (`crates/designos-app/src/canvas/hit.rs`, `crates/designos-app/src/canvas/interact.rs`, `crates/designos-app/src/canvas/mod.rs`)**:
   - Added `hit::is_nested_selection_layer(doc, id)`: a layer counts as nested when it sits deeper than plain-click depth (an ancestor between `id` and the page is opaque to a plain click with nothing selected); a direct child of a top-level frame is at click depth and is not nested.
   - Deleted `touches_other_container`. With a nested selection, `Shift + drag` toggles only matching layers intersecting the marquee rectangle (adding unselected matches and removing already-selected matches, including base members that match another base member) and adds nothing else. Without a nested selection, `Shift + drag` behaves as `main`.
2. **Precomputed `Shift + drag` candidates & fast cross-container matching (`crates/designos-doc/src/find.rs`, `crates/designos-app/src/canvas/mod.rs`, `crates/designos-app/src/canvas/interact.rs`)**:
   - Stored `matching: Option<Vec<(NodeId, Rect)>>` on `Interaction::Marquee`, computed once at drag start via `marquee_matching_candidates(app, &base)`. Each drag frame only tests the precomputed `(NodeId, Rect)` pairs against the marquee rectangle `r`.
   - In `designos-doc/src/find.rs`, `PreparedMatch::new` computes each selected layer's scope, scope root, ancestor path, effective name, sibling index, and relative position once; `walk_in_scope_root` prunes container traversal by ancestor depth/name and `matches_prepared` checks `same_kind` and `ancestor_names_match` before scope/path/style checks.
   - Removed the unnecessary `'a` lifetime on `matching_hierarchy_position` (`clippy::needless_lifetimes` at `find.rs:589`).
3. **Eyedropper deep hit (`crates/designos-app/src/panels/properties.rs`)**:
   - Updated `try_sample_color_token` to resolve the sampled node via `interact::hit_under_any_depth(app, page_pt, false).or_else(|| interact::hit_under(app, page_pt, true))` so `Shift + click` with the eyedropper samples the deepest styled nested layer (`S-Btn` on `Btn2`) rather than the outer frame (`S-List` on `List`).
4. **Persistent `"Highlight layers on hover"` preference (`crates/designos-app/src/panels/menus.rs`, `crates/designos-app/src/shell.rs`)**:
   - Persists across launches via `lib.save_flag("highlight_layers_on_hover", !app.ui.hide_hover_highlight)` on toggle and `lib.load_flag("highlight_layers_on_hover")` in `App::attach_library`.
5. **Shared `press_target` & hover cache key (`crates/designos-app/src/canvas/interact.rs`, `crates/designos-app/src/canvas/mod.rs`)**:
   - Extracted `pub(crate) fn press_target(app: &App, page: DV, mods: egui::Modifiers) -> Option<NodeId>` shared by pointer press and hover preview.
   - Updated `HoverCacheKey` to `(Pos2, u64, bool, bool, u64)` including `mods.command`, `mods.shift`, and `selection_hover_hash(&app.selection)`.
6. **`Select layer` submenu ordering, 32-item cap, and Layers-panel context menu guard (`crates/designos-app/src/canvas/hit.rs`, `crates/designos-app/src/panels/context.rs`)**:
   - Added `parents_first: bool` to `hit_all_in` / `hit_all_under_including_locked`, walking `pn.children_front_to_back()` and pushing containers before their children when `parents_first` is true so `Select layer` lists parents first and front-most sibling first, and the 32-item cap drops the deepest layers rather than the outermost parents.
   - Changed `select_layer_candidates` in `panels/context.rs` to require `let page_pt = app.ui.ctx_menu_page?;` so a context menu opened from the Layers panel (`ctx_menu_page == None`) never adds a canvas `Select layer` row even if the pointer moves over the canvas.
7. **Plain marquee doc comment & test (`crates/designos-app/src/canvas/hit.rs`, `crates/designos-app/src/shell/tests/click_selection.rs`)**:
   - Updated the doc comment on `marquee_select` to document that nested frames, groups, instances, main components, and component sets are selected as a whole (matching plain-click depth), and pinned with a test.
8. **Symmetric section scope & `Select all with same` instance vs variant (`crates/designos-doc/src/find.rs`)**:
   - `scope_compatible` now checks `MatchScope::Page(page) => c_scope == MatchScope::Page(page)` (`s_scope == c_scope`).
   - `MatchCriteria::Instance` matches instances of the same main component (`main_of(n) == Some(ref_main)`), while `MatchCriteria::Variant` matches instances of any variant in the same component set (`variant_set_of`).
9. **Test hygiene & mutation coverage (`crates/designos-app/src/shell/tests/click_selection.rs`)**:
   - Renamed all tests in `click_selection.rs` to behaviour-only names (zero `t1_..t17_`, issue numbers, review codes, or mutant IDs in code comments, assert messages, or test names).
   - Expanded `click_selection.rs` to 28 tests; verified all 8 new/updated Round 2 tests fail on `012bdf57` (`20 passed; 8 failed`, `r2-red-012bdf57.log`), 21/28 fail on `origin/main` (`7 passed; 21 failed`, `r2-red-main.log` / `red-t1-t17.log`), all 28 pass on `HEAD` (`28 passed; 0 failed`, `r2-green-head.log`), and all 15 mutants `M1`–`M15` (including all 9 target mutants `M7`–`M15`) are killed (`r2-mutants.log`).

### 3. Performance Measurement on Untitled UI Kit `"Landing pages"` (67,717 nodes)

Log: `plans/reports/agy-1482-click-selection-evidence/r2-perf-landing-page.log`

```text
running 1 test
PROBE import_s=7.2
PROBE largest_page=NodeId(88674) name="      ❖ Landing pages" nodes=67717
PROBE nested_text_candidates=9761
PROBE sel=NodeId(695506) depth=6 matches=35 drag_start_ms=0.223 drag_frame_ms=0.0002 frame_sel=13
PROBE sel=NodeId(704517) depth=9 matches=3 drag_start_ms=0.650 drag_frame_ms=0.0000 frame_sel=1
PROBE sel=NodeId(712957) depth=8 matches=7 drag_start_ms=2.865 drag_frame_ms=0.0000 frame_sel=2
PROBE sel=NodeId(722684) depth=9 matches=6 drag_start_ms=0.177 drag_frame_ms=0.0000 frame_sel=4
PROBE sel=NodeId(730694) depth=7 matches=1 drag_start_ms=0.239 drag_frame_ms=0.0000 frame_sel=1
PROBE sel=NodeId(738063) depth=7 matches=10 drag_start_ms=0.133 drag_frame_ms=0.0001 frame_sel=7
PROBE sel=NodeId(745544) depth=8 matches=15 drag_start_ms=2.683 drag_frame_ms=0.0000 frame_sel=4
PROBE sel=NodeId(753914) depth=11 matches=1 drag_start_ms=0.151 drag_frame_ms=0.0000 frame_sel=2
PROBE SUMMARY drag_start_median_ms=0.239 drag_start_max_ms=2.865 drag_frame_median_ms=0.0000 drag_frame_max_ms=0.0002
test perf_probe_landing_page_shift_marquee ... ok
```

### 4. Mutant Verification (`M1`–`M15`)

Log: `plans/reports/agy-1482-click-selection-evidence/r2-mutants.log`

```text
M1 (crates/designos-app/src/canvas/hit.rs): KILLED rc=101 failed=9 ['test shell::tests::click_selection::hover_outline_matches_plain_and_cmd_click_target ... FAILED', 'test shell::tests::click_selection::click_nested_layer_inside_section_frame_selects_direct_child_of_section_frame ... FAILED', 'test shell::tests::click_selection::hover_highlight_preference_toggles_canvas_hover_outline_and_persists_across_shells ... FAILED']
M2 (crates/designos-app/src/canvas/hit.rs): KILLED rc=101 failed=4 ['test canvas::hit::tests::click_empty_frame_area_selects_frame ... FAILED', 'test canvas::hit::tests::clicks_still_reach_the_layers_of_a_main_component ... FAILED', 'test canvas::hit::tests::hit_and_marquee_with_scene_index_matches ... FAILED', 'test canvas::hit::tests::locked_and_hidden_are_skipped ... FAILED']
M3 (crates/designos-app/src/canvas/interact.rs): KILLED rc=101 failed=2 ['test shell::tests::click_selection::shift_hover_and_selection_change_update_hover_target_to_match_press_target ... FAILED', 'test shell::tests::click_selection::shift_click_toggles_matching_nested_layer_across_frames ... FAILED']
M4 (crates/designos-app/src/canvas/interact.rs): KILLED rc=101 failed=3 ['test shell::tests::click_selection::click_sibling_branch_when_deeply_selected_selects_direct_child_of_shared_ancestor ... FAILED', 'test shell::tests::click_selection::double_click_drills_one_level_per_double_click_and_starts_text_edit_on_text ... FAILED', 'test shell::tests::click_selection::shift_hover_and_selection_change_update_hover_target_to_match_press_target ... FAILED']
M5 (crates/designos-app/src/canvas/interact.rs): KILLED rc=101 failed=1 ['test shell::tests::click_selection::double_click_drills_one_level_per_double_click_and_starts_text_edit_on_text ... FAILED']
M6 (crates/designos-doc/src/find.rs): KILLED rc=101 failed=4 ['test shell::tests::click_selection::select_matching_layers_selects_matching_in_other_frames_with_toast ... FAILED', 'test shell::tests::click_selection::shift_marquee_over_frame_with_nested_selection_adds_only_matching_layers ... FAILED', 'test shell::tests::click_selection::shift_marquee_with_nested_selection_toggles_only_matching_layers_and_caches_candidates ... FAILED']
M7 (crates/designos-doc/src/find.rs): KILLED rc=101 failed=1 ['test shell::tests::click_selection::select_matching_layers_enforces_symmetric_section_scope_sibling_index_and_text_override_rules ... FAILED']
M8 (crates/designos-doc/src/find.rs): KILLED rc=101 failed=1 ['test shell::tests::click_selection::select_matching_layers_enforces_symmetric_section_scope_sibling_index_and_text_override_rules ... FAILED']
M9 (crates/designos-doc/src/find.rs): KILLED rc=101 failed=1 ['test shell::tests::click_selection::select_matching_layers_enforces_symmetric_section_scope_sibling_index_and_text_override_rules ... FAILED']
M10 (crates/designos-app/src/canvas/draw.rs): KILLED rc=101 failed=1 ['test shell::tests::click_selection::hover_highlight_preference_toggles_canvas_hover_outline_and_persists_across_shells ... FAILED']
M11 (crates/designos-app/src/comments/pins.rs): KILLED rc=101 failed=1 ['test shell::tests::click_selection::comment_pin_inside_nested_frame_attaches_to_nested_layer ... FAILED']
M12 (crates/designos-app/src/canvas/vector.rs): KILLED rc=101 failed=1 ['test shell::tests::click_selection::eyedropper_shift_click_samples_deep_nested_token_and_vector_edit_exits_on_nested_sibling ... FAILED']
M13 (crates/designos-doc/src/find.rs): KILLED rc=101 failed=1 ['test shell::tests::click_selection::select_matching_layers_enforces_symmetric_section_scope_sibling_index_and_text_override_rules ... FAILED']
M14 (crates/designos-app/src/canvas/hit.rs): KILLED rc=101 failed=1 ['test shell::tests::click_selection::shift_marquee_with_direct_child_selected_extends_to_siblings_and_plain_marquee_hits_nested_frame ... FAILED']
M15 (crates/designos-app/src/canvas/interact.rs): KILLED rc=101 failed=2 ['test shell::tests::click_selection::shift_marquee_over_frame_with_nested_selection_adds_only_matching_layers ... FAILED', 'test shell::tests::click_selection::shift_marquee_with_nested_selection_toggles_only_matching_layers_and_caches_candidates ... FAILED']
```

### 5. Exact Commands & Pasted Output

```text
$ git log --oneline origin/main..HEAD
6d12cb22 fix(canvas): refine Shift-marquee matching, hover cache, Select layer order, and matching rules (#1482)
1ebd79ee Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1482-click-selection
012bdf57 feat(canvas): click selects outside-in like Figma, and Shift-click adds matching layers (#1482)

$ git diff origin/main -- ':(exclude)plans/' | grep -nE '^\+.*\b(t[0-9]{1,2}_|R[0-9]{1,2}|F[0-9]{1,2}|M[0-9]{1,2}|L[0-9]|#14[0-9]{2})\b'
(0 matches, exit 1)

$ cargo fmt --all -- --check
(exit 0, plans/reports/agy-1482-click-selection-evidence/r2-fmt.log)

$ cargo clippy --workspace --all-targets --locked -- -D warnings
error: unneeded `return` statement (crates/designos-app/src/shell/tests/agent_permissions.rs:344:9, 369:9 — main's 2 known lints)
$ cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::needless_return
Finished `dev` profile [unoptimized + debuginfo] target(s) (exit 0, plans/reports/agy-1482-click-selection-evidence/r2-clippy-clean.log)

$ cargo test -p designos-app --bin designos --locked click_selection
# on 012bdf57 (plans/reports/agy-1482-click-selection-evidence/r2-red-012bdf57.log):
test result: FAILED. 20 passed; 8 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 5.01s
# on origin/main (plans/reports/agy-1482-click-selection-evidence/r2-red-main.log):
test result: FAILED. 7 passed; 21 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 5.24s
# on HEAD 6d12cb22 (plans/reports/agy-1482-click-selection-evidence/r2-green-head.log):
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 7.48s

$ cargo test --workspace --locked --lib
# (plans/reports/agy-1482-click-selection-evidence/r2-test-workspace-lib.log)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 711 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.37s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.06s
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.20s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.87s
test result: ok. 322 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.41s
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 43.51s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
(Total: 1,479 passed; 0 failed vs 1,476 on main, +3 unit tests in designos-doc)

$ cargo test -p designos-app --bin designos --locked
# Full suite (plans/reports/agy-1482-click-selection-evidence/r2-test-bin-designos.log):
test result: FAILED. 4048 passed; 4 failed; 13 ignored; 0 measured; 0 filtered out; finished in 641.07s
# Targeted rerun with short /tmp/x.XXXXXX path (plans/reports/agy-1482-click-selection-evidence/r2-test-bin-designos-reruns.log):
test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok (1 passed; 0 failed)
test agent_mention::tests::search_is_budgeted_on_a_huge_document ... ok (1 passed; 0 failed)
test shell::tests::close_human_dirty ... ok (12 passed; 0 failed)
(Total: 4,065 tests vs 4,037 on main, +28 tests in shell::tests::click_selection)

$ python3 scripts/zero_fake_ui_gate.py
ZERO FAKE UI GATE: 17/17 invariants satisfied. PASS.
```

Status: DONE
