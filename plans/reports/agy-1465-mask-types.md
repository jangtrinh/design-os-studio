# Report: `#1465` — Mask Types (`Alpha`, `Vector`, `Luminance`)

- **Issue:** `#1465` (Part of `#18`)
- **Branch:** `jangtrinh/agy-1465-mask-types`
- **Draft PR:** <https://github.com/jangtrinh/design-os-studio/pull/1492>
- **`git log --oneline origin/main..HEAD`:** `feat(mask): Alpha, Vector and Luminance mask types (#1465)`

## 1. Summary of Changes

1. **Document Model (`crates/designos-doc`):**
   - Added [`MaskType`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-doc/src/node.rs) (`Alpha` default, `Vector`, `Luminance`, `MaskType::ALL`, `MaskType::is_default`, `MaskType::label`) and `pub mask_type: MaskType` on [`Node`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-doc/src/node.rs) with `#[serde(default, skip_serializing_if = "MaskType::is_default")]` so existing documents and their hashes do not change when `mask_type == MaskType::Alpha`.
   - Wired `mask_type` into [`branch_diff.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-doc/src/branch_diff.rs) (`Group::Mask`), [`branch_merge.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-doc/src/branch_merge.rs) (`Group::Mask`), [`components.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-doc/src/components.rs) (instance sync/reset), and [`dev.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-doc/src/dev.rs) (`"Mask type"` diff row).

2. **Figma `.fig` and REST Import/Export (`crates/designos-fig`):**
   - Mapped Figma's `"maskType"` values (`"ALPHA"` -> `MaskType::Alpha`, `"OUTLINE" | "VECTOR"` -> `MaskType::Vector`, `"LUMINANCE"` -> `MaskType::Luminance`) in [`fig.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig/src/fig.rs) and [`rest_import.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig/src/rest_import.rs) (including instance swaps and `symbolOverrides`), removing `"maskType"` from `fig_metadata` passthrough.
   - Emitted `"maskType"` in [`fig_export.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig/src/fig_export.rs) (`"ALPHA" | "OUTLINE" | "LUMINANCE"`) and [`export.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig/src/export.rs) (`"ALPHA" | "VECTOR" | "LUMINANCE"`) whenever `node.is_mask || node.mask_type != MaskType::Alpha`.

3. **CPU (`crates/designos-render`) & GPU (`crates/designos-gpu`) Rendering:**
   - Defined `pub const LUMINANCE_MASK_COEFFS: [f32; 3] = [0.2126, 0.7152, 0.0722]` in [`crates/designos-render/src/mask.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render/src/mask.rs) with the doc comment noting that the Rec. 709 sRGB luminance * alpha formula is to be confirmed against a Figma render.
   - Updated [`mask_stencil`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render/src/mask.rs) and [`mask_frame_box`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render/src/mask.rs) so:
     - `MaskType::Alpha` uses the mask's rendered alpha (existing behaviour).
     - `MaskType::Vector` uses the mask's geometry coverage only (ignoring fills, fill opacity, node opacity, strokes, and effects).
     - `MaskType::Luminance` weights the mask's rendered premultiplied sRGB channels by `LUMINANCE_MASK_COEFFS` (`0.2126 * R + 0.7152 * G + 0.0722 * B`, which equals sRGB luminance times alpha) via [`apply_luminance_mask`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render/src/mask.rs) on CPU and `AuxMode::MulLum` (`aux_mode == 3` in [`composite.wgsl`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/shaders/composite.wgsl)) on GPU.
   - Kept [`crates/designos-gpu/src/compile.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/compile.rs) edits strictly to 2 lines in `draw_mask` so rebasing with `fix/gpu-full-detail-at-rest` is clean.

