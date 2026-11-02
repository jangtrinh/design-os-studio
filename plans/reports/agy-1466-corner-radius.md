# DEV Report · #1466 Individual Corner Radius Fields and Corner Smoothing Slider

## What Could NOT Be Verified

- Full-suite parallel run of `vecops::tests::two_deferred_text_commands_both_convert_in_order` inside the 4,167-test `cargo test -p designos-app --bin designos --locked` invocation timed out waiting for the shared `"SlowvecFnt1"` mock font (`plans/reports/agy-1466-corner-radius-evidence/07-test-designos-app-bin.log:4186`, `plans/reports/agy-1466-corner-radius-evidence/07-test-designos-app-bin.log:4201-4208`) due to a pre-existing parallel font-registration contention across 5 tests in `crates/designos-app/src/vecops.rs:1470-2013`. It was verified in isolation with the same test harness in `1.48s` (`1 passed; 0 failed`, `plans/reports/agy-1466-corner-radius-evidence/07b-test-vecops-isolated.log:10-12`).
- `designos-gpu`'s `large_file_compile_benchmarks` test in `crates/designos-gpu/tests/large_file_perf.rs` was intentionally skipped per the common brief (`-- --skip large_file_compile_benchmarks`, `plans/reports/agy-1466-corner-radius-evidence/09-test-designos-gpu.log:119-123`) so the client benchmark file is never opened.

---

## 1. Re-anchor (`(file:line)` citations verified at HEAD in `plans/reports/agy-1466-corner-radius-evidence/r2-citation-grep.log:1-103`)

1. **Per-corner radius fields and `corner_smoothing` on `Node` (`designos-doc`):**
   - `Node::corner_radii: [f32; 4]` is stored at `crates/designos-doc/src/node.rs:97` in `(top-left, top-right, bottom-right, bottom-left)` order (`crates/designos-doc/src/node.rs:95`) and initialized to `[0.0; 4]` in `Node::new` (`crates/designos-doc/src/node.rs:389`).
   - `Node::corner_smoothing: f32` is stored at `crates/designos-doc/src/node.rs:100` (`0..1` iOS-style corner smoothing, `crates/designos-doc/src/node.rs:98`), serialized with `#[serde(default, skip_serializing_if = "is_zero")]` (`crates/designos-doc/src/node.rs:99`), and initialized to `0.0` in `Node::new` (`crates/designos-doc/src/node.rs:390`).

2. **Inspector radius controls, node-kind predicates, and what they write (`designos-app`):**
   - `PropsState` (`crates/designos-app/src/panels/properties.rs:167-175`) stores `individual_corners: bool` (`crates/designos-app/src/panels/properties.rs:167`), `individual_corners_sel: Option<Vec<NodeId>>` (`crates/designos-app/src/panels/properties.rs:169`), and two reusable scratch `Vec<NodeId>` buffers (`corner_nodes` at `crates/designos-app/src/panels/properties.rs:173` and `four_corner_nodes` at `crates/designos-app/src/panels/properties.rs:175`).
   - Helper predicates and geometry cache invalidation in `crates/designos-app/src/panels/properties.rs:386-434`:
     - `node_has_four_corners` (`crates/designos-app/src/panels/properties.rs:386`) matches `NodeKind::Rectangle | NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. }`.
     - `node_has_corners` (`crates/designos-app/src/panels/properties.rs:390`) returns `node_has_four_corners(n) || matches!(n.kind, NodeKind::Polygon { .. } | NodeKind::Star { .. })`.
     - `uniform_corner_radius` (`crates/designos-app/src/panels/properties.rs:399`) returns `Some(r0)` when all four corners are equal within `1e-4` and `None` when mixed; `common_corners` (`crates/designos-app/src/panels/properties.rs:407`) folds across selected corner nodes.
     - `clear_rect_geom` (`crates/designos-app/src/panels/properties.rs:413`) clears imported `fill_geometry` and `stroke_geometry` on `NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. } | NodeKind::Section | NodeKind::Rectangle`, matching `crates/designos-doc/src/variables_apply.rs:388`.
     - `remove_field_rect` (`crates/designos-app/src/panels/properties.rs:423`) and `remove_test_rect` (`crates/designos-app/src/panels/properties.rs:430`) build their `egui::Id` outside `ctx.data_mut`.
   - In the Appearance section (`crates/designos-app/src/panels/properties.rs:1078-1269`):
     - `corner_nodes` (`crates/designos-app/src/panels/properties.rs:1079`) and `four_corner_nodes` (`crates/designos-app/src/panels/properties.rs:1082`) are populated over the entire `sel` slice without `.take(128)`.
     - The uniform radius field (`"radius"`, `crates/designos-app/src/panels/properties.rs:1100-1157`) checks `rad_keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"]` (`crates/designos-app/src/panels/properties.rs:1106`), shows a bound variable pill when any of the 4 corner keys is bound on a selected node (`crates/designos-app/src/panels/properties.rs:1107-1139`), switches to text-edit mode when the user clicks the text portion of the pill (`crates/designos-app/src/panels/properties.rs:1129-1136`), and on commit detaches all 4 corner keys via `app.detach_variable(&corner_nodes, &rad_keys)` (`crates/designos-app/src/panels/properties.rs:1146`) and writes `n.corner_radii = [v; 4]` to every node in `corner_nodes` (`crates/designos-app/src/panels/properties.rs:1147-1153`).
     - When `has_four_corners` is true (`crates/designos-app/src/panels/properties.rs:1158`), the Individual corners toggle (`"corner-individual"`, `crates/designos-app/src/panels/properties.rs:1159-1160`) is rendered. When `has_four_corners && app.props.individual_corners` (`crates/designos-app/src/panels/properties.rs:1176`), a 2×2 grid is drawn in `TL` (`(0, 0, "radius-tl", "topLeftRadius", "TL")`, `crates/designos-app/src/panels/properties.rs:1178`), `TR` (`(1, 1, "radius-tr", "topRightRadius", "TR")`, `crates/designos-app/src/panels/properties.rs:1179`), `BL` (`(0, 3, "radius-bl", "bottomLeftRadius", "BL")`, `crates/designos-app/src/panels/properties.rs:1180`), `BR` (`(1, 2, "radius-br", "bottomRightRadius", "BR")`, `crates/designos-app/src/panels/properties.rs:1181`) order, using `field::num_field_prefix_gap` (`crates/designos-app/src/panels/properties.rs:1188` -> `crates/designos-app/src/widgets/field.rs:568`, `572`, `615`) so each prefix label has a `theme::SPACE_FACT` (`4 px`) gap to its value.
     - When `has_four_corners` is true (`crates/designos-app/src/panels/properties.rs:1203`), the Corner smoothing row renders the slider track (`"corner-smoothing-slider"`, `crates/designos-app/src/panels/properties.rs:1208-1240`), drawing the circular thumb only when `let Some(sv) = smooth_val` (`crates/designos-app/src/panels/properties.rs:1218`), calling `app.ensure_scrub(ui.ctx())` (`crates/designos-app/src/panels/properties.rs:1231` -> `crates/designos-app/src/ops.rs:298`) on drag/click so a drag across all `four_corner_nodes` records a single undo step, followed by the percentage `field::num_field` (`"corner-smoothing"`, `crates/designos-app/src/panels/properties.rs:1242-1249`, which displays `"Mixed"` when `smooth_pct` is `None` at `crates/designos-app/src/widgets/field.rs:563`) and the `"iOS"` preset button (`"corner-smoothing-ios"`, `crates/designos-app/src/panels/properties.rs:1251-1259`).

3. **Alt-drag per-corner canvas edit (`designos-app`):**
   - Canvas corner-radius handles are gated and hit-tested in `crates/designos-app/src/canvas/handles.rs:25-72`: `supports_corner_radius_handles` (`crates/designos-app/src/canvas/handles.rs:25`), `corner_radius_handle_positions` (`crates/designos-app/src/canvas/handles.rs:41`), and `hit_corner_radius_handle` (`crates/designos-app/src/canvas/handles.rs:67`).
   - Pressing a corner handle starts `Interaction::CornerRadius(CornerRadiusDragState { tx: Tx::new(), id, corner, start_radii: n.corner_radii })` at `crates/designos-app/src/canvas/interact.rs:843-845`.
   - During the drag (`crates/designos-app/src/canvas/interact.rs:1148-1166`), when `inp.mods.alt` is held (`crates/designos-app/src/canvas/interact.rs:1157`), only `r[cr.corner] = new_r` is written (`crates/designos-app/src/canvas/interact.rs:1159`); without Alt, `n.corner_radii = [new_r; 4]` is written (`crates/designos-app/src/canvas/interact.rs:1162`). Releasing commits the single `Tx` via `app.run_tx(c.tx)` (`crates/designos-app/src/canvas/interact.rs:1617-1618`).

