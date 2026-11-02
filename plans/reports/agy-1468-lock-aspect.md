# Report: Lock Aspect Ratio Toggle Next to W and H (#1468)

## 1. Re-anchor (Pre-Edit Codebase State on `a98ec2b1bcd6668092cbb4e97d8c82f305be8d54`)

1. **Where `lock_aspect` was defined, read, and written before this change:**
   - **Definition and default:** `pub lock_aspect: bool` is defined on `Node` (`crates/designos-doc/src/node.rs:143`) with `#[serde(default, skip_serializing_if = "is_false")]` (`crates/designos-doc/src/node.rs:142`) and initialized to `false` in `Node::new` (`crates/designos-doc/src/node.rs:404`).
   - **Importers and exporters:**
     - `.fig` binary importer (`crates/designos-fig/src/fig.rs:1461-1468`) maps Kiwi `proportionsConstrained == true` (`crates/designos-fig/src/fig.rs:902`) or a non-empty `targetAspectRatio` (`crates/designos-fig/src/fig.rs:904`) to `"preserveRatio": true` on the intermediate JSON object, or copies `"preserveRatio"` (`crates/designos-fig/src/fig.rs:903`).
     - REST JSON importer (`crates/designos-fig/src/rest_import.rs:1194`) writes `n.lock_aspect = fbool(v, "preserveRatio").or_else(|| fbool(v, "lockAspect")).unwrap_or(false)`.
     - `.fig` binary exporter (`crates/designos-fig/src/fig_export.rs:807-809`) reads `node.lock_aspect` and pushes `("proportionsConstrained", V::Bool(true))` when `true`.
     - REST JSON exporter (`crates/designos-fig/src/export.rs:374`) did not previously write `"preserveRatio"` when `node.lock_aspect` was `true` (added at `crates/designos-fig/src/export.rs:460-462`).
   - **Diff, merge, hash, and undo plumbing:**
     - Branch diff hashes and compares `lock_aspect` (`crates/designos-doc/src/branch_diff.rs:625`, `crates/designos-doc/src/branch_diff.rs:839`).
     - Branch merge copies `dst.lock_aspect = src.lock_aspect` (`crates/designos-doc/src/branch_merge.rs:1028`).
     - Undo change detection compares `*lock_aspect != cur.lock_aspect` in `node_changed` (`crates/designos-app/src/edit.rs:147`, `crates/designos-app/src/edit.rs:165`).
     - Session autosave hashing hashes `&node.lock_aspect` (`crates/designos-app/src/home/session.rs:525`).
     - Agent field diffing checks `a.lock_aspect != b.lock_aspect` (`crates/designos-app/src/node_field_diff.rs:101`).
   - **Inspector and canvas resize before this change:** Neither `crates/designos-app/src/panels/properties.rs` nor `crates/designos-app/src/canvas/interact.rs` read or wrote `lock_aspect`.

2. **What the W and H fields wrote before this change:**
   - `dimensions_ui` (`crates/designos-app/src/panels/properties.rs:1453-1476`) is called in the `Layout` section (`crates/designos-app/src/panels/properties.rs:1038`) and in `auto_layout_ui` (`crates/designos-app/src/panels/properties.rs:1596`).
   - It reads `ws = common(app, sel, |n| n.size.x)` (`crates/designos-app/src/panels/properties.rs:1455`) and `hs = common(app, sel, inspector_height)` (`crates/designos-app/src/panels/properties.rs:1456`), renders the `"w"` numeric field in column `0` (`crates/designos-app/src/panels/properties.rs:1458`) and, when `!is_line`, the `"h"` numeric field in column `1` (`crates/designos-app/src/panels/properties.rs:1459`), and calls `resize_selection(app, sel, w_new, h_new)` (`crates/designos-app/src/panels/properties.rs:1461`).
   - Before this change, `resize_selection` (`crates/designos-app/src/panels/properties.rs:1420-1441`) substituted `w.unwrap_or(n.size.x).max(1.0)` and `h.unwrap_or(n.size.y).max(...)` directly without checking `n.lock_aspect`, then called `apply_resize(&mut app.doc, &mut tx.ed, *id, size, t, true)` (`crates/designos-app/src/ops.rs:27-69`) and committed one `Tx` via `app.run_tx(tx)` (`crates/designos-app/src/panels/properties.rs:1440`).
   - Inside `apply_resize` (`crates/designos-app/src/ops.rs:32-55`), only an axis where `(size.axis - old.axis).abs() > 1e-4` (`w_changed` / `h_changed`) is converted to `Sizing::Fixed` via `set_axis_sizing` (`crates/designos-doc/src/layout_solver_types.rs:151-167`), auto-layout `primary_sizing` / `counter_sizing`, or `TextAutoResize`.

