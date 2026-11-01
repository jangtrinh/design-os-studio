# Report: `#1465` — Mask Types (`Alpha`, `Vector`, `Luminance`)

- **Issue:** `#1465` (Part of `#18`)
- **Branch:** `jangtrinh/agy-1465-mask-types`
- **Draft PR:** <https://github.com/jangtrinh/design-os-studio/pull/1492>
- **`git log --oneline origin/main..HEAD`:**
  - `e4ff8103 fix(mask): inject LUMINANCE_MASK_COEFFS token in WGSL, bound mask inspector scan and expand mask tests`
  - `04654004 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1465-mask-types`
  - `0d652a1e fix(mask): preserve legacy maskType export, align Vector outline coverage and luminance coefficients`
  - `7dfe45e2 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1465-mask-types`
  - `dd017308 feat(mask): Alpha, Vector and Luminance mask types (#1465)`

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
   - Kept [`crates/designos-gpu/src/compile.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/compile.rs) edits minimal in `draw_mask` so rebasing with `fix/gpu-full-detail-at-rest` is clean.

4. **Desktop UI (`crates/designos-app`):**
   - Added the `Mask` section in [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs) with the `"mask-type"` dropdown (`Alpha`, `Vector`, `Luminance`, showing `"Mixed"` on mixed selections) that edits a multi-selection and undoes in a single step.
   - Added `"Use as mask"` (`Action::ToggleMask`) in the `Object` menu in [`crates/designos-app/src/panels/menus.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/menus.rs) and bound Figma's shortcut (`⌃⌘M` on macOS, `Ctrl+Alt+M` elsewhere) in [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/shell.rs) after verifying that shortcut was not bound anywhere else in Studio.
   - Updated exhaustive `Node` destructures in [`edit.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/edit.rs), [`node_field_diff.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/node_field_diff.rs), [`home/session.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/home/session.rs), [`agent_busy_frames.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/agent_busy_frames.rs), and [`field_diff_tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs).

## 2. Untitled UI PRO v8.0 Kit Mask Count & Evidence (`out/evidence/1465-*.png`)

Round 1 inspected the initial unhydrated load (`270` masks on initial load). Across all 31 hydrated pages of `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (`designos_fig::hydrate_all_pages`), the verified census is:

| Category | Count Across All 31 Hydrated Pages |
|---|---|
| `is_mask == true`, `MaskType::Alpha` (`maskType` field absent in `.fig`) | `687` |
| `is_mask == true`, `MaskType::Vector` (`OUTLINE` / `VECTOR`) | `0` |
| `is_mask == true`, `MaskType::Luminance` (`LUMINANCE`) | `0` |
| **Total Masks (`is_mask == true`)** | **`687` (none carry a `maskType` field)** |
| Non-mask nodes (`is_mask == false`) carrying `maskType = OUTLINE` | `2` |

Because the Untitled UI PRO v8.0 kit contains `687` `Alpha` masks and `0` `Vector` or `Luminance` masks, we rendered both:
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
- **`cargo test -p designos-app --bin designos -- --test-threads=1`** ([`cargo-test-designos-app.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/cargo-test-designos-app.log)): Line 4037 of `cargo-test-designos-app.log` recorded `test result: FAILED. 3987 passed; 1 failed; 13 ignored` (the 1 failure was `agent_permissions::tests::the_production_root_sits_outside_every_agent_writable_root` when `$HOME` and `$TMPDIR` were nested; with sibling `$HOME` and `$TMPDIR` roots, that test passes).
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
  - `FACT`: On `dd017308`, `MaskType::Vector` on a `Frame` or `Group` drew children's fills (or skipped frames with own fill in `mask_frame_box`), whereas on interactive GPU frames `SceneCompiler::mask_uses_frame_box` stood in with the frame's rectangle only for unpainted frames, causing CPU/settled-GPU and interactive-GPU divergence (`M1`).
  - `FACT`: On `dd017308`, the right inspector `Mask` section in `crates/designos-app/src/panels/properties.rs` iterated over `app.selected` rather than filtering to `n.is_mask`, and the header mask icon button (`i == 0`) set every selected node to `!n0.is_mask` (`let on = n0.is_mask; app.edit_nodes(sel, |n| n.is_mask = !on)`) rather than sharing `menus::toggle_mask_selection` (`!all_mask`) (`M2`).
  - `FACT`: On `dd017308`, `crates/designos-gpu/src/shaders/composite.wgsl` hardcoded `vec3<f32>(0.2126, 0.7152, 0.0722)` independently of `designos_render::LUMINANCE_MASK_COEFFS` (`M3`).
  - `FACT`: Across all 31 hydrated pages of `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (per reviewer census on `main`), `687` masks (`is_mask == true`) carry no `maskType` field in the `.fig` binary (`meta=absent`, defaulting to `MaskType::Alpha`), and `2` non-mask nodes (`is_mask == false`) carry `maskType = OUTLINE`.

### 3. Corrected Untitled UI PRO v8.0 Kit Mask Count (All 31 Pages Hydrated)

| Category | Count Across All 31 Hydrated Pages |
|---|---|
| `is_mask == true`, `MaskType::Alpha` (`maskType` field absent in `.fig`) | `687` |
| `is_mask == true`, `MaskType::Vector` (`OUTLINE` / `VECTOR`) | `0` |
| `is_mask == true`, `MaskType::Luminance` (`LUMINANCE`) | `0` |
| **Total Masks (`is_mask == true`)** | **`687` (none carry a `maskType` field)** |
| Non-mask nodes (`is_mask == false`) with `.fig` `maskType = OUTLINE` | `2` |

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
   - Verified interactive GPU (`HeadlessGpu::render` with `compiler.cull = true` and `compiler.set_build_budget(Some(Duration::ZERO))`) and settled GPU (`compiler.cull = false` and `compiler.set_build_budget(None)`) agree at `(80, 80)` for a `MaskType::Vector` frame mask with a deferred vector child, and that settled GPU matches CPU (`designos_render::render_page`) across shape, frame, group, and text `MaskType::Vector` cases.

3. **M2 — Inspector `Mask` Section Filters to Masks & Header Button Shares Toggle Rule (`crates/designos-app/src/panels/properties.rs`, `crates/designos-app/src/panels/menus.rs`):**
   - Filtered the `Mask` section in [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs) to `masks: Vec<NodeId>` (`n.is_mask`), so non-mask layers in a mixed selection never receive `mask_type` edits.
   - Extracted [`toggle_mask_selection`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/menus.rs) in `menus.rs` (`target = !all_mask`) and wired both `Action::ToggleMask` and the right-inspector header mask button (`i == 0`) to call it.

4. **M3 — `LUMINANCE_MASK_COEFFS` in `pipelines.rs` & Chromatic Luminance Tests (`crates/designos-gpu/src/pipelines.rs`, `crates/designos-gpu/src/shaders/composite.wgsl`):**
   - Added [`composite_shader_source()`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/pipelines.rs) in `crates/designos-gpu/src/pipelines.rs` (note: in Round 2 `composite.wgsl` still held the literal coefficients with an `assert!`, which was fixed in Round 3 Ruling 1 to use the `LUMINANCE_MASK_COEFFS` token in `composite.wgsl` with no `assert!`).
   - Added pure red (`#FF0000` -> `54`), pure green (`#00FF00` -> `182`), and pure blue (`#0000FF` -> `18`) luminance mask tests on both CPU and GPU (which fail under Rec. 601 `[0.299, 0.587, 0.114]`).

5. **Shortcuts — `⌃⌘M` Only on macOS (`crates/designos-app/src/shell.rs`):**
   - Restricted the `Key::M` mask shortcut in [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/shell.rs) to `if cfg!(target_os = "macos") { m.mac_cmd && m.ctrl && !alt } else { alt }`, removing the extra `⌥⌘M` binding on macOS.

6. **REST Import — `isMaskOutline` Fallback (`crates/designos-fig/src/rest_import.rs`):**
   - Mapped legacy Figma REST `"isMaskOutline": true` (when `"maskType"` is absent) to `MaskType::Vector` in both `apply_common` and `apply_symbol_override_fields` in [`crates/designos-fig/src/rest_import.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig/src/rest_import.rs).

7. **Committed Synthetic Fixture (`crates/designos-app/tests/fixtures/mask-types.designos`):**
   - Committed the 3-card synthetic fixture (`Alpha`, `Vector`, `Luminance`) to [`crates/designos-app/tests/fixtures/mask-types.designos`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/tests/fixtures/mask-types.designos) and copied it to [`plans/reports/agy-1465-mask-types-evidence/r2-synthetic-mask-types.designos`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-synthetic-mask-types.designos).

### 5. Red Test Runs on `dd017308` vs Green Test Runs on `HEAD`

Of the 7 tests run on `dd017308` before Round 2 implementation fixes, **5 failed on the Round 2 behaviour under test** (`legacy_metadata_mask_types_survive_fig_export_and_rest_outline_flag_maps_to_vector`, `vector_mask_uses_own_outline_for_shape_frame_group_and_text`, `gpu_vector_mask_outlines_and_rgb_luminance_match_cpu`, `mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle`, and `use_as_mask_shortcut_accepts_only_ctrl_cmd_m_on_macos`), whereas **2 are mutant killers rather than behavioural reds on `dd017308`**:
- `node::mask_type_serde_tests::mask_type_fixture_merges_branches_and_syncs_component_instances` failed on `dd017308` only because `crates/designos-app/tests/fixtures/mask-types.designos` was not yet created (it kills mutants `r1M11` and `r1M12`).
- `luminance_mask_rgb_channels_and_shader_coefficients_match_rec709` failed on `dd017308` on its `pipelines_rs.contains` grep before reaching the pixel assertions (it kills mutant `r1M1`; the grep was removed in Round 3).

- **Red on `dd017308` ([`r2-red-on-dd017308.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-red-on-dd017308.log)):**
```text
=== HEAD: dd017308 ===
running 1 test
test node::mask_type_serde_tests::mask_type_fixture_merges_branches_and_syncs_component_instances ... FAILED
thread 'node::mask_type_serde_tests::mask_type_fixture_merges_branches_and_syncs_component_instances' panicked at crates/designos-doc/src/node.rs:1280:69:
read mask-types.designos fixture: Os { code: 2, kind: NotFound, message: "No such file or directory" }
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

- **Green on `0d652a1e` ([`r2-green-on-head.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-green-on-head.log)):**
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
Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-5ccf888a59871cb9)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/adapter.rs (out/cargo/debug/deps/adapter-c66eec4aeb2e3017)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
Running tests/command_buffer_limit.rs (out/cargo/debug/deps/command_buffer_limit-3fed03efd0561bf5)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.76s
Running tests/fidelity.rs (out/cargo/debug/deps/fidelity-cc261f5d4fa3c7d5)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.02s
Running tests/large_file_perf.rs (out/cargo/debug/deps/large_file_perf-0192ee8bff0db85c)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.17s
Running tests/mask_without_own_fill.rs (out/cargo/debug/deps/mask_without_own_fill-2ce90af7d717be90)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
Running tests/program_cap.rs (out/cargo/debug/deps/program_cap-11a0c449a68ab83f)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.41s
Running tests/refine_vram_budget.rs (out/cargo/debug/deps/refine_vram_budget-b0aaa3dfa8401165)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.72s
Running tests/resized_shapes.rs (out/cargo/debug/deps/resized_shapes-417154c778d81844)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s
Running tests/shadow_blend_parity.rs (out/cargo/debug/deps/shadow_blend_parity-5608cba68a554e4b)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.50s
Running tests/shadow_fillless_fixture.rs (out/cargo/debug/deps/shadow_fillless_fixture-344513bf6ab15bc9)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.54s
Running tests/shadow_silhouette_source.rs (out/cargo/debug/deps/shadow_silhouette_source-8a1801511554a0e2)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.46s
Running tests/text_hug_box.rs (out/cargo/debug/deps/text_hug_box-2731001cf15a434c)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
Total across designos-gpu: 80 passed; 0 failed; 0 ignored.
```

4. **`cargo test -p designos-doc -p designos-fig -p designos-render --locked` ([`r2-cargo-test-core.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-cargo-test-core.log)):**
```text
designos-doc (710 lib + 10 integration tests): 720 passed; 0 failed; 0 ignored
designos-fig (143 lib + 350 integration tests): 493 passed; 0 failed; 6 ignored
designos-render (56 lib + 223 integration tests): 279 passed; 0 failed; 5 ignored
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
(Note: In Round 1, line 4037 of `cargo-test-designos-app.log` logged `test result: FAILED. 3987 passed; 1 failed; 13 ignored` — the 1 failure was `the_production_root_sits_outside_every_agent_writable_root` under a nested `$HOME`/`$TMPDIR` harness.)
```