4. **How `corner_smoothing` is imported (`.fig`, REST) and rendered (CPU, GPU):**
   - **`.fig` Kiwi import & export (`designos-fig`):** `crates/designos-fig/src/fig.rs:929` and `crates/designos-fig/src/fig.rs:1576` copy `"cornerSmoothing"` from the Kiwi record into REST-shaped JSON, and `crates/designos-fig/src/fig.rs:1577-1593` maps `rectangleCornerRadiiIndependent` and per-corner fields into `rectangleCornerRadii`. Native `.fig` export writes `"cornerSmoothing"` at `crates/designos-fig/src/fig_export.rs:965`.
   - **REST JSON import & export (`designos-fig`):** `crates/designos-fig/src/rest_import.rs:1313` (and `crates/designos-fig/src/rest_import.rs:2934`) populates `n.corner_radii`, and `crates/designos-fig/src/rest_import.rs:1318` sets `n.corner_smoothing = fnum(v, "cornerSmoothing").unwrap_or(0.0)`. REST export writes `"cornerSmoothing"` at `crates/designos-fig/src/export.rs:523`.
   - **CPU rendering (`designos-render`):** `SmoothCorner` (`crates/designos-render/src/geom.rs:109`, `crates/designos-render/src/geom.rs:121`) and `smooth_rounded_rect` (`crates/designos-render/src/geom.rs:178`) build Figma's cubic-Bezier + circular-arc smooth corner path when `smoothing > 0.0`, called from `node_shape` (`crates/designos-render/src/geom.rs:418`, `crates/designos-render/src/geom.rs:441`). `crates/designos-render/src/lib.rs:564`, `crates/designos-render/src/lib.rs:893`, and `crates/designos-render/src/outline.rs:224` check `n.corner_smoothing > 0.0` before taking circular fast paths.
   - **GPU rendering (`designos-gpu`):** `crates/designos-gpu/src/geom.rs:158` hashes `fbits(n.corner_smoothing)` in `ShapeKey`, `crates/designos-gpu/src/geom.rs:201` and `crates/designos-gpu/src/geom.rs:214` cache `smoothing: n.corner_smoothing` on `NodeGeom`, and `crates/designos-gpu/src/compile.rs:1418` bypasses the analytic SDF drop-shadow quad when `radii[0] > 0.0 && n.corner_smoothing > 0.0`.

5. **How a radius field binds a variable (`designos-app` & `designos-doc`):**
   - `var_bind::numeric` (`crates/designos-app/src/panels/var_bind.rs:544`) checks `pill_field(app, ui, rect, sel, keys)` (`crates/designos-app/src/panels/var_bind.rs:514`); when unbound, it narrows the value rect via `value_rect` / `narrow_for_bind` (`crates/designos-app/src/panels/var_bind.rs:262`, `crates/designos-app/src/panels/var_bind.rs:273`) and draws the variable bind button via `hover_button` (`crates/designos-app/src/panels/var_bind.rs:532`).
   - Picking a variable in `bind_picker` (`crates/designos-app/src/panels/var_bind.rs:613`) calls `app.bind_variable(&bp.nodes, &key_refs, &id)` (`crates/designos-app/src/panels/var_bind.rs:721`), binding every key in `key_refs` and applying variables on the document.
   - `crates/designos-doc/src/variables_apply.rs:35`, `crates/designos-doc/src/variables_apply.rs:70`, `crates/designos-doc/src/variables_apply.rs:388`, and `crates/designos-doc/src/variables_apply.rs:406` scope-check, clear geometry on `Frame | Component | ComponentSet | Instance | Section | Rectangle`, and resolve `"topLeftRadius"`, `"topRightRadius"`, `"bottomRightRadius"`, and `"bottomLeftRadius"` onto `node.corner_radii[0..4]`.

---

## 2. Acceptance Table & Verification Evidence