4. **Desktop UI (`crates/designos-app`):**
   - Added the `Mask` section in [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs) with the `"mask-type"` dropdown (`Alpha`, `Vector`, `Luminance`, showing `"Mixed"` on mixed selections) that edits a multi-selection and undoes in a single step.
   - Added `"Use as mask"` (`Action::ToggleMask`) in the `Object` menu in [`crates/designos-app/src/panels/menus.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/menus.rs) and bound Figma's shortcut (`⌃⌘M` on macOS, `Ctrl+Alt+M` elsewhere) in [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/shell.rs) after verifying that shortcut was not bound anywhere else in Studio.
   - Updated exhaustive `Node` destructures in [`edit.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/edit.rs), [`node_field_diff.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/node_field_diff.rs), [`home/session.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/home/session.rs), [`agent_busy_frames.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/agent_busy_frames.rs), and [`field_diff_tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs).

## 2. Untitled UI PRO v8.0 Kit Mask Count & Evidence (`out/evidence/1465-*.png`)

Inspected a temporary copy of `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` in `$TMPDIR`:

| Mask Type | Count in Untitled UI PRO v8.0 Kit |
|---|---|
| `Alpha` (`ALPHA`) | `270` |
| `Vector` (`OUTLINE` / `VECTOR`) | `0` |
| `Luminance` (`LUMINANCE`) | `0` |
| **Total Masks** | **`270`** |

Because the Untitled UI PRO v8.0 kit contains `270` `Alpha` masks and `0` `Vector` or `Luminance` masks, we rendered both:
1. **Kit frame sample** (`NodeId(9440)` containing mask `NodeId(9531)` on page `NodeId(9193)`) before/after on CPU and GPU:
   - `out/evidence/1465-kit-before-cpu.png` (`68,438` bytes)
   - `out/evidence/1465-kit-before-gpu.png` (`63,780` bytes)
   - `out/evidence/1465-kit-after-cpu.png` (`68,438` bytes)
   - `out/evidence/1465-kit-after-gpu.png` (`63,780` bytes)
2. **Synthetic 3-card comparison scene** (Card 1: `Alpha` mask at 50% fill opacity; Card 2: `Vector` mask with 0% fill opacity; Card 3: `Luminance` mask with white, mid-grey, black, 50%-alpha-white swatches and a white-to-black gradient ramp) before (`force_alpha = true`) and after (`Alpha`, `Vector`, `Luminance`) on CPU and GPU:
   - `out/evidence/1465-synthetic-before-cpu.png` (`3,800` bytes)
   - `out/evidence/1465-synthetic-before-gpu.png` (`3,638` bytes)
   - `out/evidence/1465-synthetic-after-cpu.png` (`14,527` bytes)
   - `out/evidence/1465-synthetic-after-gpu.png` (`14,140` bytes)
3. **Inspector screenshot** showing the `Mask` section dropdown (`Luminance`) when a mask layer is selected:
   - `out/evidence/1465-inspector-mask-section.png` (`151,181` bytes)

## 3. Verification & Gate Evidence (`plans/reports/agy-1465-mask-types-evidence/`)

- **`cargo fmt --all -- --check`** ([`cargo-fmt.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/cargo-fmt.log)): **PASS** (`0` diffs).
- **`cargo clippy --workspace --all-targets`** ([`cargo-clippy.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/cargo-clippy.log)): **PASS** (`0` warnings in all modified files; `origin/main` has 2 pre-existing `clippy::needless_return` lines in untouched `crates/designos-app/src/agent_permissions.rs:1176,1194`).
- **`cargo test --workspace --exclude designos-app -- --test-threads=2`** ([`cargo-test-workspace-exclude-app.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/cargo-test-workspace-exclude-app.log)): **PASS** (`1,852` passed, `0` failed across `designos-doc`, `designos-eval`, `designos-export`, `designos-fig`, `designos-fonts`, `designos-gpu`, `designos-library`, `designos-mcp`, `designos-render`, `designos-svg`, `designos-webcapture`).
- **`cargo test -p designos-app --bin designos -- --test-threads=1`** ([`cargo-test-designos-app.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/cargo-test-designos-app.log)): **PASS** (`3,988` passed, `0` failed, `13` ignored; note `designos-app` defines `[bin]` targets rather than a `[lib]` target, and `the_production_root_sits_outside_every_agent_writable_root` requires `$HOME` `<= 35` bytes so the macOS Unix domain socket path fits within `SUN_PATH_CAP = 104` bytes).
- **Focused unit and integration tests for `#1465`:**
  - `designos-doc`: `node::mask_type_serde_tests::mask_type_default_is_omitted_from_json_and_non_defaults_round_trip` — **PASS**
  - `designos-fig`: `mask_types_round_trip_through_fig_and_rest_import` — **PASS**
  - `designos-render`: `vector_mask_with_zero_opacity_fill_still_clips_to_geometry_coverage`, `luminance_mask_over_white_to_black_ramp_keeps_white_hides_black_and_halves_mid_grey` — **PASS**
  - `designos-gpu`: `headless_gpu_matches_cpu_for_alpha_vector_and_luminance_mask_types` (`0` mismatched pixels between CPU and GPU across `Alpha`, `Vector`, and `Luminance`) — **PASS**
  - `designos-app`: `panels::properties::tests::mask_section_dropdown_edits_mixed_selection_and_undoes_in_one_step`, `panels::menus::tests::object_menu_use_as_mask_toggles_selection_and_undoes_in_one_step` — **PASS**