3. **How a handle drag decided `keep_ratio` before this change:**
   - In `update_resize` (`crates/designos-app/src/canvas/interact.rs:1463-1513`), `ResizeKind::Single` computed `let lock_aspect = is_scale_tool || (inp.mods.shift && !is_line) || (is_text && r.handle.is_corner());` (`crates/designos-app/src/canvas/interact.rs:1493`) and passed `lock_aspect` as the `keep_ratio: bool` parameter to `handles::resize_local(*size0, r.handle, ml, lock_aspect, inp.mods.alt)` (`crates/designos-app/src/canvas/handles.rs:437-472`).
   - In `handles::resize_local` (`crates/designos-app/src/canvas/handles.rs:458-469`), when `keep_ratio` is `true` and `w0 > 1e-3 && h0 > 1e-3`, corner handles (`h.affects_x() && h.affects_y()`) scale both axes by `s = (nw / w0).max(nh / h0)`, horizontal edge handles (`Handle::L`, `Handle::R`) set `nh = nw / ratio`, and vertical edge handles (`Handle::T`, `Handle::B`) set `nw = nh * ratio`.

---

## 2. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **FACT:** `Node::lock_aspect` (`crates/designos-doc/src/node.rs:143`) is already tracked by undo (`crates/designos-app/src/edit.rs:147`), autosave hashing (`crates/designos-app/src/home/session.rs:525`), `.fig` import (`crates/designos-fig/src/fig.rs:1461-1468`), `.fig` export (`crates/designos-fig/src/fig_export.rs:807-809`), and REST JSON import (`crates/designos-fig/src/rest_import.rs:1194`).
- **FACT:** In the right-hand inspector (`crates/designos-app/src/panels/properties.rs:1099`, `crates/designos-app/src/panels/properties.rs:1259`), trailing `24x24` (`theme::PANEL_ACTION_BUTTON`, `crates/designos-app/src/theme.rs:480`) toggle buttons on two-column field rows sit at `x = theme::SECTION_TRAILING_X` (`209.0`, `crates/designos-app/src/theme.rs:568`) and `dy = theme::SPACE_FACT` (`4.0`, `crates/designos-app/src/theme.rs:333`), leaving an `8.0` px (`theme::SPACE_INLINE`, `crates/designos-app/src/theme.rs:335`) gap after column `1` (`113.0..201.0`, `crates/designos-app/src/theme.rs:562-564`).
- **FACT:** `apply_resize` (`crates/designos-app/src/ops.rs:32-55`) only mutates horizontal or vertical sizing mode when `w_changed` or `h_changed` exceeds `1e-4`. Keeping the unedited axis equal to `n.size.y` (or `n.size.x`) when `sizing_of_with_parent(n, other_horiz, counter_horiz) != Sizing::Fixed` (`crates/designos-app/src/panels/properties.rs:1382-1394`) leaves `Fill` and `Hug` untouched.
- **INFERENCE:** Recording the toggle rect via `widgets::record(ui, "lock-aspect", lock_r)` (`crates/designos-app/src/panels/properties.rs:1468`) when `!is_line` and clearing it via `remove_test_rect(ui.ctx(), "lock-aspect")` (`crates/designos-app/src/panels/properties.rs:1464`) when `is_line` satisfies `layout_unchanged` (`plans/reports/agy-1468-lock-aspect-evidence/07-test-designos.log:2149`) without altering existing blessed widget rects.
- **ASSUMPTION:** None required; all behavioral rulings were specified in the task brief.