| # | Acceptance Check | Verdict | Evidence Citation |
|---|---|---|---|
| 1a | Unit test: toggle on shows four fields in Figma's order (`top-left`, `top-right`, `bottom-right`, `bottom-left`), red on base `53860a64`, green on final | PASS | Red: `plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:17`, `plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:34-38` · Green: `plans/reports/agy-1466-corner-radius-evidence/01-green-null-control.log:16` (`crates/designos-app/src/shell/tests/corner_radius.rs:46-96`) |
| 1b | Unit test: per-corner edit changes only that corner, red on base `53860a64`, green on final | PASS | Red: `plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:16`, `plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:29-33` · Green: `plans/reports/agy-1466-corner-radius-evidence/01-green-null-control.log:19` (`crates/designos-app/src/shell/tests/corner_radius.rs:99-124`) |
| 1c | Unit test: mixed selection shows `"Mixed"` and an edit writes every node, red on base `53860a64`, green on final | PASS | Red: `plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:19`, `plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:45-49` · Green: `plans/reports/agy-1466-corner-radius-evidence/01-green-null-control.log:18` (`crates/designos-app/src/shell/tests/corner_radius.rs:127-165`) |
| 1d | Unit test: smoothing slider writes `0.6` at the `"iOS"` preset and one drag is one undo step, red on base `53860a64`, green on final | PASS | Red: `plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:15`, `plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:24-28` · Green: `plans/reports/agy-1466-corner-radius-evidence/01-green-null-control.log:20` (`crates/designos-app/src/shell/tests/corner_radius.rs:168-200`) |
| 1e | Unit test: binding a corner field to a variable resolves on the node and survives a save and reopen, red on base `53860a64`, green on final | PASS | Red: `plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:18`, `plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:40-44` · Green: `plans/reports/agy-1466-corner-radius-evidence/01-green-null-control.log:15` (`crates/designos-app/src/shell/tests/corner_radius.rs:203-246`) |
| 2a | Null control green before mutation testing (`11 passed; 0 failed`) | PASS | `plans/reports/agy-1466-corner-radius-evidence/01-green-null-control.log:22` |
| 2b | Mutant 1 killed: per-corner edit writes all four corners (`n.corner_radii = [v; 4]`) | PASS (KILLED) | `plans/reports/agy-1466-corner-radius-evidence/02-mutant1-all-four-corners.log:15-16`, `plans/reports/agy-1466-corner-radius-evidence/02-mutant1-all-four-corners.log:24-44` (`9 passed; 2 failed`) |
| 2c | Mutant 2 killed: slider drag records one undo step per frame (`app.scrub = None`) | PASS (KILLED) | `plans/reports/agy-1466-corner-radius-evidence/03-mutant2-undo-per-frame.log:20`, `plans/reports/agy-1466-corner-radius-evidence/03-mutant2-undo-per-frame.log:24-36` (`10 passed; 1 failed`) |
| 3 | GUI proof on APFS clone of Untitled UI PRO v8.0 kit (`HIDIdleTime >= 300`, one window at a time) | PASS | `plans/reports/agy-1466-corner-radius-evidence/10-gui-captures.log:1-6` (`HIDIdleTime=31765`, `31790`, `31814`) · `out/evidence/1466-individual-corners-off-smoothing-0.png`, `out/evidence/1466-individual-corners-on.png`, `out/evidence/1466-smoothing-60-ios.png` |
| 4a | `cargo fmt --all -- --check; echo fmt_rc=$?` | PASS (`fmt_rc=0`) | `plans/reports/agy-1466-corner-radius-evidence/04-fmt.log:1` |
| 4b | `cargo clippy --workspace --all-targets --locked` | PASS (0 errors; only `main`'s 3 pre-existing warnings) | `plans/reports/agy-1466-corner-radius-evidence/05-clippy.log:374-414` |
| 4c | `cargo test -p designos-app --bin designos --locked` | FAIL in suite (`4153 passed; 1 failed` on `vecops::tests::two_deferred_text_commands_both_convert_in_order`) / PASS in isolation (`1 passed; 0 failed`) | `plans/reports/agy-1466-corner-radius-evidence/07-test-designos-app-bin.log:4208`, `plans/reports/agy-1466-corner-radius-evidence/07b-test-vecops-isolated.log:12` |
| 4d | `cargo test --workspace --locked --lib` | PASS (`1,498 passed; 0 failed` across all 12 workspace crates) | `plans/reports/agy-1466-corner-radius-evidence/08-test-workspace-lib.log:506`, `1232`, `1239`, `1404`, `1553`, `1589`, `1600`, `1643`, `1975`, `2039`, `2046`, `2060` |
| 4e | `python3 scripts/zero_fake_ui_gate.py` (plus spacing, chrome colour, and DS ontology parity gates) | PASS (`17/17` invariants, `rc=0`) | `plans/reports/agy-1466-corner-radius-evidence/06-python-gates.log:24-45` |
| 4f | `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | PASS (`0 failed`; `large_file_compile_benchmarks` filtered out `1 filtered out`) | `plans/reports/agy-1466-corner-radius-evidence/09-test-designos-gpu.log:62`, `69`, `79`, `103`, `117`, `123`, `143`, `154`, `172`, `197`, `206`, `214`, `222`, `233`, `241`, `247` |
| 5a | Draft PR open against `main` & `git log --oneline origin/main..HEAD` | PASS (`https://github.com/jangtrinh/design-os-studio/pull/1533`, commit `88d8579d`) | `plans/reports/agy-1466-corner-radius-evidence/12-pr-url.log:1`, `plans/reports/agy-1466-corner-radius-evidence/13-git-log.log:1` |
| 5b | Delete `out/cargo` at the end and paste `ls out/cargo` failing | PASS (`ls: out/cargo: No such file or directory`, `ls_out_cargo_rc=1`) | `plans/reports/agy-1466-corner-radius-evidence/11-cleanup.log:1-2` |

---

## 3. Exact Commands and Pasted Outputs

### 3.1 Red Tests on Base Commit `53860a64` (`plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:1-60`)

Command:
```bash
env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked corner_radius
```

Output (`plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:9-60`):
```text
running 11 tests
test canvas::handles::tests::corner_radius_handle_positions_and_hit_testing ... ok
test canvas::vector::tests::corner_radius_is_one_undo_step_and_rounds_the_derived_path ... ok
test check_designs::tests::numbers::uniform_and_per_corner_radius_match_number_variables_and_bind_cleanly ... ok
test check_designs::tests::under_an_opaque_fill::text_inside_a_rounded_opaque_card_inset_by_corner_radius_is_checked_while_corner_cutout_text_is_skipped ... ok
test shell::tests::vector::the_vector_edit_fixture_shows_the_corner_radius_field ... ok
test shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step ... FAILED
test shell::tests::corner_radius::per_corner_edit_changes_only_that_corner ... FAILED
test shell::tests::corner_radius::toggle_on_shows_four_fields_in_figma_order ... FAILED
test shell::tests::corner_radius::binding_corner_field_to_variable_resolves_and_survives_save_and_reopen ... FAILED
test shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node ... FAILED
test shell::tests::vector::the_vertex_inspector_edits_the_corner_radius_as_one_undo_step ... ok

failures:

---- shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step stdout ----

thread 'shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step' (67632200) panicked at crates/designos-app/src/shell/tests.rs:228:113:
nothing recorded for corner-smoothing-ios

---- shell::tests::corner_radius::per_corner_edit_changes_only_that_corner stdout ----

thread 'shell::tests::corner_radius::per_corner_edit_changes_only_that_corner' (67632199) panicked at crates/designos-app/src/shell/tests.rs:228:113:
nothing recorded for corner-individual

---- shell::tests::corner_radius::toggle_on_shows_four_fields_in_figma_order stdout ----

thread 'shell::tests::corner_radius::toggle_on_shows_four_fields_in_figma_order' (67632201) panicked at crates/designos-app/src/shell/tests.rs:228:113:
nothing recorded for corner-individual
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- shell::tests::corner_radius::binding_corner_field_to_variable_resolves_and_survives_save_and_reopen stdout ----

thread 'shell::tests::corner_radius::binding_corner_field_to_variable_resolves_and_survives_save_and_reopen' (67632197) panicked at crates/designos-app/src/shell/tests.rs:228:113:
nothing recorded for corner-individual

---- shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node stdout ----

thread 'shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node' (67632198) panicked at crates/designos-app/src/shell/tests/corner_radius.rs:156:5:
assertion failed: has_field(&s, "radius-tl")


failures:
    shell::tests::corner_radius::binding_corner_field_to_variable_resolves_and_survives_save_and_reopen
    shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node
    shell::tests::corner_radius::per_corner_edit_changes_only_that_corner
    shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step
    shell::tests::corner_radius::toggle_on_shows_four_fields_in_figma_order

test result: FAILED. 6 passed; 5 failed; 0 ignored; 0 measured; 4156 filtered out; finished in 0.78s
```

### 3.2 Green Null Control & Two Killed Mutants (`01-green-null-control.log`, `02-mutant1-all-four-corners.log`, `03-mutant2-undo-per-frame.log`)

Command (null control):
```bash
env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked corner_radius
```

Null control output (`plans/reports/agy-1466-corner-radius-evidence/01-green-null-control.log:9-22`):
```text
running 11 tests
test canvas::handles::tests::corner_radius_handle_positions_and_hit_testing ... ok
test canvas::vector::tests::corner_radius_is_one_undo_step_and_rounds_the_derived_path ... ok
test check_designs::tests::numbers::uniform_and_per_corner_radius_match_number_variables_and_bind_cleanly ... ok
test check_designs::tests::under_an_opaque_fill::text_inside_a_rounded_opaque_card_inset_by_corner_radius_is_checked_while_corner_cutout_text_is_skipped ... ok
test shell::tests::vector::the_vector_edit_fixture_shows_the_corner_radius_field ... ok
test shell::tests::corner_radius::binding_corner_field_to_variable_resolves_and_survives_save_and_reopen ... ok
test shell::tests::corner_radius::toggle_on_shows_four_fields_in_figma_order ... ok
test shell::tests::vector::the_vertex_inspector_edits_the_corner_radius_as_one_undo_step ... ok
test shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node ... ok
test shell::tests::corner_radius::per_corner_edit_changes_only_that_corner ... ok
test shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 4156 filtered out; finished in 2.05s
```

Mutant 1 (`n.corner_radii[corner_idx] = v;` -> `n.corner_radii = [v; 4];` in `crates/designos-app/src/panels/properties.rs:1134`) output (`plans/reports/agy-1466-corner-radius-evidence/02-mutant1-all-four-corners.log:9-46`):
```text
running 11 tests
test canvas::handles::tests::corner_radius_handle_positions_and_hit_testing ... ok
test canvas::vector::tests::corner_radius_is_one_undo_step_and_rounds_the_derived_path ... ok
test check_designs::tests::numbers::uniform_and_per_corner_radius_match_number_variables_and_bind_cleanly ... ok
test check_designs::tests::under_an_opaque_fill::text_inside_a_rounded_opaque_card_inset_by_corner_radius_is_checked_while_corner_cutout_text_is_skipped ... ok
test shell::tests::vector::the_vector_edit_fixture_shows_the_corner_radius_field ... ok
test shell::tests::corner_radius::per_corner_edit_changes_only_that_corner ... FAILED
test shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node ... FAILED
test shell::tests::corner_radius::toggle_on_shows_four_fields_in_figma_order ... ok
test shell::tests::corner_radius::binding_corner_field_to_variable_resolves_and_survives_save_and_reopen ... ok
test shell::tests::vector::the_vertex_inspector_edits_the_corner_radius_as_one_undo_step ... ok
test shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step ... ok

failures:

---- shell::tests::corner_radius::per_corner_edit_changes_only_that_corner stdout ----

thread 'shell::tests::corner_radius::per_corner_edit_changes_only_that_corner' (67843508) panicked at crates/designos-app/src/shell/tests/corner_radius.rs:108:5:
assertion `left == right` failed: editing top-left must change only corner_radii[0]
  left: [4.0, 4.0, 4.0, 4.0]
 right: [4.0, 12.0, 12.0, 12.0]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node stdout ----

thread 'shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node' (67843507) panicked at crates/designos-app/src/shell/tests/corner_radius.rs:147:5:
assertion `left == right` failed
  left: [24.0, 24.0, 24.0, 24.0]
 right: [16.0, 24.0, 16.0, 16.0]


failures:
    shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node
    shell::tests::corner_radius::per_corner_edit_changes_only_that_corner

test result: FAILED. 9 passed; 2 failed; 0 ignored; 0 measured; 4156 filtered out; finished in 1.57s
```

Mutant 2 (`app.ensure_scrub(ui.ctx());` -> `app.scrub = None;` in `crates/designos-app/src/panels/properties.rs:1170`) output (`plans/reports/agy-1466-corner-radius-evidence/03-mutant2-undo-per-frame.log:9-38`):
```text
running 11 tests
test canvas::handles::tests::corner_radius_handle_positions_and_hit_testing ... ok
test check_designs::tests::numbers::uniform_and_per_corner_radius_match_number_variables_and_bind_cleanly ... ok
test canvas::vector::tests::corner_radius_is_one_undo_step_and_rounds_the_derived_path ... ok
test check_designs::tests::under_an_opaque_fill::text_inside_a_rounded_opaque_card_inset_by_corner_radius_is_checked_while_corner_cutout_text_is_skipped ... ok
test shell::tests::vector::the_vector_edit_fixture_shows_the_corner_radius_field ... ok
test shell::tests::corner_radius::toggle_on_shows_four_fields_in_figma_order ... ok
test shell::tests::corner_radius::binding_corner_field_to_variable_resolves_and_survives_save_and_reopen ... ok
test shell::tests::vector::the_vertex_inspector_edits_the_corner_radius_as_one_undo_step ... ok
test shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node ... ok
test shell::tests::corner_radius::per_corner_edit_changes_only_that_corner ... ok
test shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step ... FAILED

failures:

---- shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step stdout ----

thread 'shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step' (67846610) panicked at crates/designos-app/src/shell/tests/corner_radius.rs:190:5:
assertion `left == right` failed: one slider drag from press to release must be a single undo step
  left: 0.64
 right: 0.0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step

test result: FAILED. 10 passed; 1 failed; 0 ignored; 0 measured; 4156 filtered out; finished in 2.25s
```

### 3.3 GUI Proof on APFS Clone of the Untitled UI Kit (`plans/reports/agy-1466-corner-radius-evidence/10-gui-captures.log:1-6`)

Commands:
```bash
cp -c "/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$S/kit.fig"
out/cargo/debug/designos "$S/kit.fig" --select id:44757 --apply-spec "$S/card-off.json" --out "$S/kit-off.designos"
out/cargo/debug/designos "$S/kit-off.designos" --select "id:210250" --zoom fit --no-toast --screenshot out/evidence/1466-individual-corners-off-smoothing-0.png
out/cargo/debug/designos "$S/kit-on.designos" --select "id:210250" --zoom fit --no-toast --screenshot out/evidence/1466-individual-corners-on.png
out/cargo/debug/designos "$S/kit-smooth60.designos" --select "id:210250" --zoom fit --no-toast --screenshot out/evidence/1466-smoothing-60-ios.png
```

Output (`plans/reports/agy-1466-corner-radius-evidence/10-gui-captures.log:1-6`):
```text
Capture 1 HIDIdleTime=31765
Capture 2 HIDIdleTime=31790
Capture 3 HIDIdleTime=31814
-rw-r--r--@ 1 jangtrinh  staff  228575 Nov  2 16:36 out/evidence/1466-individual-corners-off-smoothing-0.png
-rw-r--r--@ 1 jangtrinh  staff  226679 Nov  2 16:37 out/evidence/1466-individual-corners-on.png
-rw-r--r--@ 1 jangtrinh  staff  230220 Nov  2 16:37 out/evidence/1466-smoothing-60-ios.png
```

### 3.4 Gates (`04-fmt.log`, `05-clippy.log`, `06-python-gates.log`, `07-test-designos-app-bin.log`, `07b-test-vecops-isolated.log`, `08-test-workspace-lib.log`, `09-test-designos-gpu.log`)

#### `cargo fmt --all -- --check; echo fmt_rc=$?` (`plans/reports/agy-1466-corner-radius-evidence/04-fmt.log:1`)
```text
fmt_rc=0
```

#### `cargo clippy --workspace --all-targets --locked` (`plans/reports/agy-1466-corner-radius-evidence/05-clippy.log:374-414`)
```text
warning: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1176:9
warning: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1194:9
warning: very complex type used. Consider factoring parts into `type` definitions
    --> crates/designos-app/src/shell/tests/inspector_var_binds.rs:1181:21
warning: `designos-app` (bin "designos-studio" test) generated 3 warnings (2 duplicates)
warning: `designos-app` (bin "designos" test) generated 3 warnings (3 duplicates)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 50.21s
```

#### `python3 scripts/zero_fake_ui_gate.py` & Python design gates (`plans/reports/agy-1466-corner-radius-evidence/06-python-gates.log:23-45`)
```text
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
zero_fake_ui_gate_rc=0
literal geometry outside theme.rs: offset_literals=1521, inset_literals=41, radius_literals=171, font_literals=23, nonconcentric_pairs=2, legacy_control_refs=6
note: 2 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
check_spacing_relationships_rc=0
Chrome colour literal gate PASSED (29 files with findings, 199/199 baseline chrome sites).
check_chrome_colour_literals_rc=0
P1 registry <-> Rust symbols: PASS  78 records, 66 distinct symbols resolve, 3 records report-only (new-in-*, not blocking until the owning spec)
    report: 2 record(s) with rust_status=new-in-D: Bar/Contextual, Navigation/TitleBar
    report: 1 record(s) with rust_status=new-in-K: Card/Library
P2 painted code (colour and spacing ratchets): PASS  2 ratchet scripts run
P3 theme names <-> store <-> records (colours): PASS  188 theme.rs colour names (32 mapped, 21 shared-key, 68 pending, 67 exempt); 54 records with colours checked
P4 tiers and composition: PASS  78 records, 232 composition edges
P5 states: PASS  39 records compared with kit-states.json
P6 flow screens <-> records: PASS  16 flow screens, 16 flow ids in records
P7 absences (pinned sites): PASS  ns1 5 sites, ns2 0 sites, ns4 9 sites, ns5 62 sites
P9 shipped names (the design store holds no kit role name): PASS  10 store-set files, 0 kit roles, 0 UTILITY_* hits
    kit role scan NOT RUN (no token contract: pass --contract or set DESIGN_OS_TOKEN_CONTRACT)
RESULT: PASS (P1, P2, P3, P4, P5, P6, P7, P9)
check_ds_ontology_parity_rc=0
```

#### `cargo test -p designos-app --bin designos --locked` (`plans/reports/agy-1466-corner-radius-evidence/07-test-designos-app-bin.log:4208` and `plans/reports/agy-1466-corner-radius-evidence/07b-test-vecops-isolated.log:9-12`)
```text
test result: FAILED. 4153 passed; 1 failed; 13 ignored; 0 measured; 0 filtered out; finished in 391.92s

running 1 test
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4166 filtered out; finished in 1.48s
```

#### `cargo test --workspace --locked --lib` (`plans/reports/agy-1466-corner-radius-evidence/08-test-workspace-lib.log:506-2060`)
```text
Running unittests src/lib.rs (designos_diff): test result: ok. 13 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (designos_doc): test result: ok. 720 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (designos_eval): test result: ok. 1 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (designos_export): test result: ok. 158 passed; 0 failed; 1 ignored
Running unittests src/lib.rs (designos_fig): test result: ok. 143 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (designos_fonts): test result: ok. 30 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (designos_gpu): test result: ok. 5 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (designos_library): test result: ok. 37 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (designos_mcp): test result: ok. 326 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (designos_render): test result: ok. 56 passed; 0 failed; 2 ignored
Running unittests src/lib.rs (designos_svg): test result: ok. 1 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (designos_webcapture): test result: ok. 8 passed; 0 failed; 0 ignored
```

#### `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` (`plans/reports/agy-1466-corner-radius-evidence/09-test-designos-gpu.log:53-247`)
```text
Running unittests src/lib.rs: test result: ok. 5 passed; 0 failed; 0 ignored
Running tests/adapter.rs: test result: ok. 1 passed; 0 failed; 0 ignored
Running tests/command_buffer_limit.rs: test result: ok. 4 passed; 0 failed; 0 ignored
Running tests/fidelity.rs: test result: ok. 18 passed; 0 failed; 0 ignored
Running tests/full_detail_at_rest.rs: test result: ok. 8 passed; 0 failed; 0 ignored
Running tests/large_file_perf.rs: test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
Running tests/mask_without_own_fill.rs: test result: ok. 14 passed; 0 failed; 0 ignored
Running tests/program_cap.rs: test result: ok. 5 passed; 0 failed; 0 ignored
Running tests/refine_chunks.rs: test result: ok. 12 passed; 0 failed; 0 ignored
Running tests/refine_vram_budget.rs: test result: ok. 19 passed; 0 failed; 0 ignored
Running tests/resized_shapes.rs: test result: ok. 3 passed; 0 failed; 0 ignored
Running tests/shadow_blend_parity.rs: test result: ok. 2 passed; 0 failed; 0 ignored
Running tests/shadow_fillless_fixture.rs: test result: ok. 2 passed; 0 failed; 0 ignored
Running tests/shadow_silhouette_source.rs: test result: ok. 5 passed; 0 failed; 0 ignored
Running tests/text_hug_box.rs: test result: ok. 2 passed; 0 failed; 0 ignored
Doc-tests designos_gpu: test result: ok. 0 passed; 0 failed; 0 ignored
```

### 3.5 Draft PR, `git log --oneline origin/main..HEAD` & `out/cargo` Cleanup (`11-cleanup.log`, `12-pr-url.log`, `13-git-log.log`)

Draft PR URL (`plans/reports/agy-1466-corner-radius-evidence/12-pr-url.log:1`):
```text
https://github.com/jangtrinh/design-os-studio/pull/1533
```

`git log --oneline origin/main..HEAD` (`plans/reports/agy-1466-corner-radius-evidence/13-git-log.log:1`, plus the report commit on top):
```text
88d8579d feat(inspector): individual corner radius fields and a corner smoothing slider (#1466)
```

`rm -rf out/cargo && ls out/cargo` (`plans/reports/agy-1466-corner-radius-evidence/11-cleanup.log:1-2`):
```text
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```

---

## 4. Epistemic Ledger, Weakest Link, Skills Loaded & Follow-ups

### Epistemic Ledger
- **FACT:** `Node::corner_radii: [f32; 4]` (`[TL, TR, BR, BL]`) and `Node::corner_smoothing: f32` (`0.0..=1.0`, `0.6` = iOS) are already persisted on `designos_doc::Node` (`crates/designos-doc/src/node.rs:96-100`), imported/exported by `designos-fig` (`crates/designos-fig/src/fig.rs:1576-1593`, `crates/designos-fig/src/rest_import.rs:1311-1318`, `crates/designos-fig/src/fig_export.rs:964-965`, `crates/designos-fig/src/export.rs:522-523`), and rendered by both `designos-render` (`crates/designos-render/src/geom.rs:121-210`) and `designos-gpu` (`crates/designos-gpu/src/geom.rs:158-214`, `crates/designos-gpu/src/compile.rs:1418`).
- **FACT:** `designos_doc::variables_apply` already supports per-corner variable bindings (`"topLeftRadius"`, `"topRightRadius"`, `"bottomRightRadius"`, `"bottomLeftRadius"`, `crates/designos-doc/src/variables_apply.rs:35`, `70`, `92`, `406-416`), so zero changes to `crates/designos-doc` were required.
- **FACT:** All 5 unit tests in `crates/designos-app/src/shell/tests/corner_radius.rs:46-246` failed on base commit `53860a64` (`plans/reports/agy-1466-corner-radius-evidence/00-red-corner_radius.log:58`) and pass on the implementation (`plans/reports/agy-1466-corner-radius-evidence/01-green-null-control.log:22`). Both required mutants were killed (`plans/reports/agy-1466-corner-radius-evidence/02-mutant1-all-four-corners.log:44` and `plans/reports/agy-1466-corner-radius-evidence/03-mutant2-undo-per-frame.log:36`).
- **FACT:** Adding the 28 px Corner smoothing row (`28 px` control height + `8 px` vertical item spacing = `+36.0 px`) shifted right-panel controls below Appearance by `+36.0 px` on corner-bearing selections; updating the 48 affected `crates/designos-app/tests/layout-baseline/*.layout.json` files passed all 20 `layout_unchanged` tests (`plans/reports/agy-1466-corner-radius-evidence/07-test-designos-app-bin.log:4208`).
- **ASSUMPTION:** When a selection has uniform numeric `corner_radii` across all selected nodes, the Individual corners toggle still defaults to `true` if any selected node has an individual corner variable binding that is not bound identically across all four corners (`any_per_corner_bound`, `crates/designos-app/src/panels/properties.rs:1066-1076`), so a user reopening a document with a single bound corner sees the bound corner pill immediately.

### Weakest Link
- The Individual corners toggle override is keyed in egui temporary memory by the selected node ID list (`(app.selection.clone(), default_individual)`, `crates/designos-app/src/panels/properties.rs:1079-1089`), so if the user manually toggles Individual corners off on a mixed-corner node and then changes one corner via Alt-drag on the canvas without changing the selection IDs, the manual override remains until selection changes or the toggle is clicked again.

### Skills Loaded & Changes Made Because of Them
- `es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`), `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`), `es:orca-native-orchestration` (`/Users/jangtrinh/.gemini/config/skills/es-orca-native-orchestration/SKILL.md`): enforced red-before-green test verification on base commit `53860a64`, null-control + 2-mutant falsification, and `HIDIdleTime >= 300` single-window capture discipline.
- `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`) & `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): executed the DEV phase directly from the controller rulings without extra approval gates.
- `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`) & `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): kept `App::ensure_scrub` zero-clone once `self.scrub` is active (`crates/designos-app/src/ops.rs:306-310`), avoided touching GPU/CPU rendering paths, and ran `designos-gpu` tests with `-- --skip large_file_compile_benchmarks`.
- `es:designer` (`/Users/jangtrinh/.gemini/config/skills/es-designer/SKILL.md`): used `theme::CONTROL_H` (`28 px` >= `24 px` minimum target), `theme::SP_8` (`8 px` 4-px rhythm), existing `theme::*` colour tokens, and `widgets::icon_btn` / `var_bind::numeric` so `zero_fake_ui_gate.py`, `check_spacing_relationships.py`, `check_chrome_colour_literals.py`, and `check_ds_ontology_parity.py` all pass with zero new findings (`plans/reports/agy-1466-corner-radius-evidence/06-python-gates.log:24-45`).

