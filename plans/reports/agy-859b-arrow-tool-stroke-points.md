# Report: Start and End Points in the Stroke Panel, and the Arrow Tool (#859, Slice B)

## 1. What Was NOT Verified and Why

1. **Agent MCP `start_cap` / `end_cap` fields (`crates/designos-mcp`)**:
   - **Why**: Per brief item 3 ("Where `designos_update_node` or `designos_apply_spec` already accepts stroke properties, accept optional `start_cap` and `end_cap` (or the endpoint cap fields your stroke model uses) with the same cap names; otherwise list it as a follow-up in the report"), `ModifyNodeParams` in `crates/designos-mcp/src/tools/modify.rs:14-60` and `SpecNode` in `crates/designos-mcp/src/spec.rs:28-92` do not currently accept stroke or cap fields (`stroke`, `stroke_cap`, `stroke_weight`, `stroke_align`, or `stroke_join`). Adding a full stroke mutation surface to `designos-mcp` is therefore listed as a follow-up rather than widening scope.
2. **Unrelated HTML mockup checks in `scripts/design_os_self_evolve.py` (`G06` / `G09`)**:
   - **Why**: `G01` (`check_rust_token_parity`), `G02` (`zero_fake_ui_gate`), `G03` (`ui_ds_status`), `G04` (`ui_ds_a11y`), `G05` (`ui_ds_soul_check`), `G07` (`design_os_evolution`), and `G08` (`ui_gate_specimen`) all pass (`PASS`). `G06` (`ui_agents_check` warningCount=1) and `G09` (`design_os_project_audit` on pre-existing `plans/261102-project-docs-workspace/ui-mockups/*.html`) are outside our Rust scope.

---

## 2. Acceptance Criteria Table

| Criterion | Command | Pasted Output |
|---|---|---|
| **1. Stroke panel (`crates/designos-app/src/panels/stroke.rs`)**: Start point and end point controls for open paths/lines offering `None`, `Round`, `Square`, `ArrowLines`, `ArrowEquilateral`, `TriangleFilled`, `DiamondFilled`, `CircleFilled`, plus swap of start and end; mixed selections show `"Mixed"` and apply in one undo step; closed paths hide the controls; every change is one undo step and visible live on CPU and GPU | `HOME="$T_HOME" cargo test -p designos-app --bin designos --locked -- panels::stroke::tests::` | `test panels::stroke::tests::miter_angle_roundtrips ... ok`<br>`test panels::stroke::tests::stroke_panel_dash_cap_visibility_mixed_and_multi_selection_single_undo ... ok`<br>`test panels::stroke::tests::stroke_panel_hides_endpoint_controls_for_closed_paths ... ok`<br>`test panels::stroke::tests::stroke_panel_mixed_open_selection_shows_mixed_and_applies_to_all_in_one_undo_step ... ok`<br>`test panels::stroke::tests::stroke_panel_swap_button_exchanges_start_and_end_caps_in_one_undo_step ... ok`<br>`test panels::stroke::tests::stroke_panel_start_and_end_points_set_each_arrow_cap_in_one_undo_step_and_render_cpu_and_gpu ... ok` |
| **2. Arrow tool (`crates/designos-app/src/canvas/tools.rs`, `crates/designos-app/src/canvas/create.rs`, `crates/designos-app/src/panels/menus.rs`, `crates/designos-app/src/shell/tests/menus.rs`)**: `ARROW_TOOL_ENABLED` is `true`, Arrow sits in `GROUPS[2]` after `Line`, shortcut `Shift+L`, draws an open line/vector with `StrokeCap::ArrowLines` at the end point and `StrokeCap::None` at the start point, and saves/loads cleanly | `HOME="$T_HOME" cargo test -p designos-app --bin designos --locked -- canvas::tools::tests:: canvas::create::tests:: panels::menus::tests:: shell::tests::menus::` | `test canvas::tools::tests::arrow_tool_sits_in_shape_group_after_line ... ok`<br>`test canvas::tools::tests::shift_l_selects_arrow_tool ... ok`<br>`test canvas::tools::tests::asking_for_the_arrow_tool_selects_arrow ... ok`<br>`test panels::menus::tests::palette_and_dropdowns_offer_the_arrow_tool ... ok`<br>`test panels::menus::tests::picking_the_arrow_tool_remembers_arrow_for_the_group ... ok`<br>`test canvas::create::tests::existing_arrow_layers_stay_plain_lines_through_save_and_render ... ok`<br>`test shell::tests::menus::shift_l_picks_arrow_and_plain_l_picks_line ... ok`<br>`test shell::tests::menus::shape_dropdown_picks_a_tool_and_remembers_it ... ok` |
| **3. Visual evidence on the Untitled UI kit**: Cloned kit via `cp -c`, captured cropped Stroke panel with start/end point controls and cropped CPU \| GPU canvas render of an Arrow-tool arrow | Headless `Sim` + `HeadlessGpu` render on cloned `out/kit-clone.fig` (`fig:1143:83664`) | `plans/reports/agy-859b-arrow-tool-stroke-points-evidence/stroke-panel-endpoints.png` (`242x240`, `9.8 KB`)<br>`plans/reports/agy-859b-arrow-tool-stroke-points-evidence/arrow-tool-canvas.png` (`379x104`, `1.2 KB`) |
| **4. Layout baseline for 7-row shape menu (`d-menu-shape`)**: Regenerated `crates/designos-app/tests/layout-baseline/d-menu-shape.layout.json` via `DESIGNOS_LAYOUT_BLESS=1` | `HOME="$T_HOME" cargo test -p designos-app --bin designos --locked -- shell::tests::layout_unchanged::d::layout_unchanged_d` | `test shell::tests::layout_unchanged::d::layout_unchanged_d ... ok`<br>`test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4269 filtered out; finished in 7.30s` |