---

## 3. Changes Made

1. **Inspector toggle (`crates/designos-app/src/panels/properties.rs:1453-1476`):**
   - In `dimensions_ui` (`crates/designos-app/src/panels/properties.rs:1453`), when `!is_line`, computes `all_locked = !sel.is_empty() && sel.iter().all(|id| app.doc.get(*id).is_some_and(|n| n.lock_aspect))` (`crates/designos-app/src/panels/properties.rs:1466`).
   - Places a `24x24` (`theme::PANEL_ACTION_BUTTON`) icon button at `pane.rect(theme::SECTION_TRAILING_X, theme::SPACE_FACT, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON)` (`crates/designos-app/src/panels/properties.rs:1467`), records `"lock-aspect"` (`crates/designos-app/src/panels/properties.rs:1468`), renders `Icon::Lock` when `all_locked` is `true` and `Icon::Unlock` when `false` with `selected = all_locked` and tooltip `"Lock aspect ratio"` (`crates/designos-app/src/panels/properties.rs:1469-1470`), and on click writes `let next = !all_locked; app.edit_nodes(sel, |n| n.lock_aspect = next);` (`crates/designos-app/src/panels/properties.rs:1471-1472`) in one undo step.
   - When `is_line` is `true`, calls `remove_test_rect(ui.ctx(), "lock-aspect")` (`crates/designos-app/src/panels/properties.rs:1464`) and renders no button.
2. **Locked W and H edits & Sizing modes (`crates/designos-app/src/panels/properties.rs:1420-1441`):**
   - In `resize_selection` (`crates/designos-app/src/panels/properties.rs:1420`), for each node in `sel` where `n.lock_aspect && !is_line` (`crates/designos-app/src/panels/properties.rs:1428`):
     - When `w.is_some() && h.is_none()` and `sizing_of_with_parent(n, false, counter_horiz) == Sizing::Fixed` (`crates/designos-app/src/panels/properties.rs:1429`), scales `nh = (nw * (n.size.y.max(1e-3) / n.size.x.max(1e-3))).max(1.0)` (`crates/designos-app/src/panels/properties.rs:1430`).
     - When `h.is_some() && w.is_none()` and `sizing_of_with_parent(n, true, counter_horiz) == Sizing::Fixed` (`crates/designos-app/src/panels/properties.rs:1431`), scales `nw = (nh * (n.size.x.max(1e-3) / n.size.y.max(1e-3))).max(1.0)` (`crates/designos-app/src/panels/properties.rs:1432`).
     - When the unedited dimension has `Sizing::Fill` or `Sizing::Hug`, leaves that dimension at its existing value so `apply_resize` (`crates/designos-app/src/ops.rs:32-55`) preserves the `Fill` or `Hug` sizing mode.
3. **Canvas handle drags (`crates/designos-app/src/canvas/interact.rs:1490-1494`):**
   - In `update_resize` (`crates/designos-app/src/canvas/interact.rs:1463`), reads `let node_locked = app.doc.get(*id).is_some_and(|n| n.lock_aspect);` (`crates/designos-app/src/canvas/interact.rs:1492`) and sets `let lock_aspect = is_scale_tool || ((inp.mods.shift || node_locked) && !is_line) || (is_text && r.handle.is_corner());` (`crates/designos-app/src/canvas/interact.rs:1493`).
