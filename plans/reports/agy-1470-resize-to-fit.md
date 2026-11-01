# Issue #1470 — Resize to fit, and free its shortcut from Paste to replace

## 1. Re-anchor Sites & Drift

1. **Paste to replace shortcut bindings (`crates/designos-app`)**:
   - [`crates/designos-app/src/shell.rs:936`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-app/src/shell.rs#L936): `(Key::R, true, _) => Some(Action::PasteToReplace)` matched `m.alt` with wildcard `_`, binding both `⇧⌘R` (`Ctrl+Shift+R`) and `⌥⇧⌘R` (`Ctrl+Alt+Shift+R`) to `Action::PasteToReplace` and stealing the standard `⌥⇧⌘R` / `Ctrl+Alt+Shift+R` shortcut from **Resize to fit**.
   - [`crates/designos-app/src/panels/menus.rs:475`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-app/src/panels/menus.rs#L475): `Edit -> Paste to replace` displayed `"Ctrl+Alt+Shift+R"` instead of `"Ctrl+Shift+R"`.
   - [`crates/designos-app/src/panels/context.rs:46`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-app/src/panels/context.rs#L46): `Copy/Paste -> Paste to replace` in the canvas context menu also displayed `"Ctrl+Alt+Shift+R"` instead of `"Ctrl+Shift+R"`.
   - [`crates/designos-app/src/ops.rs:846`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-app/src/ops.rs#L846): Doc comment on `App::paste_to_replace` documented both `Cmd+Shift+R` and `Cmd+Alt+Shift+R`.
2. **Object menu (`crates/designos-app/src/panels/menus.rs:489-501`)**:
   - Listed `Group selection` (`Ctrl+G`), `Ungroup` (`Ctrl+Shift+G`), `Frame selection` (`Ctrl+Alt+G`), `Add auto layout` (`Shift+A`), and `Update auto layout`, with no `Resize to fit` item.
   - `commands(app)` in [`crates/designos-app/src/panels/menus.rs:885`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-app/src/panels/menus.rs#L885) collects all leaf actions from `main_nodes(app)`, so adding `Resize to fit` to `Object` also registers it in the Command Palette (`⌘K`) with its shortcut and disabled state.
3. **Existing frame/group fit helpers (`crates/designos-doc` and `crates/designos-app`)**:
   - `crates/designos-doc`: Had no general `resize_to_fit` operation for non-auto-layout containers (`layout_apply.rs` and `layout_solver.rs` only solved auto-layout frames with `Sizing::Hug`; `PrototypeRuntime::content_bounds` in `changelog_features.rs:1134` only computed unrotated child translation boxes for prototype scroll clamping).
   - [`crates/designos-app/src/layout.rs:119`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-app/src/layout.rs#L119) (`fit_group`): Fitted only `NodeKind::Group` in its local coordinate space and did not skip hidden children (`!k.visible`).
   - [`crates/designos-app/src/ops.rs:1096`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-app/src/ops.rs#L1096) (`wrap_in_kind`): Computed world AABBs to wrap siblings into a newly created container rather than fitting an existing container in its own coordinate space.

## 2. Implementation Summary

- **`crates/designos-doc/src/layout_constraints.rs`** (re-exported at `designos_doc` root):
  - Added `is_resize_to_fit_kind`, `visible_children_bounds`, `can_resize_to_fit`, `ordered_resize_to_fit_targets`, `apply_resize_to_fit_one`, and `Document::resize_to_fit`.
  - For non-auto-layout `Frame`, `Group`, `Component`, and `ComponentSet` nodes with at least one visible child, computes the union of visible children's transformed corner bounds in the container's own coordinate space (`k.transform.apply(corner)`), translates the container by `n.transform * Affine::translate(bb.x, bb.y)`, sets `n.size = Vec2::new(bb.w, bb.h)`, and offsets direct children by `(-bb.x, -bb.y)` without applying child constraints so every child keeps its exact page transform.
  - For auto-layout frames (`auto_layout.is_some()`), sets both axes to `Sizing::Hug` (`al.primary_sizing = Sizing::Hug; al.counter_sizing = Sizing::Hug;`) and re-solves auto layout upward.
  - Orders multi-selections deepest-first and records all mutations in a single undoable history command (`Command::Update` or `Command::Batch`), skipping leaf layers (shapes, text) and containers with no visible children.
- **`crates/designos-app/src/ops.rs`, `panels/menus.rs`, `panels/context.rs`, `shell.rs`**:
  - Added `App::can_resize_to_fit` and `App::resize_to_fit` in `ops.rs` using `Tx` and `layout::relayout_around` / `layout::relayout_up` for single-step undo.
  - Added `Action::ResizeToFit` and placed `"Resize to fit"` (`"Ctrl+Alt+Shift+R"`) in `Object` immediately after `"Frame selection"`, disabled when `!app.can_resize_to_fit()`.
  - Updated `"Paste to replace"` shortcut display in `Edit` (`menus.rs:475`) and `Copy/Paste` context submenu (`context.rs:46`) to `"Ctrl+Shift+R"`.
  - Split `(Key::R, true, _)` in `shell.rs:936-937` into `(Key::R, true, false) => Some(Action::PasteToReplace)` and `(Key::R, true, true) => Some(Action::ResizeToFit)`.

## 3. What Could NOT Be Verified and Why

- Native macOS AppKit menu bar key-equivalent interception and live Metal swapchain presentation in a physical display window were not exercised because verification ran headless under the hermetic test harness. Keyboard dispatch (`Shell::key` via `Sim`), menu state (`main_nodes` / `commands`), and pixel output (`designos_render::render_page` written to `out/evidence/1470-before.png` and `out/evidence/1470-after.png`) were verified directly in tests.

## 4. Acceptance Table & Verification Output

| Requirement / Gate | Status | Evidence / Output |
|---|---|---|
| 1. Frame larger than children shrinks & children keep page position | PASS | `layout_constraints::tests::resize_to_fit_frame_larger_than_children_shrinks_and_keeps_child_page_positions ... ok` |
| 2. Frame smaller than children grows | PASS | `layout_constraints::tests::resize_to_fit_frame_smaller_than_children_grows_and_keeps_child_page_positions ... ok` |
| 3. Group fits visible children | PASS | `layout_constraints::tests::resize_to_fit_group_fits_visible_children ... ok` |
| 4. Component & ComponentSet fit visible children | PASS | `layout_constraints::tests::resize_to_fit_component_and_component_set_fit_visible_children ... ok` |
| 5. Auto layout frame turns `Hug` on both axes | PASS | `layout_constraints::tests::resize_to_fit_auto_layout_frame_turns_hug_on_both_axes ... ok` |
| 6. Hidden child is ignored; disabled when no visible children | PASS | `layout_constraints::tests::resize_to_fit_ignores_hidden_children_and_disables_when_none_visible ... ok` & `panels::menus::tests::resize_to_fit_menu_and_paste_to_replace_shortcut_labels_and_disabled_state ... ok` |
| 7. Rotated child & rotated container in own coordinate space | PASS | `layout_constraints::tests::resize_to_fit_rotated_child_and_rotated_container_use_container_space ... ok` |
| 8. Constraints are not applied | PASS | `layout_constraints::tests::resize_to_fit_does_not_apply_child_constraints ... ok` |
| 9. One undo restores everything; skips text/shapes | PASS | `layout_constraints::tests::resize_to_fit_one_undo_restores_entire_selection_and_skips_shapes_and_text ... ok` & `ops::tests::app_resize_to_fit_resizes_containers_skips_leaf_layers_and_undoes_in_one_step ... ok` |
| 10. `⌥⇧⌘R` (`Ctrl+Alt+Shift+R`) -> Resize to fit; `⇧⌘R` (`Ctrl+Shift+R`) -> Paste to replace | PASS | `shell::tests::agent_a11y::shortcut_cmd_alt_shift_r_maps_to_resize_to_fit_and_cmd_shift_r_keeps_paste_to_replace ... ok` |
| Proof: Red -> Green test log & synthetic CPU before/after PNGs | PASS | `plans/reports/agy-1470-resize-to-fit-evidence/red-green.log`; `out/evidence/1470-before.png` (11 KB), `out/evidence/1470-after.png` (6.3 KB) via `ops::tests::resize_to_fit_cpu_render_preserves_child_pixels_and_trims_excess_frame_bounds ... ok` |
| Gate: `cargo fmt --all -- --check` | PASS | `exit=0` (0 diff) |
| Gate: `cargo clippy --workspace --all-targets --locked` | PASS | `exit=0` (0 warnings in touched files; only `main`'s 2 pre-existing `clippy::needless_return` warnings in `crates/designos-app/src/agent_permissions.rs:1176,1194`) |
| Gate: `cargo test -p designos-doc --locked --lib` | PASS | `test result: ok. 715 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 27.14s` (`+9` vs `main`'s 706) |
| Gate: `cargo test -p designos-app --bin designos --locked` | PASS | `3990 passed; 0 failed; 13 ignored` (`+4` vs `main`'s 3986 passed / 13 ignored; `the_production_root_sits_outside_every_agent_writable_root` verified with `/tmp/x.XXXXXX` socket root within macOS 104-byte `sun_path` limit) |
| Gate: `cargo test --workspace --locked --lib` | PASS | `1483 passed; 0 failed; 3 ignored` across all 12 workspace library crates (`exit=0`) |
| Gate: `python3 scripts/zero_fake_ui_gate.py` | PASS | `RESULT: 16/16 Zero Fake UI invariants passed` (`exit=0`) |

### Pasted Red -> Green & Gate Excerpts

```text
=== RED: cargo test -p designos-doc --locked --lib resize_to_fit ===
running 9 tests
test layout_constraints::tests::resize_to_fit_frame_smaller_than_children_grows_and_keeps_child_page_positions ... FAILED
test layout_constraints::tests::resize_to_fit_one_undo_restores_entire_selection_and_skips_shapes_and_text ... FAILED
test layout_constraints::tests::resize_to_fit_rotated_child_and_rotated_container_use_container_space ... FAILED
test layout_constraints::tests::resize_to_fit_frame_larger_than_children_shrinks_and_keeps_child_page_positions ... FAILED
test layout_constraints::tests::resize_to_fit_auto_layout_frame_turns_hug_on_both_axes ... FAILED
test layout_constraints::tests::resize_to_fit_component_and_component_set_fit_visible_children ... FAILED
test layout_constraints::tests::resize_to_fit_does_not_apply_child_constraints ... FAILED
test layout_constraints::tests::resize_to_fit_ignores_hidden_children_and_disables_when_none_visible ... FAILED
test layout_constraints::tests::resize_to_fit_group_fits_visible_children ... FAILED
test result: FAILED. 0 passed; 9 failed; 0 ignored; 0 measured; 706 filtered out; finished in 0.01s

=== RED: cargo test -p designos-app --bin designos --locked resize_to_fit ===
running 3 tests
test panels::menus::tests::resize_to_fit_menu_and_paste_to_replace_shortcut_labels_and_disabled_state ... FAILED
test ops::tests::app_resize_to_fit_resizes_containers_skips_leaf_layers_and_undoes_in_one_step ... FAILED
test shell::tests::agent_a11y::shortcut_cmd_alt_shift_r_maps_to_resize_to_fit_and_cmd_shift_r_keeps_paste_to_replace ... FAILED
test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 3999 filtered out; finished in 0.53s

=== GREEN: cargo test -p designos-doc --locked --lib resize_to_fit ===
running 9 tests
test layout_constraints::tests::resize_to_fit_group_fits_visible_children ... ok
test layout_constraints::tests::resize_to_fit_frame_larger_than_children_shrinks_and_keeps_child_page_positions ... ok
test layout_constraints::tests::resize_to_fit_does_not_apply_child_constraints ... ok
test layout_constraints::tests::resize_to_fit_rotated_child_and_rotated_container_use_container_space ... ok
test layout_constraints::tests::resize_to_fit_frame_smaller_than_children_grows_and_keeps_child_page_positions ... ok
test layout_constraints::tests::resize_to_fit_ignores_hidden_children_and_disables_when_none_visible ... ok
test layout_constraints::tests::resize_to_fit_component_and_component_set_fit_visible_children ... ok
test layout_constraints::tests::resize_to_fit_auto_layout_frame_turns_hug_on_both_axes ... ok
test layout_constraints::tests::resize_to_fit_one_undo_restores_entire_selection_and_skips_shapes_and_text ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 706 filtered out; finished in 0.00s

=== GREEN: cargo test -p designos-app --bin designos --locked resize_to_fit ===
running 4 tests
test panels::menus::tests::resize_to_fit_menu_and_paste_to_replace_shortcut_labels_and_disabled_state ... ok
test ops::tests::app_resize_to_fit_resizes_containers_skips_leaf_layers_and_undoes_in_one_step ... ok
test ops::tests::resize_to_fit_cpu_render_preserves_child_pixels_and_trims_excess_frame_bounds ... ok
test shell::tests::agent_a11y::shortcut_cmd_alt_shift_r_maps_to_resize_to_fit_and_cmd_shift_r_keeps_paste_to_replace ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 3999 filtered out; finished in 4.92s
```

## 5. Skills Loaded & What Changed Because of Them

- **`es:dev` / `es:feature-dev`**: Structured work into re-anchoring, red unit tests first, minimal implementation within allowed files, full gate verification, and draft PR creation.
- **`es:rust`**: Used zero-copy iteration in `visible_children_bounds`, deterministic `BTreeMap<NodeId, Node>` snapshots in `Document::resize_to_fit` to emit a single `Command::Batch` / `Command::Update` undo step, and avoided cloning untouched subtrees.
- **`es:rust-gpu-canvas`**: Composed local-space container transforms via `n.transform = n.transform * Affine::translate(bb.x, bb.y)` so rotated containers shift along their rotated axes while children offset by `(-bb.x, -bb.y)` keep bit-exact page transforms, and cleared stale `fill_geometry` / `stroke_geometry` when container dimensions change.
- **`es:designer` (`design/soul.md`, `design/ux-writing.md`)**: Placed `"Resize to fit"` in the `Object` menu immediately after `"Frame selection"` (grouping container-sizing actions by Gestalt proximity and Jakob's Law), used 3-word sentence-case copy (`"Resize to fit"`) compliant with Z16, and disabled (`off(!app.can_resize_to_fit())`) rather than hiding when unavailable.
- **`es:astra-work` & `es:fable-thinking`**: Tracked claims in the epistemic ledger, predicted test outcomes before execution, and diagnosed the macOS 104-byte `sun_path` socket length failure when `TMPDIR` expanded to `/var/folders/...`.

## 6. Epistemic Ledger & Assumptions

- **`FACT`**: `shell.rs:936` previously matched `(Key::R, true, _) => Some(Action::PasteToReplace)`, intercepting `⌥⇧⌘R` (`Ctrl+Alt+Shift+R`) before any `ResizeToFit` action could run.
- **`FACT`**: For any container with transform $T_p$ and child with local transform $T_c = \text{translate}(t_x, t_y) \cdot R_c$, updating $T_p' = T_p \cdot \text{translate}(b_x, b_y)$ and $T_c' = \text{translate}(t_x - b_x, t_y - b_y) \cdot R_c$ preserves $T_p' \cdot T_c' = T_p \cdot T_c$ identically on the page regardless of container or child rotation.
- **`FACT`**: `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` binds a Unix domain socket under `$HOME/Library/Application Support/designos/ap/<id>/codex.sock`, which exceeds macOS's 104-byte `sun_path` limit (130 bytes) when `HOME` is inside `/var/folders/...`, and succeeds (86 bytes) when `HOME` is under `/tmp/x.XXXXXX/home`.
- **`INFERENCE`**: When multiple nested containers are selected simultaneously and resized to fit, processing them deepest-first (`ordered_resize_to_fit_targets`) ensures outer containers fit the already-fitted bounds of inner containers.
- **`ASSUMPTION`**: None remaining.

## 7. Git Log & Pull Request

- **Branch**: `jangtrinh/agy-1470-resize-to-fit`
- **Draft PR**: (pending creation)

Status: DONE
