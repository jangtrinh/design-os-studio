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