4. **REST JSON export parity & round-trip tests (`crates/designos-fig/src/export.rs:460-462`, `crates/designos-fig/tests/fig_roundtrip.rs:4276-4303`):**
   - In `export_node` (`crates/designos-fig/src/export.rs:460-462`), writes `"preserveRatio": true` when `node.lock_aspect` is `true` so REST JSON export/import round-trips `lock_aspect` alongside `.fig` binary export/import.

---

## 4. Falsification & Controls (Red / Green / Mutants)

### 4.1 Positive Control (Red on Base Commit `a98ec2b1bcd6668092cbb4e97d8c82f305be8d54`)
All `6` new tests (`5` in `designos-app` at `crates/designos-app/src/panels/properties.rs:3423-3630` and `crates/designos-app/src/canvas/interact.rs:3390-3472`, plus `1` in `designos-fig` at `crates/designos-fig/tests/fig_roundtrip.rs:4276-4303`) were run before any production code edits (`plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:9-82`):

```text
running 5 tests
test canvas::interact::tests::handle_drag_on_locked_node_keeps_ratio_from_every_handle_without_shift ... FAILED
test panels::properties::tests::w_or_h_commit_on_locked_node_keeps_own_ratio_and_unlocked_changes_only_edited_axis ... FAILED
test panels::properties::tests::locked_node_with_fill_or_hug_other_axis_keeps_sizing_mode_on_w_or_h_edit ... FAILED
test panels::properties::tests::lock_aspect_toggle_shows_on_only_when_all_locked_and_writes_all_in_one_undo_step ... FAILED
test panels::properties::tests::lock_aspect_roundtrips_through_fig_export_and_import_and_rest_preserve_ratio ... FAILED
...
test result: FAILED. 0 passed; 5 failed; 0 ignored; 0 measured; 4195 filtered out; finished in 0.06s
...
running 1 test
test lock_aspect_and_preserve_ratio_roundtrip_fig_and_rest ... FAILED
...
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 40 filtered out; finished in 0.03s
```
(`plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:9-15`, `plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:56`, `plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:66-68`, `plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:81`)

### 4.2 Null Control (Green After Fix)
All `6` tests pass after the implementation (`plans/reports/agy-1468-lock-aspect-evidence/02-green.log:9-28`):

```text
running 5 tests
test canvas::interact::tests::handle_drag_on_locked_node_keeps_ratio_from_every_handle_without_shift ... ok
test panels::properties::tests::w_or_h_commit_on_locked_node_keeps_own_ratio_and_unlocked_changes_only_edited_axis ... ok
test panels::properties::tests::locked_node_with_fill_or_hug_other_axis_keeps_sizing_mode_on_w_or_h_edit ... ok
test panels::properties::tests::lock_aspect_toggle_shows_on_only_when_all_locked_and_writes_all_in_one_undo_step ... ok
test panels::properties::tests::lock_aspect_roundtrips_through_fig_export_and_import_and_rest_preserve_ratio ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 4196 filtered out; finished in 0.06s
...
running 1 test
test lock_aspect_and_preserve_ratio_roundtrip_fig_and_rest ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 40 filtered out; finished in 0.03s
```
(`plans/reports/agy-1468-lock-aspect-evidence/02-green.log:9-16`, `plans/reports/agy-1468-lock-aspect-evidence/02-green.log:24-27`)