7. **`python3 scripts/zero_fake_ui_gate.py` ([`r2-python-gates.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r2-python-gates.log)):**
```text
Gate Verdict: PASS (17/17 invariants satisfied)
```

8. **`git log --oneline origin/main..0d652a1e`:**
```text
0d652a1e fix(mask): preserve legacy maskType export, align Vector outline coverage and luminance coefficients
7dfe45e2 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1465-mask-types
dd017308 feat(mask): Alpha, Vector and Luminance mask types (#1465)
```

### 8. Acceptance Table

| # | Acceptance Criterion | Evidence | Verdict |
|---|---|---|---|
| 1 | Each new test red on `dd017308` (5 behavioural reds + 2 mutant killers), green on `0d652a1e`; the review's 7 surviving mutants killed | `r2-red-on-dd017308.log`, `r2-green-on-head.log`, `r2-mutants.log` (`13/13` killed, `7/7` previously surviving killed) | **PASS** |
| 2 | `cargo test -p designos-gpu --locked` green with no baseline change; repo `.designos` files re-serialise byte-identical | `r2-cargo-test-gpu.log` (`80 passed; 0 failed`), `r2-repo-files.log` + `node::mask_type_serde_tests::mask_type_fixture_merges_branches_and_syncs_component_instances` | **PASS** |
| 3 | Gates: `cargo fmt --all -- --check`; workspace clippy `-D warnings` with only main's 2 known lints; `cargo test -p designos-doc -p designos-fig -p designos-render --locked`; `cargo test --workspace --locked --lib`; `python3 scripts/zero_fake_ui_gate.py` | `r2-cargo-fmt.log`, `r2-cargo-clippy.log`, `r2-cargo-test-core.log` (`1,492 passed`), `r2-cargo-test-workspace-lib.log` (`1,478 passed`), `r2-python-gates.log` (`17/17 PASS`) | **PASS** |
| 4 | Pushed; paste `git log --oneline origin/main..HEAD` | Pasted in §7.8 above (`0d652a1e`, `7dfe45e2`, `dd017308`) | **PASS** |
| 5 | Append `## Round 2` to `plans/reports/agy-1465-mask-types.md`, starting with what could not be verified, ending with exact commands, pasted output, and `Status: ...` line | This section + `plans/reports/agy-1465-mask-types-evidence/r2-*` | **PASS** |

---

## Round 3

### 1. What Could Not Be Verified

- **Live Figma desktop pixel comparison for `LUMINANCE_MASK_COEFFS` and edge-case Vector group/stroke semantics (`Q1`–`Q3`):** Per workspace rules ("never open Figma or any client file"), no live Figma desktop session was opened to compare pixel values for Rec. 709 sRGB luminance (`[0.2126, 0.7152, 0.0722]`), overflowing children of non-clipping frames inside Vector groups (`Q1`), nested masks inside Vector groups (`Q2`), or strokes on Vector masks (`Q3`).
- **Physical human visual inspection of a live interactive macOS window:** All visual renders in Round 3 (`r3-proof-sheet-cpu-gpu-per-type.png`) were generated via headless CPU (`tiny-skia`) and headless Metal GPU (`wgpu` on Apple M4 Max) and inspected programmatically and via `read_file`.

### 2. Skills Loaded & What Changed Because of Them

- **`es:rust` & `es:rust-gpu-canvas`:**
  - Replaced the Round 2 `assert!`-and-literal-replace in [`crates/designos-gpu/src/pipelines.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/pipelines.rs) with true token substitution (`"LUMINANCE_MASK_COEFFS"` in [`crates/designos-gpu/src/shaders/composite.wgsl`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/shaders/composite.wgsl) replaced by `format!("vec3<f32>({r:?}, {g:?}, {b:?})")` from `designos_render::LUMINANCE_MASK_COEFFS` with no runtime `assert!`), eliminating a release-build panic path in `Pipelines::new` and ensuring any change to `designos_render::LUMINANCE_MASK_COEFFS` propagates directly into the compiled Metal shader.
  - Bounded the per-frame right-inspector header scan (`sel.iter().take(128).all(...)`) and `Mask` section scan (`sel.iter().take(128).copied().filter(...)`) in [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs) so selecting 100k layers does not perform 100k hash lookups per frame while the inspector is open; full `masks` collection happens only inside `if let Some(i) = dropdown(...)` when the user picks a menu item.
- **`es:fable-thinking` & `es:astra-work`:**
  - Falsified each Round 3 test on `0d652a1e` (`r3-red-on-0d652a1e.log`) or against its targeted mutant (`r3-mutants.log`, starting with `M0-null-control`) before declaring any test effective.
  - Identified why `N9` survived in Round 2: in `draw_vector_group_children` (`crates/designos-render/src/lib.rs:429`), a hidden leaf child is also skipped inside `draw_node`, whereas for a **nested `Group`** (`matches!(stencil_child.kind, NodeKind::Group)`), `draw_vector_group_children` recurses directly into `&stencil_child.children` without calling `draw_node`; adding a hidden nested `Group` containing a visible leaf child kills `N9` on both CPU and GPU.
  - Verified every number, file name, symbol name, and `git log` line in Round 1, Round 2, and Round 3 against raw command logs (`R1`–`R6`).
- **`es:dev` & `es:feature-dev`:**
  - Kept the scope strictly to Rulings 1–6 in `brief-agy-1465-r3.txt` and verified all workspace gates with sibling `HOME` and `TMPDIR` directories.

### 3. Epistemic Ledger & Weakest Link

| Kind | Statement | Basis |
|---|---|---|
| `FACT` | `composite.wgsl` holds `let m = dot(aux.rgb, LUMINANCE_MASK_COEFFS);` and `composite_shader_source()` in `pipelines.rs` replaces `"LUMINANCE_MASK_COEFFS"` with `format!("vec3<f32>({r:?}, {g:?}, {b:?})")` from `designos_render::LUMINANCE_MASK_COEFFS` with no `assert!` | [`composite.wgsl:193`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/shaders/composite.wgsl#L193), [`pipelines.rs:241-245`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/pipelines.rs#L241-L245), unit test `pipelines::tests::composite_shader_injects_luminance_mask_coeffs_from_render_constant` |
| `FACT` | Setting `designos_render::LUMINANCE_MASK_COEFFS` to Rec. 601 `[0.299, 0.587, 0.114]` does not panic in `Pipelines::new` and instead changes GPU luminance output, failing `gpu_vector_mask_outlines_and_rgb_luminance_match_cpu` with `gpu pure red: expected 54, got 76` | [`r3-rec601-red.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-rec601-red.log) |
| `FACT` | All 5 Round 2 surviving mutants (`N4`, `N8`, `N9`, `N11`, `N15`) plus `N16` (`.fig` `symbolOverrides` `isMaskOutline`) and `N17` (`take(128)` Mask section bound) are killed after `M0-null-control` passes (`25/25` non-equivalent mutants killed; `N2` is equivalent because `fig_export.rs` falls through to `n.fig_metadata`) | [`r3-mutants.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-mutants.log) |
| `FACT` | The per-frame right-inspector header scan (`properties.rs:854`) and `Mask` section scan (`properties.rs:1043`) are bounded with `.take(128)`, and the full `masks` vector is collected only inside `if let Some(i) = dropdown(...)` when an option is chosen | [`properties.rs:854`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs#L854), [`properties.rs:1043-1058`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs#L1043-L1058), killed mutant `N17-mask-section-unbounded-scan` |
| `FACT` | The regenerated CPU/GPU per-type proof sheet on `HEAD` (`r3-proof-sheet-cpu-gpu-per-type.png`) and probe log (`r3-probe-log.txt`) have `0` pixels difference (`ImageChops.difference` bbox `None`) and `0` lines diff against Round 2 (`proof-sheet-r2-cpu-gpu-per-type.png` and `head-r2-probe-log.txt`) | [`r3-proof-sheet-cpu-gpu-per-type.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-proof-sheet-cpu-gpu-per-type.png), [`r3-probe-log.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-probe-log.txt) |
| `ASSUMPTION` | Bounding the per-frame inspector `Mask` section and header scan to the first `128` selected nodes matches the existing `common` helper (`take(128)`) while full-selection edits on dropdown selection apply to all selected masks | Ruling 3 (`brief-agy-1465-r3.txt`) |
| `UNKNOWN` | Figma's exact rendering for `Q1` (non-clipping frame with overflowing child inside a Vector group mask), `Q2` (nested mask inside a Vector group mask), and `Q3` (strokes on a Vector mask) | Deferred per Ruling 5 (`brief-agy-1465-r3.txt`) |

- **Weakest Link:** If a user selects `> 128` nodes where the first `128` nodes are non-masks (`is_mask == false`) and a mask node sits at index `>= 128`, the bounded `.take(128)` per-frame scan hides the `Mask` section in the right inspector (matching how `common(app, sel, ...)` caps at `128` throughout `properties.rs`). This is explicitly tested by the 129-node selection assertion in `mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle`.

### 4. Round 3 Changes by Ruling

1. **Ruling 1 (Ruling 4 done for real — `crates/designos-gpu/src/shaders/composite.wgsl`, `crates/designos-gpu/src/pipelines.rs`, `crates/designos-render/tests/mask_without_own_fill.rs`):**
   - Replaced the hardcoded literal in [`crates/designos-gpu/src/shaders/composite.wgsl:193`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/shaders/composite.wgsl#L193) with `let m = dot(aux.rgb, LUMINANCE_MASK_COEFFS);`.
   - Updated [`composite_shader_source()`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/pipelines.rs#L241-L245) in `crates/designos-gpu/src/pipelines.rs` to replace `"LUMINANCE_MASK_COEFFS"` with `&format!("vec3<f32>({r:?}, {g:?}, {b:?})")` from `designos_render::LUMINANCE_MASK_COEFFS`, with no `assert!` in `Pipelines::new` or `composite_shader_source()`.
   - Added unit test [`pipelines::tests::composite_shader_injects_luminance_mask_coeffs_from_render_constant`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/src/pipelines.rs#L273-L289) verifying that the raw shader template contains `"LUMINANCE_MASK_COEFFS"`, the processed source has no `"LUMINANCE_MASK_COEFFS"` token left, and the processed source contains `vec3<f32>(0.2126, 0.7152, 0.0722)`.
   - Removed the `wgsl.contains` and `pipelines_rs.contains` source greps from `luminance_mask_rgb_channels_and_shader_coefficients_match_rec709` in [`crates/designos-render/tests/mask_without_own_fill.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render/tests/mask_without_own_fill.rs#L427-L429).

2. **Ruling 2 (`m3` — Kill Surviving Mutants `N4`, `N8`, `N9`, `N11`, `N15`):**
   - **`N4` (REST & `.fig` symbol overrides with `isMaskOutline`):** Extended `legacy_metadata_mask_types_survive_fig_export_and_rest_outline_flag_maps_to_vector` in [`crates/designos-fig/tests/fig_roundtrip.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig/tests/fig_roundtrip.rs#L4193-L4272) with a REST `COMPONENT` + `INSTANCE` carrying `_overrides: [{"path": ["1:11"], "set": {"isMaskOutline": true}}]` (kills `N4`) and a `.fig` `SYMBOL` + `INSTANCE` carrying `symbolOverrides: [{"guidPath": {"guids": [g(1, 11)]}, "isMaskOutline": true}]`, and updated `convert_override` in [`crates/designos-fig/src/fig.rs:2195`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig/src/fig.rs#L2195) to forward `"isMaskOutline"` alongside `"maskType"` (kills `N16`).
   - **`N8` (`MaskType::Vector` `BooleanOp` keeps its operands):** Added a 0%-fill `BooleanOp::Union` `MaskType::Vector` mask case to `vector_mask_uses_own_outline_for_shape_frame_group_and_text` in [`crates/designos-render/tests/mask_without_own_fill.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render/tests/mask_without_own_fill.rs#L397-L423) and `gpu_vector_mask_outlines_and_rgb_luminance_match_cpu` in [`crates/designos-gpu/tests/mask_without_own_fill.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/tests/mask_without_own_fill.rs#L569-L598).
   - **`N9` (Hidden nested `Group` inside a `MaskType::Vector` group mask):** Added a hidden nested `Group` (`visible = false`) containing a visible rectangle at `(5, 45, 25, 25)` inside the `Group` mask case in both [`crates/designos-render/tests/mask_without_own_fill.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render/tests/mask_without_own_fill.rs#L343-L361) and [`crates/designos-gpu/tests/mask_without_own_fill.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu/tests/mask_without_own_fill.rs#L533-L552), asserting `(15, 55)` stays unrevealed (`[255, 255, 255, 255]`).
   - **`N11` (Dropdown label reads mask layers only):** In `mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle` in [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs#L2981-L2992), after setting `[m1 (Luminance mask), plain (non-mask, Alpha)]` and opening the `"mask-type"` dropdown, asserted that `test_rect(&ctx, "menu-check-0-2")` (`Luminance` checkmark) is positive (whereas under `N11`, `common(app, sel, ...)` yields `None` -> `"Mixed"` and `"menu-check-0-2"` is `Rect::NOTHING`).
   - **`N15` (`⌃⌥⌘M` does not toggle):** In `use_as_mask_shortcut_accepts_only_ctrl_cmd_m_on_macos` in [`crates/designos-app/src/shell/tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/shell/tests.rs#L1943-L1953), dispatched `Key::M` with `Modifiers { alt: true, ctrl: true, mac_cmd: true, command: true, .. }` (`⌃⌥⌘M`) and asserted `!app.doc.get(r).unwrap().is_mask`.

3. **Ruling 3 (`m5` — Bound Per-Frame Inspector Scan in `crates/designos-app/src/panels/properties.rs`):**
   - Bounded the header `all_mask` check at [`properties.rs:854`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs#L854) to `sel.iter().take(128).all(|id| app.doc.get(*id).is_some_and(|n| n.is_mask))`.
   - Bounded the per-frame `Mask` section filter at [`properties.rs:1043`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app/src/panels/properties.rs#L1043) to `let masks: Vec<NodeId> = sel.iter().take(128).copied().filter(|id| app.doc.get(*id).is_some_and(|n| n.is_mask)).collect();`, and collected the full `masks` list (`sel.iter().copied().filter(...)`) only inside `if let Some(i) = dropdown(...)` when the user selects a mask type.
   - Added a 129-node selection assertion (`128` non-masks followed by `m1` at index `128`) in `mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle` verifying that the `Mask` section does not scan beyond index `127` per frame (kills `N17`).

### 5. Red Runs, Rec. 601 Red Check, and Full Mutant Sweep

1. **Red on `0d652a1e` (`r3-red-on-0d652a1e.log`) vs Green on `HEAD` (`r3-green-on-head.log`):**
   - The 3 new tests for code modified in Round 3 (`pipelines::tests::composite_shader_injects_luminance_mask_coeffs_from_render_constant`, the `.fig` `symbolOverrides` `isMaskOutline` check in `legacy_metadata_mask_types_survive_fig_export_and_rest_outline_flag_maps_to_vector`, and the `take(128)` bound check in `mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle`) failed on `0d652a1e` production code and passed on `HEAD`; the 5 tests for `N4`, `N8`, `N9`, `N11`, `N15` (where `0d652a1e` production code already implemented the rule) were verified red against their corresponding mutants in `r3-mutants.log`:
```text
=== RED RUN ON 0d652a1e PRODUCTION CODE ===
running 1 test
test pipelines::tests::composite_shader_injects_luminance_mask_coeffs_from_render_constant ... FAILED
thread 'pipelines::tests::composite_shader_injects_luminance_mask_coeffs_from_render_constant' panicked at crates/designos-gpu/src/pipelines.rs:281:9:
composite.wgsl must hold the LUMINANCE_MASK_COEFFS token
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s

running 1 test
test legacy_metadata_mask_types_survive_fig_export_and_rest_outline_flag_maps_to_vector ... FAILED
thread 'legacy_metadata_mask_types_survive_fig_export_and_rest_outline_flag_maps_to_vector' panicked at crates/designos-fig/tests/fig_roundtrip.rs:4272:5:
assertion `left == right` failed: .fig symbolOverrides with isMaskOutline: true must map to MaskType::Vector
  left: Alpha
 right: Vector
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 39 filtered out; finished in 0.02s

running 1 test
test panels::properties::tests::mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle ... FAILED
thread 'panels::properties::tests::mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle' panicked at crates/designos-app/src/panels/properties.rs:3042:9:
Mask section per-frame scan must be bounded to take(128) and not scan beyond index 127
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4040 filtered out; finished in 0.35s
```

2. **Ruling 1 Rec. 601 Red Check ([`r3-rec601-red.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-rec601-red.log)):**
   - Temporarily setting `designos_render::LUMINANCE_MASK_COEFFS` in `crates/designos-render/src/mask.rs` to `[0.299, 0.587, 0.114]` (Rec. 601) does **not** panic in `Pipelines::new` and instead fails the GPU and CPU pixel assertions:
```text
running 1 test
test gpu_vector_mask_outlines_and_rgb_luminance_match_cpu ... FAILED
thread 'gpu_vector_mask_outlines_and_rgb_luminance_match_cpu' (63223772) panicked at crates/designos-gpu/tests/mask_without_own_fill.rs:628:9:
gpu pure red: expected 54, got 76

running 1 test
test luminance_mask_rgb_channels_and_shader_coefficients_match_rec709 ... FAILED
thread 'luminance_mask_rgb_channels_and_shader_coefficients_match_rec709' (63237149) panicked at crates/designos-render/tests/mask_without_own_fill.rs:471:9:
pure red: expected alpha 54, got 76
```

3. **Full Mutant Sweep — `M0-null-control` First, Then All 26 Mutants ([`r3-mutants.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-mutants.log)):**
```text
=== FULL MUTANT SUMMARY ===
M0-null-control: PASS (0 failed) []
r1M1-cpu-coeffs-rec601: KILLED ['pure red: expected alpha 54, got 76']
r1M2a-wgsl-coeffs-rec601: KILLED ['composite.wgsl must hold the LUMINANCE_MASK_COEFFS token']
r1M2b-pipelines-coeffs-rec601: KILLED ['processed composite shader must contain injected constant vec3<f32>(0.2126, 0.7152, 0.0722)']
r1M5-vector-text-keeps-text-fills: KILLED ['vector text mask with transparent text fill must reveal glyph pixels, got 0']
r1M6a-vector-frame-keeps-children: KILLED ['assertion `left == right` failed: outside rounded corner of vector frame is clipped despite unclipped child']
r1M6b-vector-shape-frame-keeps-own-fills: KILLED ['assertion `left == right` failed: center of rounded vector frame is fully opaque']
r1M7-frame-box-ignores-vector-rule: KILLED ['vector frame with children returns its own box from mask_frame_box even when it has a visible fill']
r1M11-merge-drops-mask_type: KILLED ['assertion `left == right` failed: branch merge must propagate mask_type changes']
r1M12-instance-sync-drops-mask_type: KILLED ['assertion `left == right` failed: component instance sync must propagate mask_type changes']
N1-fig-export-no-legacy-fallback: KILLED ['assertion `left == right` failed']
N2-fig-export-fallback-without-VECTOR: EQUIVALENT (falls through to fig_metadata rest loop) []
N3-metadata-skips-maskType-again: KILLED ['assertion `left == right` failed']
N4-rest-override-isMaskOutline-dropped: KILLED ['assertion `left == right` failed: REST symbol override with isMaskOutline: true must map to MaskType::Vector']
N5-cpu-group-path-off: KILLED ['assertion `left == right` failed: first group child outline is covered at full opacity']
N6-gpu-group-path-off: KILLED ['assertion `left == right` failed']
N8-boolean-children-cleared: KILLED ['assertion `left == right` failed: first boolean operand covered at full opacity']
N9-cpu-hidden-group-child-not-skipped: KILLED ['assertion `left == right` failed: child inside hidden nested group is not covered']
N10-dropdown-edits-whole-selection: KILLED ['assertion `left == right` failed: non-mask layer must never receive mask_type']
N11-dropdown-label-over-whole-selection: KILLED ['dropdown label for [Luminance mask, plain] must read mask layers only (Luminance checked, not Mixed)']
N12-dropdown-keeps-legacy-metadata: KILLED ['dropdown edit removes legacy fig_metadata maskType']
N13-header-back-to-first-node-rule: KILLED ['header mask button on mixed selection must turn all selected nodes into masks']
N14-shortcut-alt-cmd-again: KILLED ['⌥⌘M must not toggle Use as mask on macOS']
N15-shortcut-drops-not-alt: KILLED ['⌃⌥⌘M must not toggle Use as mask on macOS']
N16-fig-override-isMaskOutline-dropped: KILLED ['assertion `left == right` failed: .fig symbolOverrides with isMaskOutline: true must map to MaskType::Vector']
N7-descendant-keeps-own-type: KILLED ['assertion `left == right` failed: first group child outline is covered at full opacity']
N17-mask-section-unbounded-scan: KILLED ['Mask section per-frame scan must be bounded to take(128) and not scan beyond index 127']
```

### 6. Proof Sheet Regeneration on `HEAD` (Acceptance 2)

Regenerated the 8-row × 3-type (`Alpha` / `Vector` / `Luminance`) + committed fixture CPU/GPU proof sheet on `HEAD` using `review_probe_1492_r2.rs` and `make_sheet_r2.py`:
- **Saved artifact:** [`plans/reports/agy-1465-mask-types-evidence/r3-proof-sheet-cpu-gpu-per-type.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-proof-sheet-cpu-gpu-per-type.png) (`1266 × 1842`)
- **Saved probe log:** [`plans/reports/agy-1465-mask-types-evidence/r3-probe-log.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-probe-log.txt) (`76` lines)
- **Comparison against Round 2:**
```text
wrote plans/reports/agy-1465-mask-types-evidence/r3-proof-sheet-cpu-gpu-per-type.png (1266, 1842)
diff bbox against round-2 proof sheet: None
=== PROBE LOG DIFF ===
PROBE LOG IDENTICAL TO ROUND 2 (0 lines diff)
```

### 7. Exact Gate Commands & Pasted Outputs (Acceptance 3)

1. **`cargo fmt --all -- --check` ([`r3-cargo-fmt.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-cargo-fmt.log)):**
```text
cargo fmt exit=0
```

2. **`cargo clippy --workspace --all-targets --locked --keep-going -- -D warnings` ([`r3-cargo-clippy.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-cargo-clippy.log)):**
```text
    Checking designos-doc v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-doc)
    Checking designos-svg v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-svg)
    Checking designos-fonts v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fonts)
    Checking designos-render v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-render)
    Checking designos-diff v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-diff)
    Checking designos-export v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-export)
    Checking designos-webcapture v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-webcapture)
    Checking designos-gpu v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-gpu)
    Checking designos-fig v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-fig)
    Checking designos-library v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-library)
    Checking designos-eval v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-eval)
    Checking designos-mcp v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-mcp)
    Checking designos-app v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/crates/designos-app)
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1176:9
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1194:9
```

3. **`cargo test -p designos-gpu --locked -- --skip large_file_perf` (and `--skip large_file_compile_benchmarks` so the `tests/large_file_perf.rs` benchmark function `large_file_compile_benchmarks` is skipped; [`r3-cargo-test-gpu.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-cargo-test-gpu.log)):**
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-5ccf888a59871cb9)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/adapter.rs (out/cargo/debug/deps/adapter-c66eec4aeb2e3017)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.57s
     Running tests/command_buffer_limit.rs (out/cargo/debug/deps/command_buffer_limit-3fed03efd0561bf5)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.72s
     Running tests/fidelity.rs (out/cargo/debug/deps/fidelity-cc261f5d4fa3c7d5)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.23s
     Running tests/large_file_perf.rs (out/cargo/debug/deps/large_file_perf-0192ee8bff0db85c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
     Running tests/mask_without_own_fill.rs (out/cargo/debug/deps/mask_without_own_fill-2ce90af7d717be90)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.75s
     Running tests/program_cap.rs (out/cargo/debug/deps/program_cap-11a0c449a68ab83f)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.41s
     Running tests/refine_vram_budget.rs (out/cargo/debug/deps/refine_vram_budget-b0aaa3dfa8401165)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.51s
     Running tests/resized_shapes.rs (out/cargo/debug/deps/resized_shapes-417154c778d81844)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.59s
     Running tests/shadow_blend_parity.rs (out/cargo/debug/deps/shadow_blend_parity-5608cba68a554e4b)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.59s
     Running tests/shadow_fillless_fixture.rs (out/cargo/debug/deps/shadow_fillless_fixture-344513bf6ab15bc9)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.92s
     Running tests/shadow_silhouette_source.rs (out/cargo/debug/deps/shadow_silhouette_source-8a1801511554a0e2)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.99s
     Running tests/text_hug_box.rs (out/cargo/debug/deps/text_hug_box-2731001cf15a434c)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s
Total across designos-gpu: 80 passed; 0 failed; 0 ignored; 1 filtered out.
```

4. **`cargo test -p designos-render --locked` ([`r3-cargo-test-render.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-cargo-test-render.log)):**
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-9b14b2798b267e2b)
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 32.95s
     Running tests/mask_without_own_fill.rs (out/cargo/debug/deps/mask_without_own_fill-0f3196e55216fa9f)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     (... 24 integration test binaries total in designos-render ...)