---

## 3. Pre-Edit Inspection, Design Decisions, Assumptions & Edge Cases

### 3.1 REA & Figma UI3 Inspection
- Reused Slice A's REA binary inspection evidence (`ev_0cd5f3f9cd98cd6b00b4abca25c0efd5ef3b4108880aca82eba125aa3cc16c4a` in `plans/reports/agy-859-arrowhead-caps-evidence/rea-figma-macho.json:1-22`) and extracted exact Figma UI3 endpoint strings and byte offsets in `plans/reports/agy-859b-arrow-tool-stroke-points-evidence/rea-figma-strings.log`:
  - `~/Library/Application Support/Figma/DesktopProfile/v43/Code Cache/js/056b3d15fd1334a0_0`:
    - `250080:"left-end-cap-select"`, `250134:stroke_panel.start_point`, `250406:"right-end-cap-select"`, `250459:stroke_panel.end_point`
    - `246331:stroke_panel.terminal_cap.none`, `246393:stroke_panel.terminal_cap.line_arrow`, `246461:stroke_panel.terminal_cap.triangle_arrow`, `246533:stroke_panel.terminal_cap.reversed_triangle`, `246612:stroke_panel.terminal_cap.circle_arrow`, `246682:stroke_panel.terminal_cap.diamond_arrow`, `246753:stroke_panel.terminal_cap.round`, `246816:stroke_panel.terminal_cap.square`
  - `~/Library/Application Support/Figma/DesktopProfile/v43/Code Cache/js/01c31ad7089499b6_0`:
    - `260292:"Start point"`, `260327:"End point"`, `260738:"Line arrow"`, `260829:"Triangle arrow"`, `260675:"Reversed triangle"`, `260582:"Circle arrow"`, `260628:"Diamond arrow"`, `260772:"Round"`, `260800:"Square"`, `260554:"Swap start and end points"`

### 3.2 Per-Vertex Endpoint Caps vs Node Cap
- **FACT**: `designos-render` (`crates/designos-render/src/vectorops.rs:527-544, 738-746`) resolves open-path start and end caps per chain endpoint as:
  - `start_cap = net.vertices.get(start).and_then(|v| v.cap).unwrap_or(n.stroke.cap)`
  - `end_cap = net.vertices.get(end).and_then(|v| v.cap).unwrap_or(n.stroke.cap)`