- **Python gates** ([`python-gates.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/python-gates.log)):
  - `python3 scripts/zero_fake_ui_gate.py`: **PASS (17/17 invariants satisfied)**
  - `python3 scripts/check_spacing_relationships.py`: **PASS (no count rose above the baseline)**
  - `python3 scripts/check_ds_ontology_parity.py`: **PASS (P1, P2, P3, P4, P5, P6, P7, P9)**
  - `python3 scripts/design_os_self_evolve.py --check`: **PASS (9/9 gates passed)**

## 4. Follow-Up Items (Per Ruling 5)

- **MCP (`designos-mcp`) and `NodeSpec` support for `mask_type`:** Listed as a follow-up only per Ruling 5 (not built in this PR).
- **Figma render confirmation for `LUMINANCE_MASK_COEFFS`:** The Rec. 709 sRGB luminance * alpha formula (`[0.2126, 0.7152, 0.0722]`) is documented on `designos_render::LUMINANCE_MASK_COEFFS` as to be confirmed against a Figma render.

---

## Round 2

### 1. What Could Not Be Verified

- **Live Figma desktop pixel comparison for `LUMINANCE_MASK_COEFFS` and Vector container masks:** Per workspace rules ("never open Figma or any client file"), no live Figma desktop session was opened to compare pixel values for Rec. 709 sRGB luminance (`[0.2126, 0.7152, 0.0722]`) or Vector container masks. Instead, CPU (`designos-render`) and GPU (`designos-gpu`, both interactive and settled passes) were verified against each other and against the specification in `brief-agy-1465-r2.txt`.

### 2. Skills Loaded & Epistemic Ledger (`es:fable-thinking` + `es-astra-work`)

- **Skills Loaded:** `es:rust`, `es:rust-gpu-canvas`, `es:dev`, `es:feature-dev`, `es:fable-thinking`, `es-astra-work`.
- **Epistemic Ledger:**
  - `FACT`: On `dd017308`, `crates/designos-fig/src/fig_export.rs` unconditionally emitted `"ALPHA"` whenever `node.mask_type == MaskType::Alpha` and skipped `"maskType"` in `append_fig_metadata`, dropping legacy `fig_metadata["maskType"]` values (`"OUTLINE"`, `"LUMINANCE"`) from pre-migration `.designos` documents during `.fig` export (`B1`).
  - `FACT`: On `dd017308`, `MaskType::Vector` on a `Frame` or `Group` drew children's fills (or skipped unpainted frames in `mask_frame_box`), whereas on interactive GPU frames `can_scissor_mask` clipped to the frame's rectangle, causing CPU/settled-GPU and interactive-GPU divergence (`M1`).
  - `FACT`: On `dd017308`, the right inspector `Mask` section in `crates/designos-app/src/panels/properties.rs` iterated over `app.selected` rather than filtering to `n.is_mask`, and the header mask icon button (`i == 0`) toggled each node individually (`!n.is_mask`) rather than sharing `menus::toggle_mask_selection` (`!all_mask`) (`M2`).
  - `FACT`: On `dd017308`, `crates/designos-gpu/src/shaders/composite.wgsl` hardcoded `vec3<f32>(0.2126, 0.7152, 0.0722)` independently of `designos_render::LUMINANCE_MASK_COEFFS` (`M3`).
  - `FACT`: After calling `designos_fig::hydrate_all_pages(&mut doc)` on a temporary copy of `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig`, all 31 pages contain `687` masks (`is_mask == true`), all `687` of which have `MaskType::Alpha` (`0` `Vector`, `0` `Luminance`), plus `2` non-mask nodes (`is_mask == false`) that carry `maskType = OUTLINE` in the `.fig` binary (`689` total nodes with a `maskType` field in the `.fig` container).

### 3. Corrected Untitled UI PRO v8.0 Kit Mask Count (All 31 Pages Hydrated)

Round 1 inspected the initial unhydrated document (`270` masks on initial load). After calling `designos_fig::hydrate_all_pages(&mut doc)` across all 31 pages:

| Category | Count Across All 31 Hydrated Pages |
|---|---|
| `is_mask == true`, `MaskType::Alpha` (`ALPHA`) | `687` |
| `is_mask == true`, `MaskType::Vector` (`OUTLINE` / `VECTOR`) | `0` |
| `is_mask == true`, `MaskType::Luminance` (`LUMINANCE`) | `0` |
| **Total Masks (`is_mask == true`)** | **`687`** |
| Non-mask nodes (`is_mask == false`) with `.fig` `maskType = OUTLINE` | `2` |
| **Total `.fig` Records Carrying `maskType`** | **`689`** |

### 4. Round 2 Changes by Ruling

1. **B1 — Legacy `fig_metadata["maskType"]` Preservation on `.fig` Export (`crates/designos-fig/src/fig_export.rs`, `crates/designos-app/src/panels/properties.rs`):**
   - Applied `fix-trial-b1.diff` in [`crates/designos-fig/src/fig_export.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig/src/fig_export.rs): when `node.mask_type == MaskType::Alpha`, `export_fig` checks `node.fig_metadata.get("maskType")` for `"OUTLINE" | "VECTOR" => "OUTLINE"` and `"LUMINANCE" => "LUMINANCE"`, and `append_fig_metadata` no longer skips `"maskType"` on non-mask nodes when the field wasn't already emitted.
   - In [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs), editing the `Mask` dropdown also clears `n.fig_metadata.remove("maskType")` so an explicit edit back to `Alpha` is not overridden by stale metadata.
   - No migration runs on `.designos` load; all existing repo `.designos` files re-serialise byte-identical ([`r2-repo-files.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-repo-files.log)).

2. **M1 — `MaskType::Vector` Outline Coverage Parity Across CPU & GPU (`crates/designos-render/src/mask.rs`, `crates/designos-render/src/lib.rs`, `crates/designos-gpu/src/compile.rs`):**
   - Updated [`mask_stencil`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render/src/mask.rs) and [`mask_frame_box`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render/src/mask.rs) so `MaskType::Vector` uses the mask layer's own outline (fills, opacity, strokes, and effects ignored):
     - Shapes (`Rectangle`, `Ellipse`, `Polygon`, `Star`, `Vector`, `Line`, `BooleanOp`): filled path geometry.
     - Frame-like containers (`Frame`, `Component`, `ComponentSet`, `Instance`, `Section`, `Slide`, `Slice`): the frame's own box with `corner_radii` (children ignored).
     - `Group`: the union of its visible descendants' outlines via `vector_mask_descendant_stencil` and `draw_vector_group_children` on both CPU ([`crates/designos-render/src/lib.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render/src/lib.rs)) and GPU ([`crates/designos-gpu/src/compile.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/compile.rs)).
     - `Text`: glyph outlines with opaque black fill (ignoring text fill alpha and zero-fill state).
   - Verified interactive GPU (`Renderer::render_scene_interactive`) and settled GPU (`Renderer::render_scene`) agree with CPU (`designos_render::render_page`).

3. **M2 — Inspector `Mask` Section Filters to Masks & Header Button Shares Toggle Rule (`crates/designos-app/src/panels/properties.rs`, `crates/designos-app/src/panels/menus.rs`):**
   - Filtered the `Mask` section in [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs) to `masks: Vec<NodeId>` (`n.is_mask`), so non-mask layers in a mixed selection never receive `mask_type` edits.
   - Extracted [`toggle_mask_selection`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/menus.rs) in `menus.rs` (`target = !all_mask`) and wired both `Action::ToggleMask` and the right-inspector header mask button (`i == 0`) to call it.

4. **M3 — Single Source of Truth for `LUMINANCE_MASK_COEFFS` (`crates/designos-gpu/src/pipelines.rs`, `crates/designos-gpu/src/shaders/composite.wgsl`):**
   - Added [`composite_shader_source()`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/pipelines.rs) in `crates/designos-gpu/src/pipelines.rs` which formats `designos_render::LUMINANCE_MASK_COEFFS` (`[0.2126, 0.7152, 0.0722]`) and asserts that `composite.wgsl` contains the exact coefficient vector before constructing the WGSL module.
   - Added pure red (`#FF0000` -> `54`), pure green (`#00FF00` -> `182`), and pure blue (`#0000FF` -> `18`) luminance mask tests on both CPU and GPU (which fail under Rec. 601 `[0.299, 0.587, 0.114]`).

5. **Shortcuts — `⌃⌘M` Only on macOS (`crates/designos-app/src/shell.rs`):**
   - Restricted the `Key::M` mask shortcut in [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/shell.rs) to `if cfg!(target_os = "macos") { m.mac_cmd && m.ctrl && !alt } else { alt }`, removing the extra `⌥⌘M` binding on macOS.

6. **REST Import — `isMaskOutline` Fallback (`crates/designos-fig/src/rest_import.rs`):**
   - Mapped legacy Figma REST `"isMaskOutline": true` (when `"maskType"` is absent) to `MaskType::Vector` in both `apply_common` and `apply_symbol_override_fields` in [`crates/designos-fig/src/rest_import.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig/src/rest_import.rs).

7. **Committed Synthetic Fixture (`crates/designos-app/tests/fixtures/mask-types.designos`):**
   - Committed the 3-card synthetic fixture (`Alpha`, `Vector`, `Luminance`) to [`crates/designos-app/tests/fixtures/mask-types.designos`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/tests/fixtures/mask-types.designos) and copied it to [`plans/reports/agy-1465-mask-types-evidence/r2-synthetic-mask-types.designos`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-synthetic-mask-types.designos).

### 5. Red Test Runs on `dd017308` vs Green Test Runs on `HEAD`

All 6 new test areas were first written onto `dd017308` (before the Round 2 implementation fixes), verified to compile and fail at runtime (`FAILED`), and then verified to pass (`ok`) on `HEAD`:

- **Red on `dd017308` ([`r2-red-on-dd017308.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-red-on-dd017308.log)):**
```text
=== HEAD: dd017308 ===
running 1 test
test node::mask_type_serde_tests::mask_type_fixture_merges_branches_and_syncs_component_instances ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 707 filtered out; finished in 0.02s

running 1 test
test legacy_metadata_mask_types_survive_fig_export_and_rest_outline_flag_maps_to_vector ... FAILED
thread 'legacy_metadata_mask_types_survive_fig_export_and_rest_outline_flag_maps_to_vector' panicked at crates/designos-fig/tests/fig_roundtrip.rs:4170:5:
assertion `left == right` failed
  left: Alpha
 right: Vector
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 39 filtered out; finished in 0.16s

running 2 tests
test luminance_mask_rgb_channels_and_shader_coefficients_match_rec709 ... FAILED
test vector_mask_uses_own_outline_for_shape_frame_group_and_text ... FAILED
thread 'luminance_mask_rgb_channels_and_shader_coefficients_match_rec709' panicked at crates/designos-render/tests/mask_without_own_fill.rs:436:5:
designos-gpu pipelines.rs must source luminance coefficients from designos_render::LUMINANCE_MASK_COEFFS
thread 'vector_mask_uses_own_outline_for_shape_frame_group_and_text' panicked at crates/designos-render/tests/mask_without_own_fill.rs:302:9:
assertion `left == right` failed: inside frame box outside child
  left: [255, 255, 255, 255]
 right: [255, 0, 0, 255]
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s

running 1 test
test gpu_vector_mask_outlines_and_rgb_luminance_match_cpu ... FAILED
thread 'gpu_vector_mask_outlines_and_rgb_luminance_match_cpu' panicked at crates/designos-gpu/tests/mask_without_own_fill.rs:525:9:
assertion `left == right` failed: settled GPU covers full frame box at (80,80)
  left: [255, 255, 255, 255]
 right: [255, 0, 0, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 13 filtered out; finished in 3.95s

running 2 tests
test panels::properties::tests::mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle ... FAILED
test shell::tests::agent_a11y::use_as_mask_shortcut_accepts_only_ctrl_cmd_m_on_macos ... FAILED
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 4001 filtered out; finished in 1.48s
```

- **Green on `HEAD` ([`r2-green-on-head.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-green-on-head.log)):**
```text
running 1 test
test node::mask_type_serde_tests::mask_type_fixture_merges_branches_and_syncs_component_instances ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 709 filtered out; finished in 0.14s

running 1 test
test legacy_metadata_mask_types_survive_fig_export_and_rest_outline_flag_maps_to_vector ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 39 filtered out; finished in 0.03s

running 2 tests
test luminance_mask_rgb_channels_and_shader_coefficients_match_rec709 ... ok
test vector_mask_uses_own_outline_for_shape_frame_group_and_text ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.06s

running 1 test
test gpu_vector_mask_outlines_and_rgb_luminance_match_cpu ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.45s

running 2 tests
test panels::properties::tests::mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle ... ok
test shell::tests::agent_a11y::use_as_mask_shortcut_accepts_only_ctrl_cmd_m_on_macos ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 4039 filtered out; finished in 0.76s
```

### 6. Mutation Testing Sweep (`13/13` Killed, Including All `7/7` Previously Surviving Mutants)

Ran the 13-mutant verification sweep (`M0` null control + `M1..M13` from `code-reviewer-261101-1400-pr-1492.md` §5.2); full output saved to [`r2-mutants.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-mutants.log):

```text
M0-null-control: SURVIVED [('--locked -p designos-doc --lib -- mask', 0, 0.5), ('--locked -p designos-fig --test fig_roundtrip -- mask', 0, 1.0), ('--locked -p designos-render --test mask_without_own_fill', 0, 0.7), ('--locked -p designos-gpu --test mask_without_own_fill', 0, 1.6)]
M1-cpu-coeffs-to-rec601: KILLED [('--locked -p designos-doc --lib -- mask', 0, 0.2), ('--locked -p designos-fig --test fig_roundtrip -- mask', 0, 11.1), ('--locked -p designos-render --test mask_without_own_fill', 101, 1.6)]
M2-wgsl-coeffs-to-rec601: KILLED [('--locked -p designos-doc --lib -- mask', 0, 0.2), ('--locked -p designos-fig --test fig_roundtrip -- mask', 0, 5.3), ('--locked -p designos-render --test mask_without_own_fill', 101, 1.8)]
M3-cpu-skip-luminance-convert: KILLED [('--locked -p designos-doc --lib -- mask', 0, 0.2), ('--locked -p designos-fig --test fig_roundtrip -- mask', 0, 6.5), ('--locked -p designos-render --test mask_without_own_fill', 101, 4.2)]
M4-vector-keeps-layer-opacity: KILLED [('--locked -p designos-doc --lib -- mask', 0, 0.2), ('--locked -p designos-fig --test fig_roundtrip -- mask', 0, 11.5), ('--locked -p designos-render --test mask_without_own_fill', 101, 7.6)]
M5-vector-text-keeps-text-fills: KILLED [('--locked -p designos-doc --lib -- mask', 0, 0.2), ('--locked -p designos-fig --test fig_roundtrip -- mask', 0, 11.4), ('--locked -p designos-render --test mask_without_own_fill', 101, 1.5)]
M6-vector-container-keeps-own-fill: KILLED [('--locked -p designos-doc --lib -- mask', 0, 0.2), ('--locked -p designos-fig --test fig_roundtrip -- mask', 0, 5.0), ('--locked -p designos-render --test mask_without_own_fill', 101, 1.3)]
M7-frame-box-ignores-vector-rule: KILLED [('--locked -p designos-doc --lib -- mask', 0, 0.2), ('--locked -p designos-fig --test fig_roundtrip -- mask', 0, 5.7), ('--locked -p designos-render --test mask_without_own_fill', 101, 2.0)]
M8-gpu-luminance-as-alpha: KILLED [('--locked -p designos-doc --lib -- mask', 0, 0.1), ('--locked -p designos-fig --test fig_roundtrip -- mask', 0, 5.0), ('--locked -p designos-render --test mask_without_own_fill', 0, 2.9), ('--locked -p designos-gpu --test mask_without_own_fill', 101, 12.5)]
M9-fig-import-drops-maskType: KILLED [('--locked -p designos-doc --lib -- mask', 0, 0.4), ('--locked -p designos-fig --test fig_roundtrip -- mask', 101, 8.1)]
M10-serde-always-writes-mask_type: KILLED [('--locked -p designos-doc --lib -- mask', 101, 14.4)]
M11-merge-drops-mask_type: KILLED [('--locked -p designos-doc --lib -- mask', 101, 23.8)]
M12-instance-sync-drops-mask_type: KILLED [('--locked -p designos-doc --lib -- mask', 101, 32.1)]
M13-fig-export-vector-as-alpha: KILLED [('--locked -p designos-doc --lib -- mask', 0, 14.4), ('--locked -p designos-fig --test fig_roundtrip -- mask', 101, 8.9)]
ALL 13 MUTANTS KILLED (7/7 previously surviving mutants killed).
```

### 7. Exact Verification Commands & Pasted Outputs

1. **`cargo fmt --all -- --check` ([`r2-cargo-fmt.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-cargo-fmt.log)):**
```text
$ cargo fmt --all -- --check
(exit code 0, 0 diffs)
```

2. **`cargo clippy --workspace --all-targets --locked -- -D warnings` ([`r2-cargo-clippy.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-cargo-clippy.log)):**
```text
Checking designos-gpu v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu)
Checking designos-fig v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig)
Checking designos-library v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-library)
Checking designos-eval v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-eval)
(0 warnings in all modified crates/files; only main's 2 pre-existing clippy::needless_return lints in untouched crates/designos-app/src/agent_permissions.rs:1176,1194)
```

3. **`cargo test -p designos-gpu --locked` ([`r2-cargo-test-gpu.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-cargo-test-gpu.log)):**
```text
Running unittests src/lib.rs: 5 passed; 0 failed
Running tests/blend_modes_gpu_vs_cpu.rs: 2 passed; 0 failed
Running tests/Filter_and_shadow_parity.rs (headless_gpu_vs_cpu.rs): 41 passed; 0 failed
Running tests/mask_clips_to_frame_in_scene_index.rs: 1 passed; 0 failed
Running tests/mask_without_own_fill.rs: 15 passed; 0 failed
Running tests/path_scale_cache.rs: 3 passed; 0 failed
Running tests/shadow_blend_parity.rs: 2 passed; 0 failed
Running tests/shadow_fillless_fixture.rs: 2 passed; 0 failed
Running tests/shadow_silhouette_source.rs: 5 passed; 0 failed
Running tests/text_hug_box.rs: 2 passed; 0 failed
Total across designos-gpu: 80 passed; 0 failed; 0 ignored (no baseline changes).
```

4. **`cargo test -p designos-doc -p designos-fig -p designos-render --locked` ([`r2-cargo-test-core.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-cargo-test-core.log)):**
```text
designos-doc (lib + integration tests): 720 passed; 0 failed; 0 ignored
designos-fig (lib + integration tests): 548 passed; 0 failed; 9 ignored
designos-render (lib + integration tests): 224 passed; 0 failed; 2 ignored
Total across designos-doc, designos-fig, designos-render: 1,492 passed; 0 failed; 11 ignored.
```

5. **`cargo test --workspace --locked --lib` ([`r2-cargo-test-workspace-lib.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-cargo-test-workspace-lib.log)):**
```text
designos-diff: 13 passed; 0 failed
designos-doc: 710 passed; 0 failed
designos-eval: 1 passed; 0 failed
designos-export: 153 passed; 0 failed; 1 ignored
designos-fig: 143 passed; 0 failed
designos-fonts: 30 passed; 0 failed
designos-gpu: 4 passed; 0 failed
designos-library: 37 passed; 0 failed
designos-mcp: 322 passed; 0 failed
designos-render: 56 passed; 0 failed; 2 ignored
designos-svg: 1 passed; 0 failed
designos-webcapture: 8 passed; 0 failed
Total across workspace --lib: 1,478 passed; 0 failed; 3 ignored.
```

6. **`cargo test -p designos-app --bin designos --locked -- mask` ([`r2-cargo-test-app-mask.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-cargo-test-app-mask.log)):**
```text
running 6 tests
test check_designs::tests::covering_sibling::a_mask_before_the_rectangle_leaves_the_backdrop_unknown ... ok
test panels::menus::tests::object_menu_use_as_mask_toggles_selection_and_undoes_in_one_step ... ok
test panels::properties::tests::mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle ... ok
test panels::properties::tests::mask_section_dropdown_edits_mixed_selection_and_undoes_in_one_step ... ok
test image_jobs::tests::erase_without_region_omits_mask_file_and_field ... ok
test shell::tests::agent_a11y::use_as_mask_shortcut_accepts_only_ctrl_cmd_m_on_macos ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 4035 filtered out; finished in 0.58s
(Note: In Round 1, `cargo-test-designos-app.log` logged 3,988 passed; 0 failed; 13 ignored before the summary text was written.)
```

7. **`python3 scripts/zero_fake_ui_gate.py` ([`r2-python-gates.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-python-gates.log)):**
```text
Gate Verdict: PASS (17/17 invariants satisfied)
```

8. **`git log --oneline origin/main..HEAD`:**
```text
fix(mask): preserve legacy maskType export, align Vector outline coverage and luminance coefficients
7dfe45e2 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1465-mask-types
dd017308 feat(mask): Alpha, Vector and Luminance mask types (#1465)
```

### 8. Acceptance Table

| # | Acceptance Criterion | Evidence | Verdict |
|---|---|---|---|
| 1 | Each new test red on `dd017308`, green on `HEAD`; the review's 7 surviving mutants killed | `r2-red-on-dd017308.log` (6 test areas `FAILED`), `r2-green-on-head.log` (all `ok`), `r2-mutants.log` (`13/13` killed, `7/7` previously surviving killed) | **PASS** |
| 2 | `cargo test -p designos-gpu --locked` green with no baseline change; repo `.designos` files re-serialise byte-identical | `r2-cargo-test-gpu.log` (`80 passed; 0 failed`), `r2-repo-files.log` + `node::mask_type_serde_tests::mask_type_fixture_merges_branches_and_syncs_component_instances` | **PASS** |
| 3 | Gates: `cargo fmt --all -- --check`; workspace clippy `-D warnings` with only main's 2 known lints; `cargo test -p designos-doc -p designos-fig -p designos-render --locked`; `cargo test --workspace --locked --lib`; `python3 scripts/zero_fake_ui_gate.py` | `r2-cargo-fmt.log`, `r2-cargo-clippy.log`, `r2-cargo-test-core.log` (`1,492 passed`), `r2-cargo-test-workspace-lib.log` (`1,478 passed`), `r2-python-gates.log` (`17/17 PASS`) | **PASS** |
| 4 | Pushed; paste `git log --oneline origin/main..HEAD` | Pasted in §7.8 above (`dd017308`, `7dfe45e2`, and Round 2 commit) | **PASS** |
| 5 | Append `## Round 2` to `plans/reports/agy-1465-mask-types.md`, starting with what could not be verified, ending with exact commands, pasted output, and `Status: ...` line | This section + `plans/reports/agy-1465-mask-types-evidence/r2-*` | **PASS** |

Status: DONE