Total across designos-render: 279 passed; 0 failed; 5 ignored.
```

5. **`cargo test -p designos-fig --locked` ([`r3-cargo-test-fig.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-cargo-test-fig.log)):**
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-861945a6becd4004)
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.42s
     Running tests/fig_roundtrip.rs (out/cargo/debug/deps/fig_roundtrip-ab37cec47eaf5b06)
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s
     (... 37 integration test binaries total in designos-fig ...)
Total across designos-fig: 493 passed; 0 failed; 6 ignored.
```

6. **`cargo test --workspace --locked --lib` ([`r3-cargo-test-workspace-lib.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-cargo-test-workspace-lib.log)):**
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-97b5db69fdcd9c68)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-18f4500351079eb1)
test result: ok. 710 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.38s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-c2489b38bbcccff9)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-18adcc4017a7a7af)
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.53s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-861945a6becd4004)
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.82s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-c2d586a1e5c886ba)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-77666ce24b473797)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-7534529c79ad779d)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.52s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-db051a2df27fbe42)
test result: ok. 322 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.80s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-59facfce9e95a574)
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 38.18s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-dd3732f70a143b29)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-2fa66b1349242d0a)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
Total across workspace --lib: 1,479 passed; 0 failed; 3 ignored.
```

7. **`cargo test -p designos-app --bin designos --locked -- mask` ([`r3-cargo-test-app-mask.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-cargo-test-app-mask.log)):**
```text
     Running unittests src/main.rs (out/cargo/debug/deps/designos-0146e1a2c129c956)

running 6 tests
test check_designs::tests::covering_sibling::a_mask_before_the_rectangle_leaves_the_backdrop_unknown ... ok
test panels::menus::tests::object_menu_use_as_mask_toggles_selection_and_undoes_in_one_step ... ok
test panels::properties::tests::mask_section_filters_to_masks_and_clears_legacy_metadata_and_header_matches_menu_toggle ... ok
test panels::properties::tests::mask_section_dropdown_edits_mixed_selection_and_undoes_in_one_step ... ok
test image_jobs::tests::erase_without_region_omits_mask_file_and_field ... ok
test shell::tests::agent_a11y::use_as_mask_shortcut_accepts_only_ctrl_cmd_m_on_macos ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 4035 filtered out; finished in 0.51s
```