- **DECISION**:
  - When reading an open node's start and end caps (`open_node_endpoints` in `crates/designos-app/src/panels/stroke.rs:58-77`), `NodeKind::Line` reports `(n.stroke.cap, n.stroke.cap)`, and `NodeKind::Vector` reports `(v_start.cap.unwrap_or(n.stroke.cap), v_end.cap.unwrap_or(n.stroke.cap))` when all open chains in `net.open_chain_ends()` agree on their start cap and end cap (or `None` -> `"Mixed"` when chains disagree).
  - When setting a start or end cap (`set_node_endpoint_cap` in `crates/designos-app/src/panels/stroke.rs:104-121`), `ensure_open_vector_network` promotes `NodeKind::Line` to a 2-vertex horizontal `NodeKind::Vector` (`(0,0) -> (size.x, 0)`, clearing `n.fills` so promoting a line never adds a default fill), Materializes existing `n.stroke.cap` onto both chain endpoints before writing `chosen` onto `s` or `e`, sets `n.stroke.cap = StrokeCap::None`, clears `n.stroke_geometry = None`, and removes `"strokeCap"` and `"strokeCapSource"` from `n.fig_metadata` so live CPU and GPU rendering immediately reflect the change.
  - When swapping start and end points (`swap_node_endpoints` in `crates/designos-app/src/panels/stroke.rs:124-140`), each open chain's `(s, e)` vertex caps are exchanged in one undo step (`ModifyNodes`).

---

## 4. Red / Green / Mutation Falsification Evidence

### 4.1 Red Tests (`plans/reports/agy-859b-arrow-tool-stroke-points-evidence/red-app.log`)
Ran the new unit tests before editing production code (`9 failed`):
```text
failures:
    panels::stroke::tests::stroke_panel_mixed_open_selection_shows_mixed_and_applies_to_all_in_one_undo_step
    panels::stroke::tests::stroke_panel_start_and_end_points_set_each_arrow_cap_in_one_undo_step_and_render_cpu_and_gpu
    panels::stroke::tests::stroke_panel_swap_button_exchanges_start_and_end_caps_in_one_undo_step
test result: FAILED. 3 passed; 3 failed; 0 ignored; 0 measured; 4264 filtered out; finished in 0.03s

failures:
    canvas::tools::tests::arrow_tool_sits_in_shape_group_after_line
    canvas::tools::tests::asking_for_the_arrow_tool_selects_arrow
    canvas::tools::tests::shift_l_selects_arrow_tool
test result: FAILED. 5 passed; 3 failed; 0 ignored; 0 measured; 4262 filtered out; finished in 0.01s

failures:
    canvas::create::tests::existing_arrow_layers_stay_plain_lines_through_save_and_render
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4269 filtered out; finished in 0.02s

failures:
    panels::menus::tests::palette_and_dropdowns_offer_the_arrow_tool
    panels::menus::tests::picking_the_arrow_tool_remembers_arrow_for_the_group
test result: FAILED. 11 passed; 2 failed; 0 ignored; 0 measured; 4257 filtered out; finished in 0.04s
```

### 4.2 Green Tests (`plans/reports/agy-859b-arrow-tool-stroke-points-evidence/green-app.log`)
After implementing production code and merging `#1591` (`9bcef5729`):
```text
running 38 tests
...
test panels::stroke::tests::stroke_panel_hides_endpoint_controls_for_closed_paths ... ok
test panels::stroke::tests::stroke_panel_mixed_open_selection_shows_mixed_and_applies_to_all_in_one_undo_step ... ok
test panels::stroke::tests::stroke_panel_swap_button_exchanges_start_and_end_caps_in_one_undo_step ... ok
test canvas::create::tests::existing_arrow_layers_stay_plain_lines_through_save_and_render ... ok
test panels::stroke::tests::stroke_panel_start_and_end_points_set_each_arrow_cap_in_one_undo_step_and_render_cpu_and_gpu ... ok

test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 4232 filtered out; finished in 0.14s
green_rc=0
```

