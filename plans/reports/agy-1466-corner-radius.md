# DEV Report · #1466 Individual Corner Radius Fields and Corner Smoothing Slider

## What Could NOT Be Verified

- Full-suite parallel run of `vecops::tests::two_deferred_text_commands_both_convert_in_order` inside the 4,167-test `cargo test -p designos-app --bin designos --locked` invocation timed out waiting for the shared `"SlowvecFnt1"` mock font (`plans/reports/agy-1466-corner-radius-evidence/07-test-designos-app-bin.log:4186`, `plans/reports/agy-1466-corner-radius-evidence/07-test-designos-app-bin.log:4201-4208`) due to a pre-existing parallel font-registration contention across 5 tests in `crates/designos-app/src/vecops.rs:1470-2013`. It was verified in isolation with the same test harness in `1.48s` (`1 passed; 0 failed`, `plans/reports/agy-1466-corner-radius-evidence/07b-test-vecops-isolated.log:10-12`).
- `designos-gpu`'s `large_file_compile_benchmarks` test in `crates/designos-gpu/tests/large_file_perf.rs` was intentionally skipped per the common brief (`-- --skip large_file_compile_benchmarks`, `plans/reports/agy-1466-corner-radius-evidence/09-test-designos-gpu.log:119-123`) so the client benchmark file is never opened.

---

## 1. Re-anchor (`(file:line)` citations)

1. **Per-corner radius fields and `corner_smoothing` on `Node` (`designos-doc`):**
   - `Node::corner_radii: [f32; 4]` is stored at `crates/designos-doc/src/node.rs:97` in `[top-left, top-right, bottom-right, bottom-left]` order (`crates/designos-doc/src/node.rs:96`).
   - `Node::corner_smoothing: f32` is stored at `crates/designos-doc/src/node.rs:100` (`0.0` = circular arc, `0.6` = Figma `"iOS"` squircle, `1.0` = maximum smoothing, `crates/designos-doc/src/node.rs:98-99`), serialized with `#[serde(default, skip_serializing_if = "is_zero")]` (`crates/designos-doc/src/node.rs:99`).
   - Helper accessors on `Node`: `Node::corner_radius(&self) -> f32` returns `self.corner_radii[0]` (`crates/designos-doc/src/node.rs:514-516`), and `Node::with_radius(mut self, r: f32) -> Self` sets `self.corner_radii = [r; 4]` (`crates/designos-doc/src/node.rs:479-482`).

2. **Inspector radius control and what it writes (`designos-app`):**
   - On base commit `53860a64`, the Appearance section in `crates/designos-app/src/panels/properties.rs:1058-1076` checked `has_radius = sel.iter().any(|n| crate::edit::node_has_corners(n))` (`crates/designos-app/src/edit.rs:76-89`), read `r0 = crate::edit::uniform_corner_radius(first)` (`crates/designos-app/src/edit.rs:91-98`) and `r_mixed` (`crates/designos-app/src/edit.rs:100-102`), rendered a single `var_bind::numeric` field (`"radius"`, bound to `["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"]`), and on edit called `app.edit_nodes(&ids, |n| { if crate::edit::node_has_corners(n) { n.corner_radii = [v; 4]; crate::edit::clear_rect_geom(n); } })`.
   - With `#1466` implemented (`crates/designos-app/src/panels/properties.rs:1058-1214`), the Appearance section adds:
     - The Individual corners toggle (`"corner-individual"`, `crates/designos-app/src/panels/properties.rs:1098-1108`), whose default state follows the selection (`default_individual = r_mixed || any_per_corner_bound`, `crates/designos-app/src/panels/properties.rs:1066-1077`) and whose per-selection override is stored in egui temp view state (`egui::Id::new("inspector-corner-individual-state")`, `crates/designos-app/src/panels/properties.rs:1079-1089`), never in `Document`.
     - When Individual corners is on, a 2×2 grid of four `var_bind::numeric` fields in Figma's order (`("radius-tl", "TL", "topLeftRadius", 0)`, `("radius-tr", "TR", "topRightRadius", 1)`, `("radius-br", "BR", "bottomRightRadius", 2)`, `("radius-bl", "BL", "bottomLeftRadius", 3)`, `crates/designos-app/src/panels/properties.rs:1111-1140`), where each field edits only `n.corner_radii[corner_idx] = v` (`crates/designos-app/src/panels/properties.rs:1132-1135`) on selected corner-bearing nodes and calls `crate::edit::clear_rect_geom(n)`.
     - The Corner smoothing row (`crates/designos-app/src/panels/properties.rs:1142-1212`), shown when `has_radius` is true: a `0..=100%` slider (`"corner-smoothing-slider"`, `crates/designos-app/src/panels/properties.rs:1150-1187`) using `app.ensure_scrub(ui.ctx())` (`crates/designos-app/src/ops.rs:298-343`) so a press-to-release drag records one undo step, a `0..=100%` numeric field (`"corner-smoothing"`, `crates/designos-app/src/panels/properties.rs:1189-1197`), and an `"iOS"` preset button (`"corner-smoothing-ios"`, `crates/designos-app/src/panels/properties.rs:1199-1211`) writing `n.corner_smoothing = 0.6`.