### Follow-ups
- `crates/designos-app/src/vecops.rs:1470-2013`: 5 tests (`deferred_flatten_converts_text_once_slow_font_arrives`, `deferred_flatten_cancels_cleanly_on_undo_before_font_arrives`, `deferred_flatten_respects_failed_state_without_infinite_polling`, `deferred_flatten_rechecks_live_text_font_when_family_changes_before_arrival`, `two_deferred_text_commands_both_convert_in_order`) share the static font family name `"SlowvecFnt1"`, causing a parallel test race when `two_deferred_text_commands_both_convert_in_order` runs concurrently with another `"SlowvecFnt1"` test. Each test should use a distinct font family name.

---

## Round 2

### What Could NOT Be Verified

- Across all three full-suite `cargo test -p designos-app --bin designos --locked` runs (Round 1 `plans/reports/agy-1466-corner-radius-evidence/07-test-designos-app-bin.log:4201-4208`, Round 2 run 1 `plans/reports/agy-1466-corner-radius-evidence/r2-08a-test-app-bin-run1.log:4954-4968`, and Round 2 run 2 `plans/reports/agy-1466-corner-radius-evidence/r2-08b-test-app-bin-run2.log:4239-4252`), `vecops::tests::two_deferred_text_commands_both_convert_in_order` failed in the parallel suite (`both texts still waiting for slow fonts` at `crates/designos-app/src/vecops.rs:1882:9`) due to pre-existing parallel contention on the shared `"SlowvecFnt1"` mock font, and passed in isolation in `1.49s` (`1 passed; 0 failed`, `plans/reports/agy-1466-corner-radius-evidence/r2-08c-test-isolated-reruns.log:380-385`). Per Ruling 9, it is reported here with its logs and not modified in this PR.
- In Round 2 run 1 (`plans/reports/agy-1466-corner-radius-evidence/r2-08a-test-app-bin-run1.log:4949-4953`), `vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope` also hit the same parallel slow-font contention once (`crates/designos-app/src/vecops.rs:1483:9`); it passed in Round 2 run 2 (`plans/reports/agy-1466-corner-radius-evidence/r2-08b-test-app-bin-run2.log:4227`) and in isolation in `1.49s` (`plans/reports/agy-1466-corner-radius-evidence/r2-08c-test-isolated-reruns.log:366-371`).
- In Round 2 runs 1 and 2 (`plans/reports/agy-1466-corner-radius-evidence/r2-08a-test-app-bin-run1.log:4943-4947`, `plans/reports/agy-1466-corner-radius-evidence/r2-08b-test-app-bin-run2.log:4233-4237`), `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` failed when `HOME` was placed under macOS `$TMPDIR` (`/var/folders/.../T/designos-test.XXXXXX/home`) because the resulting `codex.sock` Unix domain socket path was `142 bytes` (exceeding the macOS `sockaddr_un.sun_path` `103-byte` limit at `crates/designos-app/src/agent_approval_channel.rs:286:9`); rerunning with `S="$(mktemp -d /tmp/designos-test.XXXXXX)"` passed in `0.00s` (`1 passed; 0 failed`, `plans/reports/agy-1466-corner-radius-evidence/r2-08c-test-isolated-reruns.log:352-357`).
- `designos-gpu`'s `large_file_compile_benchmarks` test in `crates/designos-gpu/tests/large_file_perf.rs` was skipped per the common brief (`-- --skip large_file_compile_benchmarks`, `plans/reports/agy-1466-corner-radius-evidence/r2-10-test-gpu.log:119-125`).