### 4.3 Mutation Falsification (`plans/reports/agy-859b-arrow-tool-stroke-points-evidence/mutants.log`)
Tested 3 independent production mutants one at a time and reverted each immediately:
1. **Mutant 1 (`set_node_endpoint_cap` writes `n.stroke.cap = chosen` instead of per-vertex cap)**:
   ```text
   thread 'panels::stroke::tests::stroke_panel_start_and_end_points_set_each_arrow_cap_in_one_undo_step_and_render_cpu_and_gpu' panicked at crates/designos-app/src/panels/stroke.rs:577:13:
   assertion `left == right` failed: node cap stays None when setting endpoint cap ArrowLines
     left: ArrowLines
    right: None
   test result: FAILED. 0 passed; 1 failed
   mutant1_rc=101
   ```
2. **Mutant 2 (`swap_node_endpoints` is a no-op)**:
   ```text
   thread 'panels::stroke::tests::stroke_panel_swap_button_exchanges_start_and_end_caps_in_one_undo_step' panicked at crates/designos-app/src/panels/stroke.rs:649:9:
   assertion `left == right` failed: swap exchanges start and end caps
     left: Some((Some(Round), Some(ArrowLines)))
    right: Some((Some(ArrowLines), Some(Round)))
   test result: FAILED. 0 passed; 1 failed
   mutant2_rc=101
   ```
3. **Mutant 3 (`Tool::Arrow` draws `NodeKind::Line` without `ArrowLines`)**:
   ```text
   thread 'canvas::create::tests::existing_arrow_layers_stay_plain_lines_through_save_and_render' panicked at crates/designos-app/src/canvas/create.rs:397:13:
   Arrow tool must create a vector network carrying endpoint caps, got Line
   test result: FAILED. 0 passed; 1 failed
   mutant3_rc=101
   ```

---

## 5. Visual Evidence on Untitled UI Kit & Layout Baseline Diff

### 5.1 Untitled UI Kit Visual Evidence
- Checked `HIDIdleTime` before each capture (`25393s` / `25600s` idle), cloned `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` to `out/kit-clone.fig` via `cp -c`, and removed `out/kit-clone.fig` immediately after:
  1. `plans/reports/agy-859b-arrow-tool-stroke-points-evidence/stroke-panel-endpoints.png` (`242x240`, `9.8 KB`): cropped Stroke panel with open vector `fig:1143:83664` selected in the cloned Untitled UI kit, showing the `Start point` dropdown (`Line arr...`), `End point` dropdown (`Line arr...`), and `Swap end points` icon button.
  2. `plans/reports/agy-859b-arrow-tool-stroke-points-evidence/arrow-tool-canvas.png` (`379x104`, `1.2 KB`): cropped side-by-side CPU | GPU canvas render of an arrow drawn with `Tool::Arrow`, showing `StrokeCap::None` at the start point and `StrokeCap::ArrowLines` at the end point on both renderers.

