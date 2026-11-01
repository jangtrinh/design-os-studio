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
- **Draft PR**: https://github.com/jangtrinh/design-os-studio/pull/1488
- **`git log --oneline origin/main..HEAD`**:
  ```text
  1ab431b1 feat(object): Resize to fit, and Paste to replace keeps only its own shortcut (#1470)
  ```

Status: DONE

---

## Round 2

### 1. What Could NOT Be Verified

- **Live GUI window & native macOS menu bar key-equivalent interception**: Not run; verification ran headless through `App`, `Sim`, and `designos_render::render_page` (`macos_menu.rs` installs only `Quit` at the AppKit menu layer).
- **Proprietary Figma desktop app oracle for F2 and I1**: Not opened per the worker boundary ("Never open Figma or any client file"). Both behaviours are pinned by controller rulings.

### 2. Conclusion & Rulings Implemented

- **F1 (`FACT`)**: Deleted `Document::resize_to_fit` and `snapshot_subtree` from [`crates/designos-doc/src/layout_constraints.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-doc/src/layout_constraints.rs), keeping `is_resize_to_fit_kind`, `visible_children_bounds`, `can_resize_to_fit`, `ordered_resize_to_fit_targets`, and `apply_resize_to_fit_one` in `designos-doc`. Ported all 9 acceptance tests from `layout_constraints::tests` to [`crates/designos-app/src/ops.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-app/src/ops.rs) driving `App::resize_to_fit` (and `App::can_resize_to_fit` / `App::undo`) with geometry assertions unchanged.
- **F2 (`FACT`, accepted ruling)**: A frame that is a child of an auto layout parent is placed by that layout, so after Resize to fit the parent re-flows and the frame's content moves on the page, as for any other size change of a flow child. Pinned by `ops::tests::resize_to_fit_frame_inside_auto_layout_parent_reflows_parent_and_moves_child_on_page` (`f` shrinks from `200×200` to `60×60` at `(70, 10)`, parent `p` shrinks from `280×220` to `140×80`, and child `c` moves on the page from `(110, 50)` to `(70, 10)` with exact undo and redo).
- **F4 (`FACT`)**: Added three `App` tests in [`crates/designos-app/src/ops.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-app/src/ops.rs):
  1. `resize_to_fit_nested_selection_fits_deepest_container_before_outer_container` (kills mutant `M6`: selecting `[o, n]` fits `n` to `20×20` first and then `o` to `50×50` at `(60, 60)`).
  2. `resize_to_fit_frame_inside_group_refits_enclosing_group` (kills mutant `M10`: fitting a frame inside a `Group` triggers `relayout_up` and re-fits the `Group` from `300×300` at `(10, 20)` to `80×60` at `(50, 70)`).
  3. `resize_to_fit_fill_width_frame_in_auto_layout_parent_becomes_fixed_and_keeps_fitted_size` (kills mutant `M11`: fitting a `Fill`-sized non-auto-layout frame inside an auto-layout parent sets `layout_child` to `(Sizing::Fixed, Sizing::Fixed)` and preserves the fitted `80×50` size after `relayout_subtree`).
- **F6 (`FACT`)**: Updated [`scripts/figma_parity_meter.py:425`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/scripts/figma_parity_meter.py#L425) to `"Paste to replace (Cmd+Shift+R)"`.
- **F7 (`FACT`)**: Renamed `DESIGNOS_1470_EVIDENCE_OUT` and `1470-{before,after}.png` in [`crates/designos-app/src/ops.rs:3103-3108`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1470-resize-to-fit/crates/designos-app/src/ops.rs#L3103-L3108) to `DESIGNOS_RESIZE_TO_FIT_EVIDENCE_OUT` and `resize-to-fit-{before,after}.png`.
- **Ruling 6 (`FACT`)**: Left F3 (group refit counting hidden children — controller follow-up issue), F5 (keep builder report under `plans/reports/`), F8 (Line-only frame `120×0`), and I1 (main component fit moves instance content, as in Figma) unchanged per controller ruling.

### 3. Acceptance Table (Round 2)

| Acceptance Item | Status | Evidence / Output |
|---|---|---|
| 1a. 9 ported tests + 4 new tests green on head (`17` total `resize_to_fit` tests in `designos-app`) | PASS | `test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 4037 filtered out` (`plans/reports/agy-1470-resize-to-fit-evidence/r2-gates.log`) |
| 1b. Mutants `M0-null` (control), `M6`, `M10`, `M11`, and `M14-equiv` (remove relayout from shipped `App::resize_to_fit`) | PASS | `SUMMARY [('M0-null', 'SURVIVED'), ('M6', 'KILLED'), ('M10', 'KILLED'), ('M11', 'KILLED'), ('M14-equiv', 'KILLED')]` (`plans/reports/agy-1470-resize-to-fit-evidence/r2-mutants.log`) |
| 2. `git grep -n 'fn resize_to_fit' crates/designos-doc` | PASS | No match (`exit=1`) |
| 3a. `cargo fmt --all -- --check` | PASS | `exit=0` |
| 3b. `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | Only `main`'s 2 known `clippy::needless_return` lints at `crates/designos-app/src/agent_permissions.rs:1176,1194`; `clippy -p designos-doc -p designos-app --all-targets --locked -- -D warnings -A clippy::needless_return` -> `exit=0` |
| 3c. `cargo test -p designos-doc --locked --lib` | PASS | `test result: ok. 708 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.09s` |
| 3d. `cargo test -p designos-app --bin designos --locked resize_to_fit` | PASS | `test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 11.18s` |
| 3e. `cargo test --workspace --locked --lib` | PASS | All 12 library crates `ok` (`13 + 708 + 1 + 153 + 143 + 30 + 4 + 37 + 322 + 56 + 1 + 8 = 1,476 passed; 0 failed; 3 ignored`) |
| 3f. `python3 scripts/zero_fake_ui_gate.py` | PASS | `Gate Verdict: PASS (17/17 invariants satisfied)` |

### 4. Skills Loaded & What Changed Because of Them

- **`es:dev` / `es:feature-dev`**: Followed the DEV-phase fix loop directly from the controller's rulings without re-opening brainstorm/plan gates.
- **`es:rust`**: Removed dead `BTreeMap` subtree-cloning history plumbing (`snapshot_subtree` and `Document::resize_to_fit`) from `designos-doc`, consolidating undo/redo recording onto `App::resize_to_fit`'s existing `Tx` (`Edit::touch` + `run_tx`) path.
- **`es:rust-gpu-canvas`**: Preserved local-space transform composition (`n.transform * Affine::translate(bb.x, bb.y)`) and geometry cache invalidation (`fill_geometry = None`, `stroke_geometry = None`) in `apply_resize_to_fit_one`.
- **`es:astra-work` & `es:fable-thinking`**: Verified every mutant against the shipped `App::resize_to_fit` path after running a null control (`M0-null`) first, and maintained the epistemic ledger below.

### 5. Epistemic Ledger, Assumptions & Weakest Link

- **`FACT`**: `git grep -n 'fn resize_to_fit' crates/designos-doc` returns no match (`exit=1`); `App::resize_to_fit` in `crates/designos-app/src/ops.rs:1169` is the sole `resize_to_fit` entry point.
- **`FACT`**: In `ops::tests::resize_to_fit_frame_inside_auto_layout_parent_reflows_parent_and_moves_child_on_page`, fitting a `200×200` non-auto-layout frame inside a `Hug` horizontal auto-layout parent shrinks the frame to `60×60` at `(70, 10)`, re-flows the parent from `280×220` to `140×80`, and moves the child on the page from `(110, 50)` to `(70, 10)`, with `app.undo()` and `app.redo()` restoring the exact pre-fit and post-fit document JSON.
- **`FACT`**: Null control `M0-null` passes (`17 passed; 0 failed`), and mutants `M6`, `M10`, `M11`, and `M14-equiv` are all killed by `cargo test -p designos-app --bin designos --locked resize_to_fit`.
- **`INFERENCE`**: Follow-up finding F3 (`layout::fit_group` counting hidden children on subsequent edits inside a group) remains tracked for the controller's separate issue and does not affect `App::resize_to_fit`'s own bounds calculation.
- **`ASSUMPTION`**: None.
- **Weakest link**: `layout::fit_group` still includes hidden children when a subsequent edit inside a Group triggers `relayout_up` (deferred to follow-up per Ruling 6 / F3).
- **Unresolved questions**: None.

### 6. Exact Commands Run & Pasted Output

```text
$ git grep -n 'fn resize_to_fit' crates/designos-doc
(no match, exit=1)

$ cargo fmt --all -- --check
(exit=0)

$ env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1176:9
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1194:9
error: could not compile `designos-app` (bin "designos-studio") due to 2 previous errors
error: could not compile `designos-app` (bin "designos") due to 2 previous errors
error: could not compile `designos-app` (bin "designos-studio" test) due to 2 previous errors
error: could not compile `designos-app` (bin "designos" test) due to 2 previous errors

$ env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy -p designos-doc -p designos-app --all-targets --locked -- -D warnings -A clippy::needless_return
(exit=0)

$ env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-doc --locked --lib
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-6a14e6c7eed30945)
running 708 tests
test result: ok. 708 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.09s

$ env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" DESIGNOS_RESIZE_TO_FIT_EVIDENCE_OUT="$PWD/out/evidence" cargo test -p designos-app --bin designos --locked resize_to_fit
     Running unittests src/main.rs (out/cargo/debug/deps/designos-0146e1a2c129c956)
running 17 tests
test ops::tests::resize_to_fit_frame_inside_auto_layout_parent_reflows_parent_and_moves_child_on_page ... ok
test ops::tests::resize_to_fit_ignores_hidden_children_and_disables_when_none_visible ... ok
test ops::tests::resize_to_fit_nested_selection_fits_deepest_container_before_outer_container ... ok
test ops::tests::resize_to_fit_does_not_apply_child_constraints ... ok
test ops::tests::app_resize_to_fit_resizes_containers_skips_leaf_layers_and_undoes_in_one_step ... ok
test ops::tests::resize_to_fit_one_undo_restores_entire_selection_and_skips_shapes_and_text ... ok
test ops::tests::resize_to_fit_frame_smaller_than_children_grows_and_keeps_child_page_positions ... ok
test ops::tests::resize_to_fit_rotated_child_and_rotated_container_use_container_space ... ok
test ops::tests::resize_to_fit_frame_larger_than_children_shrinks_and_keeps_child_page_positions ... ok
test ops::tests::resize_to_fit_fill_width_frame_in_auto_layout_parent_becomes_fixed_and_keeps_fitted_size ... ok
test ops::tests::resize_to_fit_group_fits_visible_children ... ok
test ops::tests::resize_to_fit_auto_layout_frame_turns_hug_on_both_axes ... ok
test panels::menus::tests::resize_to_fit_menu_and_paste_to_replace_shortcut_labels_and_disabled_state ... ok
test ops::tests::resize_to_fit_component_and_component_set_fit_visible_children ... ok
test ops::tests::resize_to_fit_frame_inside_group_refits_enclosing_group ... ok
test ops::tests::resize_to_fit_cpu_render_preserves_child_pixels_and_trims_excess_frame_bounds ... ok
test shell::tests::agent_a11y::shortcut_cmd_alt_shift_r_maps_to_resize_to_fit_and_cmd_shift_r_keeps_paste_to_replace ... ok
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 11.18s

$ env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-97b5db69fdcd9c68)
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-18f4500351079eb1)
running 708 tests
test result: ok. 708 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.82s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-c2489b38bbcccff9)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-18adcc4017a7a7af)
running 154 tests
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-861945a6becd4004)
running 143 tests
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.65s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-c2d586a1e5c886ba)
running 30 tests
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-77666ce24b473797)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-7534529c79ad779d)
running 37 tests
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 25.05s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-db051a2df27fbe42)
running 322 tests
test result: ok. 322 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.65s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-59facfce9e95a574)
running 58 tests
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 34.26s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-dd3732f70a143b29)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-2fa66b1349242d0a)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s

$ cat plans/reports/agy-1470-resize-to-fit-evidence/r2-mutants.log
## M0-null SURVIVED (77s) :: null control (head as-is)
exit=0 test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 5.65s failed=[]

## M6 KILLED (83s) :: targets shallowest-first
exit=101 test result: FAILED. 16 passed; 1 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 1.12s failed=['ops::tests::resize_to_fit_nested_selection_fits_deepest_container_before_outer_container']

## M10 KILLED (96s) :: non-AL target: no ancestor relayout
exit=101 test result: FAILED. 15 passed; 2 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 0.98s failed=['ops::tests::resize_to_fit_frame_inside_auto_layout_parent_reflows_parent_and_moves_child_on_page', 'ops::tests::resize_to_fit_frame_inside_group_refits_enclosing_group']

## M11 KILLED (32s) :: non-AL target keeps Fill/Hug layout sizing
exit=101 test result: FAILED. 16 passed; 1 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 0.80s failed=['ops::tests::resize_to_fit_fill_width_frame_in_auto_layout_parent_becomes_fixed_and_keeps_fitted_size']

## M14-equiv KILLED (23s) :: shipped App::resize_to_fit: drop relayout from shipped path
exit=101 test result: FAILED. 12 passed; 5 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 0.76s failed=['ops::tests::resize_to_fit_auto_layout_frame_turns_hug_on_both_axes', 'ops::tests::resize_to_fit_one_undo_restores_entire_selection_and_skips_shapes_and_text', 'ops::tests::app_resize_to_fit_resizes_containers_skips_leaf_layers_and_undoes_in_one_step', 'ops::tests::resize_to_fit_frame_inside_group_refits_enclosing_group', 'ops::tests::resize_to_fit_frame_inside_auto_layout_parent_reflows_parent_and_moves_child_on_page']

SUMMARY [('M0-null', 'SURVIVED'), ('M6', 'KILLED'), ('M10', 'KILLED'), ('M11', 'KILLED'), ('M14-equiv', 'KILLED')]

$ git log --oneline origin/main..HEAD
80af401f refactor(object): remove unused Document::resize_to_fit and port tests to App (#1470)
b5a7ad6b Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1470-resize-to-fit
0bfca899 docs(reports): record PR #1488 in 1470 report
1ab431b1 feat(object): Resize to fit, and Paste to replace keeps only its own shortcut (#1470)
```

Status: DONE