### Round 2 Acceptance Table

| # | Ruling / Check | Verdict | Evidence Citation |
|---|---|---|---|
| R2-1 | Grid (finding 1): Top row `TL` (`0`), `TR` (`1`); bottom row `BL` (`3`), `BR` (`2`); test asserts `bl` left of `br`, `tl` above `bl`, `tr` above `br` | PASS | Code: `crates/designos-app/src/panels/properties.rs:1178-1181` · Test: `crates/designos-app/src/shell/tests/corner_radius.rs:82-135` · Red on `76a3f9b6`: `plans/reports/agy-1466-corner-radius-evidence/r2-00-red-on-76a3f9b6.log:19`, `79` · Green: `plans/reports/agy-1466-corner-radius-evidence/r2-01-green-null-control.log:17` |
| R2-2 | Write set (finding 2): `corner_nodes` and `four_corner_nodes` computed over entire selection without `.take(128)`; tests for 129 rectangles and 128 text layers + 1 frame | PASS | Code: `crates/designos-app/src/panels/properties.rs:1079-1082` · Tests: `crates/designos-app/src/shell/tests/corner_radius.rs:292-353` · Red on `76a3f9b6`: `plans/reports/agy-1466-corner-radius-evidence/r2-00-red-on-76a3f9b6.log:18`, `20`, `76-77` · Green: `plans/reports/agy-1466-corner-radius-evidence/r2-01-green-null-control.log:16`, `19` |
| R2-3 | Kinds (finding 3): `Rectangle`, `Frame`, `Component`, `ComponentSet`, `Instance` get single + toggle + 4 fields + smoothing; `Polygon` and `Star` get single field only; `Ellipse`, `Section`, `Text`, `Line`, `Group`, `Vector` get no radius row | PASS | Code: `crates/designos-app/src/panels/properties.rs:386-393`, `1158`, `1176`, `1203` · Test: `crates/designos-app/src/shell/tests/corner_radius.rs:356-427` · Red on `76a3f9b6`: `plans/reports/agy-1466-corner-radius-evidence/r2-00-red-on-76a3f9b6.log:23`, `78` · Green: `plans/reports/agy-1466-corner-radius-evidence/r2-01-green-null-control.log:22` |
| R2-4 | Uniform field & binds (finding 6): Uniform field shows bound pill when any of the 4 corner keys is bound; typing a uniform value detaches all 4 corner keys | PASS | Code: `crates/designos-app/src/panels/properties.rs:1106-1146` · Test: `crates/designos-app/src/shell/tests/corner_radius.rs:430-476` · Red on `76a3f9b6`: `plans/reports/agy-1466-corner-radius-evidence/r2-00-red-on-76a3f9b6.log:24`, `73` · Green: `plans/reports/agy-1466-corner-radius-evidence/r2-01-green-null-control.log:23` |
| R2-5 | Label-to-value gap (finding 5): `TL`, `TR`, `BL`, `BR` place value at `label.right() + SPACE_FACT` (`4 px`) via opt-in `field::num_field_prefix_gap` | PASS | Code: `crates/designos-app/src/widgets/field.rs:568-617`, `crates/designos-app/src/panels/properties.rs:1188` · Test: `crates/designos-app/src/shell/tests/corner_radius.rs:544-580` · Red on `76a3f9b6`: `plans/reports/agy-1466-corner-radius-evidence/r2-00-red-on-76a3f9b6.log:15`, `72` · Green: `plans/reports/agy-1466-corner-radius-evidence/r2-01-green-null-control.log:14` |
| R2-6 | Geometry clear (finding 7): `clear_rect_geom` matches `variables_apply.rs:386-389` (`Frame | Component | ComponentSet | Instance | Section | Rectangle`); test on `ComponentSet` | PASS | Code: `crates/designos-app/src/panels/properties.rs:413-421` · Test: `crates/designos-app/src/shell/tests/corner_radius.rs:508-541` · Red on `76a3f9b6`: `plans/reports/agy-1466-corner-radius-evidence/r2-00-red-on-76a3f9b6.log:17`, `74` · Green: `plans/reports/agy-1466-corner-radius-evidence/r2-01-green-null-control.log:15` |
| R2-7 | Mixed smoothing (finding 8): When `smooth_val.is_none()`, field shows `"Mixed"` and track draws no thumb (`Shape::Circle`); drag writes pointer position to every corner node in one undo step | PASS | Code: `crates/designos-app/src/panels/properties.rs:1218-1240` · Test: `crates/designos-app/src/shell/tests/corner_radius.rs:479-505` · Red on `76a3f9b6`: `plans/reports/agy-1466-corner-radius-evidence/r2-00-red-on-76a3f9b6.log:21`, `75` · Green: `plans/reports/agy-1466-corner-radius-evidence/r2-01-green-null-control.log:20` |
| R2-8 | Section 1 re-anchored at HEAD (`r2-citation-grep.log`), Round 1 row 4c updated, `## Round 2` appended | PASS | `plans/reports/agy-1466-corner-radius-evidence/r2-citation-grep.log:1-103` |
| R2-9 | Two additional `cargo test -p designos-app --bin designos --locked` runs on final head (three runs total across R1 + R2) | FAIL in suite (`vecops::tests::two_deferred_text_commands_both_convert_in_order` in all 3 suite runs) / PASS in isolation (`1 passed; 0 failed`) | Run 1: `plans/reports/agy-1466-corner-radius-evidence/r2-08a-test-app-bin-run1.log:4954-4968` · Run 2: `plans/reports/agy-1466-corner-radius-evidence/r2-08b-test-app-bin-run2.log:4239-4252` · Isolated: `plans/reports/agy-1466-corner-radius-evidence/r2-08c-test-isolated-reruns.log:352-385` |
| R2-10 | Comments & scratch reuse (findings 10 & 11): `#1466` dropped from `corner_radius.rs` module comment; scratch `Vec`s on `PropsState`; `egui::Id` built outside `ctx.data_mut` | PASS | `crates/designos-app/src/shell/tests/corner_radius.rs:1-2`, `crates/designos-app/src/panels/properties.rs:172-175`, `423-434`, `1078-1082`, `1264-1267` |
| R2-M0 | Green null control before Round 2 mutants (`18 passed; 0 failed`) | PASS | `plans/reports/agy-1466-corner-radius-evidence/r2-01-green-null-control.log:28` |
| R2-M1 | Mutant 1 killed: old `BR`, `BL` bottom row order (`17 passed; 1 failed` in `toggle_on_shows_four_fields_in_figma_order`) | PASS (KILLED) | `plans/reports/agy-1466-corner-radius-evidence/r2-02-mutant1-old-br-bl-order.log:16-19`, `38` |
| R2-M2 | Mutant 2 killed: `.take(128)` on `corner_nodes` (`16 passed; 2 failed` in 129-rectangles and 128-text+1-frame tests) | PASS (KILLED) | `plans/reports/agy-1466-corner-radius-evidence/r2-03-mutant2-take-128.log:16-26`, `44` |
| R2-M3 | Mutant 3 killed: `NodeKind::Star { .. }` in `node_has_four_corners` (`17 passed; 1 failed` in `star_shows_single_field_only_and_ellipse_and_section_show_no_row`) | PASS (KILLED) | `plans/reports/agy-1466-corner-radius-evidence/r2-04-mutant3-star-four-fields.log:16-19`, `38` |
| R2-GUI | GUI capture `out/evidence/1466-r2-individual-corners-on.png` (`HIDIdleTime=835 >= 300`, `TL 24`, `TR 4`, `BL 8`, `BR 16`, 4 px prefix gap) | PASS | `plans/reports/agy-1466-corner-radius-evidence/r2-04b-gui-capture.log:1-13` · `out/evidence/1466-r2-individual-corners-on.png` |
| R2-G1 | `cargo fmt --all -- --check; echo fmt_rc=$?` | PASS (`fmt_rc=0`) | `plans/reports/agy-1466-corner-radius-evidence/r2-05-fmt.log:1` |
| R2-G2 | `cargo clippy --workspace --all-targets --locked` | PASS (`clippy_rc=0`, 0 warnings in modified files) | `plans/reports/agy-1466-corner-radius-evidence/r2-06-clippy.log:401-415` |
| R2-G3 | `python3 scripts/zero_fake_ui_gate.py` | PASS (`17/17` invariants, `zero_fake_ui_rc=0`) | `plans/reports/agy-1466-corner-radius-evidence/r2-07-zero-fake-ui.log:24-26` |
| R2-G4 | `cargo test --workspace --locked --lib` | PASS (`workspace_lib_rc=0`, `0 failed`) | `plans/reports/agy-1466-corner-radius-evidence/r2-09-test-workspace-lib.log:2065-2075` |
| R2-G5 | `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | PASS (`gpu_rc=0`, `0 failed`) | `plans/reports/agy-1466-corner-radius-evidence/r2-10-test-gpu.log:228-249` |
| R2-CLN | Delete `out/cargo` and verify `ls out/cargo` fails | PASS (`ls: out/cargo: No such file or directory`, `ls_out_cargo_rc=1`) | `plans/reports/agy-1466-corner-radius-evidence/r2-11-cleanup.log:1-2` |

### Round 2 Epistemic Ledger & Weakest Link

- **FACT:** All 8 new/updated Round 2 tests in `crates/designos-app/src/shell/tests/corner_radius.rs:82-580` failed on Round 1 commit `76a3f9b6` (`10 passed; 8 failed`, `plans/reports/agy-1466-corner-radius-evidence/r2-00-red-on-76a3f9b6.log:85`) before merging `origin/main` or editing production files, and all 18 `corner_radius` tests pass on the Round 2 implementation (`18 passed; 0 failed`, `plans/reports/agy-1466-corner-radius-evidence/r2-01-green-null-control.log:28`).
- **FACT:** All 3 Round 2 mutants (`r2-02-mutant1-old-br-bl-order.log:38`, `r2-03-mutant2-take-128.log:44`, `r2-04-mutant3-star-four-fields.log:38`) were killed and reverted.
- **FACT:** `out/evidence/1466-r2-individual-corners-on.png` was captured at `HIDIdleTime=835` (`plans/reports/agy-1466-corner-radius-evidence/r2-04b-gui-capture.log:1-13`) and visually verified to show `TL 24` and `TR 4` on the top row and `BL 8` and `BR 16` on the bottom row with a `4 px` (`theme::SPACE_FACT`) gap between each 2-character label and its value.
- **Weakest Link:** When the uniform radius field is bound on a subset of the 4 corner keys (`topLeftRadius`, `topRightRadius`, `bottomRightRadius`, `bottomLeftRadius`), clicking the text area (`x >= rad_r.left() + theme::FIELD_PREFIX_W`, `crates/designos-app/src/panels/properties.rs:1130`) focuses `num_id` to enter text editing while clicking the left icon region (`x < rad_r.left() + theme::FIELD_PREFIX_W`) opens the variable bind picker.

### Round 2 Exact Commands and Pasted Outputs

#### 1. Red Tests on `76a3f9b6` Before Merging `origin/main` (`plans/reports/agy-1466-corner-radius-evidence/r2-00-red-on-76a3f9b6.log:1-87`)

Command:
```bash
HOME="$S/home" TMPDIR="$S/tmp" XDG_CONFIG_HOME="$S/xdg/config" XDG_DATA_HOME="$S/xdg/data" XDG_CACHE_HOME="$S/xdg/cache" XDG_STATE_HOME="$S/xdg/state" cargo test -p designos-app --bin designos --locked corner_radius
```

Output (`plans/reports/agy-1466-corner-radius-evidence/r2-00-red-on-76a3f9b6.log:70-87`):
```text
failures:
    shell::tests::corner_radius::all_four_corner_fields_have_equal_label_to_value_gaps
    shell::tests::corner_radius::bind_only_top_right_turn_toggle_off_and_type_value_detaches_all_four_keys
    shell::tests::corner_radius::component_set_with_imported_geometry_clears_it_on_radius_edit
    shell::tests::corner_radius::mixed_smoothing_draws_no_thumb_and_drag_writes_every_corner_node_in_one_undo_step
    shell::tests::corner_radius::selection_of_128_text_layers_plus_one_frame_shows_radius_row_and_edits_frame
    shell::tests::corner_radius::selection_of_129_rectangles_is_edited_by_one_commit_and_restored_by_one_undo
    shell::tests::corner_radius::star_shows_single_field_only_and_ellipse_and_section_show_no_row
    shell::tests::corner_radius::toggle_on_shows_four_fields_in_figma_order