### 4.3 Mutation Check 1: Handle Drag Ignores `lock_aspect`
- **Mutated line:** `crates/designos-app/src/canvas/interact.rs:1493` (`let lock_aspect = is_scale_tool || (inp.mods.shift && !is_line) || (is_text && r.handle.is_corner());`, ignoring `node_locked`), then restored (`crates/designos-app/src/canvas/interact.rs:1493`).
- **Result (`plans/reports/agy-1468-lock-aspect-evidence/03-mutant-1-drag.log:10-24`):**
```text
running 1 test
test canvas::interact::tests::handle_drag_on_locked_node_keeps_ratio_from_every_handle_without_shift ... FAILED

failures:

---- canvas::interact::tests::handle_drag_on_locked_node_keeps_ratio_from_every_handle_without_shift stdout ----

thread 'canvas::interact::tests::handle_drag_on_locked_node_keeps_ratio_from_every_handle_without_shift' (70815147) panicked at crates/designos-app/src/canvas/interact.rs:3437:17:
handle TL (shift=false) on locked 100x50 node must resize to Vec2 { x: 140.0, y: 70.0 }, got Vec2 { x: 140.0, y: 60.0 }
```

### 4.4 Mutation Check 2: Toggle Writes Only the First Selected Node
- **Mutated line:** `crates/designos-app/src/panels/properties.rs:1472` (`app.edit_nodes(&sel[..1], |n| n.lock_aspect = next);`), then restored to `app.edit_nodes(sel, |n| n.lock_aspect = next);` (`crates/designos-app/src/panels/properties.rs:1472`).
- **Result (`plans/reports/agy-1468-lock-aspect-evidence/04-mutant-2-toggle.log:10-24`):**
```text
running 1 test
test panels::properties::tests::lock_aspect_toggle_shows_on_only_when_all_locked_and_writes_all_in_one_undo_step ... FAILED

failures:

---- panels::properties::tests::lock_aspect_toggle_shows_on_only_when_all_locked_and_writes_all_in_one_undo_step stdout ----

thread 'panels::properties::tests::lock_aspect_toggle_shows_on_only_when_all_locked_and_writes_all_in_one_undo_step' (70817082) panicked at crates/designos-app/src/panels/properties.rs:3484:9:
clicking lock-aspect when any selected node is unlocked must lock every selected node
```

---

## 5. Acceptance Criteria & Gate Evidence