8. **`python3 scripts/zero_fake_ui_gate.py` ([`r3-zero-fake-ui.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1465-mask-types/plans/reports/agy-1465-mask-types-evidence/r3-zero-fake-ui.log)):**
```text
 Gate Verdict: PASS (17/17 invariants satisfied)
```

9. **`git log --oneline origin/main..HEAD` (at Round 3 code commit `e4ff8103`):**
```text
e4ff8103 fix(mask): inject LUMINANCE_MASK_COEFFS token in WGSL, bound mask inspector scan and expand mask tests
04654004 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1465-mask-types
0d652a1e fix(mask): preserve legacy maskType export, align Vector outline coverage and luminance coefficients
7dfe45e2 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1465-mask-types
dd017308 feat(mask): Alpha, Vector and Luminance mask types (#1465)
```

### 8. Acceptance Table

| # | Acceptance Criterion | Evidence | Verdict |
|---|---|---|---|
| 1 | Each new test red on `0d652a1e` (or against its target mutant), green on `HEAD`; `N4`, `N8`, `N9`, `N11`, `N15` killed, null control first | `r3-red-on-0d652a1e.log`, `r3-rec601-red.log` (`gpu pure red: expected 54, got 76`), `r3-green-on-head.log`, `r3-mutants.log` (`M0-null-control: PASS`, `25/25` non-equivalent mutants killed including `N4`, `N8`, `N9`, `N11`, `N15`, `N16`, `N17`) | **PASS** |
| 2 | CPU/GPU per-type proof sheet regenerated on `HEAD`, unchanged against Round 2 | `r3-proof-sheet-cpu-gpu-per-type.png` (`0` px diff against Round 2 proof sheet), `r3-probe-log.txt` (`0` lines diff against Round 2 probe log) | **PASS** |
| 3 | Gates: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked --keep-going -- -D warnings` (only main's 2 `agent_permissions.rs` lints); `cargo test -p designos-gpu --locked -- --skip large_file_perf`; `cargo test -p designos-render --locked`; `cargo test -p designos-fig --locked`; `cargo test --workspace --locked --lib` | `r3-cargo-fmt.log` (`exit=0`), `r3-cargo-clippy.log`, `r3-cargo-test-gpu.log` (`80 passed; 0 failed`), `r3-cargo-test-render.log` (`279 passed; 0 failed`), `r3-cargo-test-fig.log` (`493 passed; 0 failed`), `r3-cargo-test-workspace-lib.log` (`1,479 passed; 0 failed`), `r3-cargo-test-app-mask.log` (`6 passed; 0 failed`), `r3-zero-fake-ui.log` (`17/17 PASS`) | **PASS** |
| 4 | Pushed; PR body updated; paste `git log --oneline origin/main..HEAD` | Pasted in §7.9 above; PR #1492 body updated via `gh pr edit 1492` | **PASS** |

### 9. Assumptions & Unresolved Questions

- **Assumptions:**
  - Bounding the per-frame right-inspector `Mask` section and header scan to `take(128)` matches the existing `common` helper (`take(128)`) while collecting the full `masks` list on dropdown selection so edits still apply to all selected masks.
- **Unresolved Questions (Deferred per Ruling 5):**
  1. **Q1:** When a `MaskType::Vector` group holds a non-clipping frame (`clips_content == false`) whose child overflows the frame box, should the overflowing child's outline be included in the group stencil?
  2. **Q2:** When a `MaskType::Vector` group contains a nested mask, should the nested mask clip its sibling descendants within the group or be unioned like other children?
  3. **Q3:** Should stroke geometry count toward `MaskType::Vector` coverage (e.g., on `Line` or open vector paths)?

Status: DONE