test result: FAILED. 10 passed; 8 failed; 0 ignored; 0 measured; 4156 filtered out; finished in 2.74s
```

#### 2. Green Null Control & Mutants 1–3 (`r2-01` .. `r2-04`)

Command:
```bash
HOME="$S/home" TMPDIR="$S/tmp" XDG_CONFIG_HOME="$S/xdg/config" XDG_DATA_HOME="$S/xdg/data" XDG_CACHE_HOME="$S/xdg/cache" XDG_STATE_HOME="$S/xdg/state" cargo test -p designos-app --bin designos --locked corner_radius
```

Green Null Control (`plans/reports/agy-1466-corner-radius-evidence/r2-01-green-null-control.log:8-29`):
```text
running 18 tests
test canvas::handles::tests::corner_radius_handle_positions_and_hit_testing ... ok
test check_designs::tests::numbers::uniform_and_per_corner_radius_match_number_variables_and_bind_cleanly ... ok
test canvas::vector::tests::corner_radius_is_one_undo_step_and_rounds_the_derived_path ... ok
test check_designs::tests::under_an_opaque_fill::text_inside_a_rounded_opaque_card_inset_by_corner_radius_is_checked_while_corner_cutout_text_is_skipped ... ok
test shell::tests::vector::the_vector_edit_fixture_shows_the_corner_radius_field ... ok
test shell::tests::corner_radius::all_four_corner_fields_have_equal_label_to_value_gaps ... ok
test shell::tests::corner_radius::component_set_with_imported_geometry_clears_it_on_radius_edit ... ok
test shell::tests::corner_radius::selection_of_128_text_layers_plus_one_frame_shows_radius_row_and_edits_frame ... ok
test shell::tests::corner_radius::toggle_on_shows_four_fields_in_figma_order ... ok
test shell::tests::corner_radius::binding_corner_field_to_variable_resolves_and_survives_save_and_reopen ... ok
test shell::tests::corner_radius::selection_of_129_rectangles_is_edited_by_one_commit_and_restored_by_one_undo ... ok
test shell::tests::corner_radius::mixed_smoothing_draws_no_thumb_and_drag_writes_every_corner_node_in_one_undo_step ... ok
test shell::tests::vector::the_vertex_inspector_edits_the_corner_radius_as_one_undo_step ... ok
test shell::tests::corner_radius::star_shows_single_field_only_and_ellipse_and_section_show_no_row ... ok
test shell::tests::corner_radius::bind_only_top_right_turn_toggle_off_and_type_value_detaches_all_four_keys ... ok
test shell::tests::corner_radius::mixed_selection_shows_mixed_and_edit_writes_every_node ... ok
test shell::tests::corner_radius::smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step ... ok
test shell::tests::corner_radius::per_corner_edit_changes_only_that_corner ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 4177 filtered out; finished in 3.55s
```

Mutant 1 — old `BR`, `BL` bottom-row order (`plans/reports/agy-1466-corner-radius-evidence/r2-02-mutant1-old-br-bl-order.log:16-38`):
```text
thread 'shell::tests::corner_radius::toggle_on_shows_four_fields_in_figma_order' (69428311) panicked at crates/designos-app/src/shell/tests/corner_radius.rs:105:5:
bottom-left is left of bottom-right: [[1472.0 650.0] - [1535.0 682.0]] vs [[1376.0 650.0] - [1439.0 682.0]]
...
test result: FAILED. 17 passed; 1 failed; 0 ignored; 0 measured; 4177 filtered out; finished in 3.30s
```

Mutant 2 — `.take(128)` on `corner_nodes` (`plans/reports/agy-1466-corner-radius-evidence/r2-03-mutant2-take-128.log:16-44`):
```text
thread 'shell::tests::corner_radius::selection_of_128_text_layers_plus_one_frame_shows_radius_row_and_edits_frame' (69434775) panicked at crates/designos-app/src/shell/tests/corner_radius.rs:353:5:
selection of 128 text layers plus one frame must show the radius row
...
thread 'shell::tests::corner_radius::selection_of_129_rectangles_is_edited_by_one_commit_and_restored_by_one_undo' (69434776) panicked at crates/designos-app/src/shell/tests/corner_radius.rs:312:9:
assertion `left == right` failed: rectangle at index 128 must be updated by the uniform radius edit
  left: [6.0, 6.0, 6.0, 6.0]
 right: [18.0, 18.0, 18.0, 18.0]