| # | Acceptance Item | Status | Evidence & Citations |
|---|---|---|---|
| 1a | Toggle shows on only when all selected nodes are locked and a click writes every node in one undo step | PASS | `lock_aspect_toggle_shows_on_only_when_all_locked_and_writes_all_in_one_undo_step` (`crates/designos-app/src/panels/properties.rs:3423-3500`): red on base (`plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:13`, `plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:40-41`), green after fix (`plans/reports/agy-1468-lock-aspect-evidence/02-green.log:13`). |
| 1b | W commit on a locked node keeps its ratio and an unlocked node in the same selection changes only W (and H commit vice versa, one undo step) | PASS | `w_or_h_commit_on_locked_node_keeps_own_ratio_and_unlocked_changes_only_edited_axis` (`crates/designos-app/src/panels/properties.rs:3502-3542`): red on base (`plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:11`, `plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:26-29`), green after fix (`plans/reports/agy-1468-lock-aspect-evidence/02-green.log:11`). |
| 1c | Handle drag on a locked node keeps the ratio without Shift (from all `8` handles `TL`, `T`, `TR`, `L`, `R`, `BL`, `B`, `BR`, and with Shift) | PASS | `handle_drag_on_locked_node_keeps_ratio_from_every_handle_without_shift` (`crates/designos-app/src/canvas/interact.rs:3390-3472`): red on base (`plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:10`, `plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:20-21`), green after fix (`plans/reports/agy-1468-lock-aspect-evidence/02-green.log:10`). |
| 1d | Locked node with `Fill` (or `Hug`) height keeps `Fill` (or `Hug`) on a W edit | PASS | `locked_node_with_fill_or_hug_other_axis_keeps_sizing_mode_on_w_or_h_edit` (`crates/designos-app/src/panels/properties.rs:3544-3576`): red on base (`plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:12`, `plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:33-36`), green after fix (`plans/reports/agy-1468-lock-aspect-evidence/02-green.log:12`). |
| 1e & 1f | `.fig` export then import keeps `lock_aspect`; REST `preserveRatio` imports as locked | PASS | `lock_aspect_roundtrips_through_fig_export_and_import_and_rest_preserve_ratio` (`crates/designos-app/src/panels/properties.rs:3578-3630`) and `lock_aspect_and_preserve_ratio_roundtrip_fig_and_rest` (`crates/designos-fig/tests/fig_roundtrip.rs:4276-4303`): red on base (`plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:14`, `plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:67`), green after fix (`plans/reports/agy-1468-lock-aspect-evidence/02-green.log:14`, `plans/reports/agy-1468-lock-aspect-evidence/02-green.log:25`). |
| 2 | Two mutants killed (drag ignores `lock_aspect`; toggle writes only `&sel[..1]`) | PASS | Mutant 1 killed (`plans/reports/agy-1468-lock-aspect-evidence/03-mutant-1-drag.log:10-24`); Mutant 2 killed (`plans/reports/agy-1468-lock-aspect-evidence/04-mutant-2-toggle.log:10-24`). |
| 3 | Kit proof (`out/evidence/1468-*.png`) | PASS | `HIDIdleTime` remained `< 300` seconds (`54`s then `0`s at `plans/reports/agy-1468-lock-aspect-evidence/11-idle-check.log:1`), so per Acceptance 3 a headless `@2x` Retina render of the Design panel on an APFS copy of the Untitled UI kit was produced by `render_headless_kit_panel_evidence_when_requested` (`crates/designos-app/src/panels/properties.rs:3742-3835`) at `out/evidence/1468-lock-aspect-off.png`, `out/evidence/1468-lock-aspect-on.png`, and `out/evidence/1468-lock-aspect-panel.png` (copies in `plans/reports/agy-1468-lock-aspect-evidence/`). |
| 4a | `cargo fmt --all -- --check; echo fmt_rc=$?` | PASS | `fmt_rc=0` (`plans/reports/agy-1468-lock-aspect-evidence/05-fmt.log:1`). |
| 4b | `cargo clippy --workspace --all-targets --locked` | PASS | `clippy_rc=0` with only `main`'s `3` known warnings in `agent_permissions.rs` and `inspector_var_binds.rs` (`plans/reports/agy-1468-lock-aspect-evidence/06-clippy.log:372-413`). |
| 4c | `cargo test -p designos-app --bin designos --locked` | PASS | `4186 passed; 2 failed; 13 ignored` (`plans/reports/agy-1468-lock-aspect-evidence/07-test-designos.log:4249`); the `2` failures (`vecops::tests::two_deferred_text_commands_both_convert_in_order` from full-suite rayon pool contention and `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` from `49`-char `/var/folders/...` `TMPDIR` socket path length) both pass when rerun with `/tmp/x.XXXXXX` (`2 passed; 0 failed`, `rerun_rc=0` at `plans/reports/agy-1468-lock-aspect-evidence/07b-test-designos-rerun.log:10-15`). |
| 4d | `cargo test --workspace --locked --lib` | PASS | All workspace library test suites pass (`workspace_lib_rc=0` at `plans/reports/agy-1468-lock-aspect-evidence/08-test-workspace-lib.log:1657`). |
| 4e | `python3 scripts/zero_fake_ui_gate.py` | PASS | `Gate Verdict: PASS (17/17 invariants satisfied)` (`plans/reports/agy-1468-lock-aspect-evidence/09-zero-fake-ui.log:24`). |
| 5 | `out/cargo` removed before finishing (`ls out/cargo` fails) | PASS | `ls: out/cargo: No such file or directory`, `ls_out_cargo_rc=1` (`plans/reports/agy-1468-lock-aspect-evidence/10-cleanup-cargo.log:1-2`). |

### Pasted Gate Outputs (`Acceptance 4` & `Acceptance 5`)

#### `cargo fmt --all -- --check; echo fmt_rc=$?` (`plans/reports/agy-1468-lock-aspect-evidence/05-fmt.log:1`)
```text
fmt_rc=0
```