3. **Alt-drag per-corner canvas edit (`designos-app`):**
   - Canvas corner-radius handles are hit-tested and drawn in `crates/designos-app/src/canvas/handles.rs:25-120` (`supports_corner_radius_handles` at `crates/designos-app/src/canvas/handles.rs:25-30`, `corner_handle_positions` at `crates/designos-app/src/canvas/handles.rs:34-61`, `hit_corner_handle` at `crates/designos-app/src/canvas/handles.rs:64-73`).
   - Dragging a handle starts `Interaction::CornerRadius(CornerRadiusDrag { id, corner, initial_radii })` at `crates/designos-app/src/canvas/interact.rs:843-847` (after `self.snapshot()` for single-step undo at `crates/designos-app/src/canvas/interact.rs:844`).
   - During the drag (`crates/designos-app/src/canvas/interact.rs:1148-1166`), when `inp.mods.alt` is held (`crates/designos-app/src/canvas/interact.rs:1157`), only the dragged corner is updated (`let mut r = cr.initial_radii; r[cr.corner] = new_r; r`, `crates/designos-app/src/canvas/interact.rs:1158-1160`); without Alt, all four corners receive `[new_r; 4]` (`crates/designos-app/src/canvas/interact.rs:1162`). Releasing commits via `finish_drag_History` (`crates/designos-app/src/canvas/interact.rs:1617-1619`).

4. **How `corner_smoothing` is imported (`.fig`, REST) and rendered (CPU, GPU):**
   - **`.fig` Kiwi import & export (`designos-fig`):** `crates/designos-fig/src/fig.rs:929` and `crates/designos-fig/src/fig.rs:1576` copy `"cornerSmoothing"` from the Kiwi record into REST-shaped JSON, and `crates/designos-fig/src/fig.rs:1577-1593` maps `rectangleCornerRadiiIndependent` and `rectangleTopLeftCornerRadius`..`rectangleBottomLeftCornerRadius` into `rectangleCornerRadii`. Export writes `corner_smoothing` back to `"cornerSmoothing"` at `crates/designos-fig/src/fig_export.rs:964-965`.
   - **REST JSON import & export (`designos-fig`):** `crates/designos-fig/src/rest_import.rs:1311-1317` populates `n.corner_radii` from `rectangleCornerRadii` or `cornerRadius`, and `crates/designos-fig/src/rest_import.rs:1318` sets `n.corner_smoothing = fnum(v, "cornerSmoothing").unwrap_or(0.0)`. REST export serializes `cornerSmoothing` at `crates/designos-fig/src/export.rs:522-523`.
   - **CPU rendering (`designos-render`):** `crates/designos-render/src/geom.rs:121-210` (`SmoothCorner::new` and `smooth_rounded_rect`) builds Figma's cubic-Bezier + circular-arc smooth corner path whenever `smoothing > 0.0` (called from `node_shape` at `crates/designos-render/src/geom.rs:441`). `crates/designos-render/src/lib.rs:564`, `crates/designos-render/src/lib.rs:893`, and `crates/designos-render/src/outline.rs:224` check `n.corner_smoothing == 0.0` before taking circular fast paths.
   - **GPU rendering (`designos-gpu`):** `crates/designos-gpu/src/geom.rs:158` includes `n.corner_smoothing.to_bits()` in `ShapeKey`, `crates/designos-gpu/src/geom.rs:201` and `crates/designos-gpu/src/geom.rs:214` cache `corner_smoothing` on `NodeGeom` and tessellate `designos_render`'s smooth corner path via Lyon, and `crates/designos-gpu/src/compile.rs:1418` bypasses the analytic SDF drop-shadow quad when `radii[0] > 0.0 && n.corner_smoothing > 0.0` so smoothed corners use the silhouette blur path.

5. **How a radius field binds a variable today (`designos-app` & `designos-doc`):**
   - `var_bind::numeric` (`crates/designos-app/src/panels/var_bind.rs:514-552`) inspects `pill_field(app, &[&keys[0]])` (`crates/designos-app/src/panels/var_bind.rs:517`); when bound, it draws `bound_pill` (`crates/designos-app/src/panels/var_bind.rs:518-534`); when unbound, it reserves `ICON_W` (`18.0 px`, `crates/designos-app/src/panels/var_bind.rs:342`) on the right of the numeric field (`crates/designos-app/src/panels/var_bind.rs:536-545`) and draws the always-visible variable bind button (`"bind:<key>"`) via `hover_button` (`crates/designos-app/src/panels/var_bind.rs:394-441`), which opens `BindPicker` (`crates/designos-app/src/panels/var_bind.rs:431-438`).
   - Selecting a variable in `BindPicker` calls `bind_keys` (`crates/designos-app/src/panels/var_bind.rs:647-661`), writing `node.variables.bindings.insert(k, var_id)` for every key in `keys` (`["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"]` for the uniform field, or `["topLeftRadius"]` / `["topRightRadius"]` / `["bottomRightRadius"]` / `["bottomLeftRadius"]` for the individual corner fields) and calling `app.doc.apply_Variables()`.
   - `crates/designos-doc/src/variables_apply.rs:35`, `crates/designos-doc/src/variables_apply.rs:70`, `crates/designos-doc/src/variables_apply.rs:92`, and `crates/designos-doc/src/variables_apply.rs:406-416` resolve `"topLeftRadius"`, `"topRightRadius"`, `"bottomRightRadius"`, and `"bottomLeftRadius"` onto `node.corner_radii[0..4]`.

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
| 4c | `cargo test -p designos-app --bin designos --locked` | PASS (`4153 passed` in full run + isolated `vecops::tests::two_deferred_text_commands_both_convert_in_order` `1 passed; 0 failed`) | `plans/reports/agy-1466-corner-radius-evidence/07-test-designos-app-bin.log:4208`, `plans/reports/agy-1466-corner-radius-evidence/07b-test-vecops-isolated.log:12` |
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

Status: DONE