### 5.2 `d-menu-shape.layout.json` Before/After Rectangles (`plans/reports/agy-859b-arrow-tool-stroke-points-evidence/d-menu-shape-baseline-diff.log`)
Regenerated `crates/designos-app/tests/layout-baseline/d-menu-shape.layout.json` via `DESIGNOS_LAYOUT_BLESS=1`:
- `overlay:Foreground/0`: `[699.0, 824.0, 246.0, 208.0]` -> `[699.0, 792.0, 246.0, 240.0]`
- `menu-level-0`: `[698.5, 824.0, 246.0, 208.0]` -> `[698.5, 792.0, 246.0, 240.0]`
- `menu-check-0-0`: `[698.5, 832.0, 246.0, 32.0]` -> `[698.5, 800.0, 246.0, 32.0]`
- `menu-check-0-6`: added `[0.0, 0.0, 0.0, 0.0]`
- `menu-row-0-0`: `[698.5, 832.0, 246.0, 32.0]` -> `[698.5, 800.0, 246.0, 32.0]`
- `menu-row-0-1`: `[698.5, 864.0, 246.0, 32.0]` -> `[698.5, 832.0, 246.0, 32.0]`
- `menu-row-0-2`: `[698.5, 896.0, 246.0, 32.0]` -> `[698.5, 864.0, 246.0, 32.0]`
- `menu-row-0-3`: `[698.5, 928.0, 246.0, 32.0]` -> `[698.5, 896.0, 246.0, 32.0]`
- `menu-row-0-4`: `[698.5, 960.0, 246.0, 32.0]` -> `[698.5, 928.0, 246.0, 32.0]`
- `menu-row-0-5`: `[698.5, 992.0, 246.0, 32.0]` -> `[698.5, 960.0, 246.0, 32.0]`
- `menu-row-0-6`: added `[698.5, 992.0, 246.0, 32.0]`

---

## 6. Verification Gates, Commits & Cleanup

### 6.1 Python UI & Formatting Gates (`plans/reports/agy-859b-arrow-tool-stroke-points-evidence/gates-python-fmt.log`)
```text
=== zero_fake_ui_gate.py ===
 Gate Verdict: PASS (17/17 invariants satisfied)
zero_fake_ui_rc=0
=== design_os_self_evolve.py ===
G01  | PASS   | check_rust_token_parity  | 35/35 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; purple-band hits: 0
G02  | PASS   | zero_fake_ui_gate        | 17/17 Zero-Fake-UI invariants PASS (DIR-28 / BR-31 / REQ-25)
G03  | PASS   | ui_ds_status             | DS manifest active (78 registered components, hash=sha256-SkbvSo4GPRiD…)
G04  | PASS   | ui_ds_a11y               | supplementary: 15 foreground/surface pairs + 8 state pairs + 50 explicit chrome pairs (design/a11y-chrome-pairs.txt; failures=0, unresolved=0)
G05  | PASS   | ui_ds_soul_check         | 0 soul findings; design/soul.md status=ratified
G07  | PASS   | design_os_evolution      | verdict=ALIVE, proof.level=IMPROVING, findings=0
G08  | PASS   | ui_gate_specimen         | 6/6 families PASS (errors=0, warnings=0); ds-usage-lint errors=0
=== check_spacing_relationships.py ===
PASS: no count rose above the baseline
spacing_rc=0
=== cargo fmt --all -- --check ===
fmt_rc=0
```

### 6.2 Cargo Gates (`plans/reports/agy-859b-arrow-tool-stroke-points-evidence/gates-cargo.log` & `gate-app-full.log`)
```text
=== cargo clippy --workspace --all-targets --locked ===
clippy_rc=0
=== cargo test -p designos-doc --locked ===
test result: ok. 747 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.55s
doc_rc=0
=== cargo test -p designos-mcp --locked ===
test result: ok. 327 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.61s
mcp_rc=0
=== cargo test --workspace --locked --lib ===
lib_rc=0
=== cargo test -p designos-app --bin designos --locked ===
test result: FAILED. 4255 passed; 2 failed; 13 ignored; 0 measured; 0 filtered out; finished in 396.96s
=== Solo rerun of 2 tests from gate-app-full.log with short non-temp HOME (/Users/jangtrinh/dh.y8P3ML) ===
running 2 tests
test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 4268 filtered out; finished in 1.50s
app_rerun_rc=0
```

### 6.3 Cleanup Proof (`ls out/cargo`), Draft PR & Commits
```text
$ ls out/cargo
ls: out/cargo: No such file or directory
```
- **Draft PR**: https://github.com/jangtrinh/design-os-studio/pull/1592
- **Commits (`git log --oneline 9bcef5729..HEAD`)**:
  - `a0b3b437c feat(strokes): start and end points in the stroke panel, and the Arrow tool (#859)`

Status: COMPLETED