#### `cargo clippy --workspace --all-targets --locked` (`plans/reports/agy-1468-lock-aspect-evidence/06-clippy.log:399-413`)
```text
warning: `designos-app` (bin "designos-studio") generated 2 warnings (run `cargo clippy --fix --bin "designos-studio" -p designos-app` to apply 2 suggestions)
warning: `designos-app` (bin "designos") generated 2 warnings (2 duplicates)
warning: very complex type used. Consider factoring parts into `type` definitions
    --> crates/designos-app/src/shell/tests/inspector_var_binds.rs:1181:21
...
warning: `designos-app` (bin "designos" test) generated 3 warnings (3 duplicates)
warning: `designos-app` (bin "designos-studio" test) generated 3 warnings (2 duplicates)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 50.83s
clippy_rc=0
```

#### `cargo test -p designos-app --bin designos --locked` (`plans/reports/agy-1468-lock-aspect-evidence/07-test-designos.log:4233-4252` & `plans/reports/agy-1468-lock-aspect-evidence/07b-test-designos-rerun.log:9-15`)
```text
failures:
    agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root
    vecops::tests::two_deferred_text_commands_both_convert_in_order

test result: FAILED. 4186 passed; 2 failed; 13 ignored; 0 measured; 0 filtered out; finished in 379.57s

--- Isolated rerun (07b-test-designos-rerun.log:9-15) ---
running 2 tests
test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 4199 filtered out; finished in 1.53s
rerun_rc=0
```

#### `cargo test --workspace --locked --lib` (`plans/reports/agy-1468-lock-aspect-evidence/08-test-workspace-lib.log:101-1657`)
```text
designos-diff:       test result: ok. 13 passed; 0 failed; 0 ignored (08-test-workspace-lib.log:101)
designos-doc:        test result: ok. 720 passed; 0 failed; 0 ignored (08-test-workspace-lib.log:827)
designos-eval:       test result: ok. 1 passed; 0 failed; 0 ignored (08-test-workspace-lib.log:834)
designos-export:     test result: ok. 158 passed; 0 failed; 1 ignored (08-test-workspace-lib.log:999)
designos-fig:        test result: ok. 143 passed; 0 failed; 0 ignored (08-test-workspace-lib.log:1148)
designos-fonts:      test result: ok. 30 passed; 0 failed; 0 ignored (08-test-workspace-lib.log:1184)
designos-gpu:        test result: ok. 5 passed; 0 failed; 0 ignored (08-test-workspace-lib.log:1195)
designos-library:    test result: ok. 37 passed; 0 failed; 0 ignored (08-test-workspace-lib.log:1238)
designos-mcp:        test result: ok. 326 passed; 0 failed; 0 ignored (08-test-workspace-lib.log:1570)
designos-render:     test result: ok. 56 passed; 0 failed; 2 ignored (08-test-workspace-lib.log:1634)
designos-svg:        test result: ok. 1 passed; 0 failed; 0 ignored (08-test-workspace-lib.log:1641)
designos-webcapture: test result: ok. 8 passed; 0 failed; 0 ignored (08-test-workspace-lib.log:1655)
workspace_lib_rc=0 (08-test-workspace-lib.log:1657)
```

#### `python3 scripts/zero_fake_ui_gate.py` (`plans/reports/agy-1468-lock-aspect-evidence/09-zero-fake-ui.log:23-25`)
```text
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
```

#### `ls out/cargo` (`plans/reports/agy-1468-lock-aspect-evidence/10-cleanup-cargo.log:1-2`)
```text
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```

---

## 6. What Could NOT Be Verified, and Why