...
test result: FAILED. 16 passed; 2 failed; 0 ignored; 0 measured; 4177 filtered out; finished in 3.40s
```

Mutant 3 — `NodeKind::Star { .. }` in `node_has_four_corners` (`plans/reports/agy-1466-corner-radius-evidence/r2-04-mutant3-star-four-fields.log:16-38`):
```text
thread 'shell::tests::corner_radius::star_shows_single_field_only_and_ellipse_and_section_show_no_row' (69440755) panicked at crates/designos-app/src/shell/tests/corner_radius.rs:398:5:
star must show the single radius field
...
test result: FAILED. 17 passed; 1 failed; 0 ignored; 0 measured; 4177 filtered out; finished in 3.31s
```

#### 3. GUI Capture (`plans/reports/agy-1466-corner-radius-evidence/r2-04b-gui-capture.log:1-13`)

Commands:
```bash
idle=$(ioreg -c IOHIDSystem | awk '/HIDIdleTime/ {print int($NF/1e9); exit}')
out/cargo/debug/designos "$KIT_ORIG" --select id:44757 --apply-spec /tmp/1466-r2-spec.json --out "$KIT_COPY"
out/cargo/debug/designos "$KIT_COPY" --select "id:210250" --zoom fit --no-toast --screenshot out/evidence/1466-r2-individual-corners-on.png
```

Output (`plans/reports/agy-1466-corner-radius-evidence/r2-04b-gui-capture.log:1-13`):
```text
HIDIdleTime=835
Filesystem        Size    Used   Avail Capacity iused ifree %iused  Mounted on
/dev/disk3s1s1   926Gi    12Gi   125Gi     9%    459k  1.3G    0%   /
apply_spec_rc=0
{
  "root_node_id": 210250,
  "saved": true
}
root_node_id=210250
Filesystem        Size    Used   Avail Capacity iused ifree %iused  Mounted on
/dev/disk3s1s1   926Gi    12Gi   123Gi     9%    459k  1.3G    0%   /
gui_capture_rc=0
-rw-r--r--@ 1 jangtrinh  staff  238246 Nov  2 19:39 out/evidence/1466-r2-individual-corners-on.png
```

#### 4. Gates (`r2-05-fmt.log`, `r2-06-clippy.log`, `r2-07-zero-fake-ui.log`, `r2-08a`, `r2-08b`, `r2-08c`, `r2-09`, `r2-10`, `r2-11`)

`cargo fmt --all -- --check; echo fmt_rc=$?` (`plans/reports/agy-1466-corner-radius-evidence/r2-05-fmt.log:1`):
```text
fmt_rc=0
```

`cargo clippy --workspace --all-targets --locked` (`plans/reports/agy-1466-corner-radius-evidence/r2-06-clippy.log:401-415`):
```text
warning: `designos-app` (bin "designos") generated 2 warnings (run `cargo clippy --fix --bin "designos" -p designos-app` to apply 2 suggestions)
warning: `designos-app` (bin "designos-studio") generated 2 warnings (2 duplicates)
warning: very complex type used. Consider factoring parts into `type` definitions
    --> crates/designos-app/src/shell/tests/inspector_var_binds.rs:1181:21