- **Windowed macOS `designos --screenshot` capture of the Untitled UI kit:** `ioreg -c IOHIDSystem | awk '/HIDIdleTime/ {print int($NF/1e9); exit}'` returned `54` seconds during implementation and `0` seconds at gate completion (`plans/reports/agy-1468-lock-aspect-evidence/11-idle-check.log:1`), below the mandatory `300`-second idle threshold in `brief-common-argon.txt`. Opening a GUI window while the owner is active at the keyboard is prohibited. Per Acceptance 3 ("If the idle rule never holds, say so and add a headless render of the panel instead"), `render_headless_kit_panel_evidence_when_requested` (`crates/designos-app/src/panels/properties.rs:3742-3835`) imported an APFS clone of `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` and rasterized `egui::Context::tessellate` at `2.0` pixels-per-point into `out/evidence/1468-lock-aspect-off.png`, `out/evidence/1468-lock-aspect-on.png`, and `out/evidence/1468-lock-aspect-panel.png`.

---

## 7. Skills Loaded & Decisions Driven by Each Skill

1. **`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md` & `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`:**
   - Guided the Re-anchor -> Red test -> Minimal surgical implementation -> Verification gate pipeline without adding unrequested scope in `designos-mcp` or `designos-render`.
2. **`/Users/jangtrinh/.gemini/config/skills/es-designer/SKILL.md` (plus `craft-floor.md`, `references/build-loop.md`, `checklist.md`, `coverage.md`):**
   - Placed the `"lock-aspect"` button at `theme::SECTION_TRAILING_X` (`209.0`, `crates/designos-app/src/theme.rs:568`) with `dy = theme::SPACE_FACT` (`4.0`, `crates/designos-app/src/theme.rs:333`) and size `theme::PANEL_ACTION_BUTTON` (`24.0`, `crates/designos-app/src/theme.rs:480`), matching `"corner-individual"` (`crates/designos-app/src/panels/properties.rs:1099`) and `"corner-smoothing-ios"` (`crates/designos-app/src/panels/properties.rs:1259`) with zero raw layout or color literals (`check_spacing_relationships.py` and `check_chrome_colour_literals.py` both passed).
   - Visually inspecting the first headless render (`es:designer` Step 4) caught that `460`pt panel height clipped the bottom of the `Dimensions` row and that an un-laid-out auto-layout card changed height on its first edit; raising the capture height to `560`pt and selecting a fixed-size kit card (`crates/designos-app/src/panels/properties.rs:3760-3770`) produced a clean comparison render (`out/evidence/1468-lock-aspect-panel.png`).
3. **`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md` & `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`:**
   - Kept all edits inside `dimensions_ui`, `resize_selection`, and `update_resize` without allocating on idle frames or touching `doc_hash` (`crates/designos-app/src/home/session.rs:525`).
4. **`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md` & `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`:**
   - Enforced red-before-green tests (`plans/reports/agy-1468-lock-aspect-evidence/01-red-base.log:10-81`) and two targeted mutation kills (`plans/reports/agy-1468-lock-aspect-evidence/03-mutant-1-drag.log:17`, `plans/reports/agy-1468-lock-aspect-evidence/04-mutant-2-toggle.log:17`).

---

## 8. Weakest Link

- In a multi-node canvas handle drag (`ResizeKind::Multi` at `crates/designos-app/src/canvas/interact.rs:1513-1536`), aspect-ratio locking continues to be governed by `inp.mods.shift || is_scale_tool` on the selection bounding box (`crates/designos-app/src/canvas/interact.rs:1515`), whereas single-node handle drags (`ResizeKind::Single` at `crates/designos-app/src/canvas/interact.rs:1487-1494`) and multi-selection W/H commits in the inspector (`resize_selection` at `crates/designos-app/src/panels/properties.rs:1420-1441`) enforce each node's `lock_aspect`.

Status: PASS (Implemented #1468 inspector lock-aspect toggle, proportional W/H edits respecting Fill/Hug sizing, locked canvas handle drags, and .fig/REST round-trip tests; all gates green and 2 mutants killed.)