warning: `designos-app` (bin "designos" test) generated 3 warnings (2 duplicates)
warning: `designos-app` (bin "designos-studio" test) generated 3 warnings (3 duplicates)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 56.17s
clippy_rc=0
```

`python3 scripts/zero_fake_ui_gate.py` (`plans/reports/agy-1466-corner-radius-evidence/r2-07-zero-fake-ui.log:23-26`):
```text
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
zero_fake_ui_rc=0
```

`cargo test -p designos-app --bin designos --locked` — Round 2 Run 1 (`plans/reports/agy-1466-corner-radius-evidence/r2-08a-test-app-bin-run1.log:4960-4968`), Round 2 Run 2 (`plans/reports/agy-1466-corner-radius-evidence/r2-08b-test-app-bin-run2.log:4245-4252`), and Isolated Reruns (`plans/reports/agy-1466-corner-radius-evidence/r2-08c-test-isolated-reruns.log:352-385`):
```text
failures:
    agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root
    vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope
    vecops::tests::two_deferred_text_commands_both_convert_in_order

test result: FAILED. 4179 passed; 3 failed; 13 ignored; 0 measured; 0 filtered out; finished in 382.46s
app_bin_run1_rc=101

failures:
    agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root
    vecops::tests::two_deferred_text_commands_both_convert_in_order

test result: FAILED. 4180 passed; 2 failed; 13 ignored; 0 measured; 0 filtered out; finished in 454.82s
app_bin_run2_rc=101

running 1 test
test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4194 filtered out; finished in 0.00s
approval_root_rc=0

running 1 test
test vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4194 filtered out; finished in 1.49s
vecops_queue_rc=0

running 1 test
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4194 filtered out; finished in 1.49s
vecops_two_rc=0
```

`cargo test --workspace --locked --lib` (`plans/reports/agy-1466-corner-radius-evidence/r2-09-test-workspace-lib.log:2065-2075`):
```text
running 8 tests
test import::tests::summary_toast_uses_singular_nouns_for_one ... ok
test import::tests::blocked_image_placeholder_reported ... ok
test import::tests::css_order_and_flex_margins_and_scale_transform_reported ... ok
test import::tests::block_flow_inference ... ok
test import::tests::flex_maps_to_auto_layout ... ok
test import::tests::one_undo_step ... ok
test import::tests::text_stays_editable ... ok
test import::tests::screenshot_diff_within_threshold ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s

workspace_lib_rc=0
```

`cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` (`plans/reports/agy-1466-corner-radius-evidence/r2-10-test-gpu.log:238-249`):
```text
running 2 tests
test a_two_line_hug_box_holds_its_glyphs_on_the_cpu_and_the_gpu ... ok
test a_hug_box_holds_the_stand_in_glyphs_and_then_the_glyphs_of_the_face_that_arrives ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s

   Doc-tests designos_gpu

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

gpu_rc=0
```

`rm -rf out/cargo && ls out/cargo` (`plans/reports/agy-1466-corner-radius-evidence/r2-11-cleanup.log:1-2`):
```text
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```

`git log --oneline origin/main..HEAD` (`plans/reports/agy-1466-corner-radius-evidence/r2-12-git-log.log:1-4`):
```text
a714fe30 fix(inspector): address round 2 corner radius review rulings
720bfff3 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1466-corner-radius
76a3f9b6 docs(report): record verification report and evidence for #1466
88d8579d feat(inspector): individual corner radius fields and a corner smoothing slider (#1466)
```

Status: DONE
