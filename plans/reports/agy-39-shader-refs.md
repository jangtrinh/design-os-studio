# Report: Preserve Custom Shader Effect and Paint References on Round Trip (`#39`)

- **Branch**: `jangtrinh/agy-39-shader-refs`
- **Draft PR**: https://github.com/jangtrinh/design-os-studio/pull/1602

## 1. Summary

Custom shader effects (`EffectType::CUSTOM = 9`) and custom shader paints (`PaintType::CUSTOM = 10` in `.fig`, `type: "SHADER"` in REST/plugin JSON) are now preserved across `.fig`, REST JSON, and `.designos` JSON round trips without silent drops.

- **Document Model (`crates/designos-doc/src/paint.rs`)**:
  - Added `ShaderRef { pub raw: serde_json::Value }` (`crates/designos-doc/src/paint.rs:400-409`), `PaintKind::Shader(ShaderRef)` (`crates/designos-doc/src/paint.rs:439`), and `Effect::Shader { shader: ShaderRef, visible: bool }` (`crates/designos-doc/src/paint.rs:573-578`).
  - `Paint::silhouette_alpha` returns `0.0` for `PaintKind::Shader(_)` (`crates/designos-doc/src/paint.rs:463`), ensuring unrendered shader fills contribute zero silhouette coverage to shadows, blurs, noise, and glass effects.
  - `Effect::visible` reads `visible` on `Effect::Shader` (`crates/designos-doc/src/paint.rs:588`).
  - Note on backward compatibility: older Studio versions that predated `PaintKind::Shader` and `Effect::Shader` cannot open `.designos` files containing those tagged enum variants; existing `.designos` files without shaders load and save byte-for-byte unchanged (`crates/designos-doc/src/paint.rs:1000-1008`).
- **`.fig` & REST Import/Export (`crates/designos-fig`)**:
  - `.fig` import (`crates/designos-fig/src/fig.rs:2734-2739`, `2788-2793`) extracts the full decoded Kiwi message minus `type` into `"shader"` for `PaintType::CUSTOM` and `EffectType::CUSTOM`.
  - REST/internal JSON import (`crates/designos-fig/src/rest_import.rs:176-205`, `278-308`) parses `"SHADER"` / `"CUSTOM"` paints and effects into `PaintKind::Shader(ShaderRef { raw })` and `Effect::Shader { shader: ShaderRef { raw }, visible }`, preserving fill `opacity`, `visible`, `blend`, and slot order as well as effect styles.
  - REST export (`crates/designos-fig/src/export.rs:285-290`, `351-354`) emits `type: "SHADER"` with `"shader": raw`.
  - `.fig` export (`crates/designos-fig/src/fig_export.rs:1931-1940`, `2051-2085`, `2101-2191`) re-encodes every field stored in `ShaderRef.raw` through `Schema::field_from_json` (`crates/designos-fig/src/kiwi.rs:788-903`) and writes back the layer's `opacity`, `visible`, and `blendMode`. If any field or nested message/array element in `ShaderRef.raw` cannot be encoded against the target `.fig` schema, `export_fig` returns `Err(FigError::Format(...))` naming the layer and the offending field instead of silently dropping the shader (`crates/designos-fig/src/fig_export.rs:2125-2190`).
- **CPU, GPU, Export & Codegen Skip Arms**:
  - CPU (`crates/designos-render/src/paint.rs:24`) and GPU (`crates/designos-gpu/src/paintconv.rs:34`) return `None` for `PaintKind::Shader(_)` and ignore `Effect::Shader`, allocating zero GPU programs and drawing zero pixels.
  - SVG (`crates/designos-export/src/svg.rs:196,650`), PDF (`crates/designos-export/src/pdf.rs:163,260,729`), CSS (`crates/designos-export/src/css.rs:176,189,405`), Tailwind, SwiftUI, Compose, UIKit, and Android XML skip `PaintKind::Shader` and `Effect::Shader` without adding export overflow or forcing PDF vector frames to rasterize.
- **Honest Inspector UI (`crates/designos-app`)**:
  - The Effects panel (`crates/designos-app/src/panels/effects.rs:150,224-239,406`) and Fill/Stroke rows (`crates/designos-app/src/panels/properties.rs:1463-1467,1514-1542`) display `Icon::Shader` and static secondary caption text `"Shader (not rendered)"` with no settings popover and no clickable swatch, while keeping the visibility eye toggle, opacity field (on fills), and `-` remove button working.
  - Added the 1-line exhaustive match arm in `crates/designos-app/src/widgets/picker.rs:97` (`PaintKind::Solid(_) | PaintKind::Shader(_) => Mode::Solid`) per coordinator approval (`msg_bab45df7604b`), and verified in `crates/designos-app/src/panels/properties.rs:2978-2998` that opening and closing the fill picker on a shader fill leaves the paint byte-equal.

---

## 2. Two-Commit Breakdown

1. **`a28f6274d`** — `feat(doc): add ShaderRef, PaintKind::Shader, and Effect::Shader variants`
   - Adds `ShaderRef`, `PaintKind::Shader`, `Effect::Shader`, `Paint::silhouette_alpha` (`0.0`), `Effect::visible`, `.designos` JSON round-trip tests, and exhaustive skip/summary arms across `designos-doc`, `designos-render`, `designos-gpu`, `designos-svg`, `designos-mcp`, `designos-export`, `designos-fig/src/pen`, and `designos-app`.
2. **`c8ab44e13`** — `feat(fig): round-trip custom shader effects and fills through .fig and REST`
   - Implements `.fig` and REST import/export for `PaintType::CUSTOM` and `EffectType::CUSTOM`, strict schema validation returning `FigError::Format` naming the layer and field on re-encode failure, `"Shader (not rendered)"` inspector rows in `effects.rs` and `properties.rs`, and unit/integration tests across `designos-fig`, `designos-render`, `designos-gpu`, `designos-export`, and `designos-app`.

---

## 3. Kit Scan & Coordinator Rulings

- **Untitled UI PRO v8.0 Kit Scan (`plans/reports/agy-39-shader-refs-evidence/kit-scan.log:17`)**:
  - Scanned all `119,748` `nodeChanges` in `out/kit-clone.fig`:
    `kit_scan: nodeChanges=119748 EffectType::CUSTOM=0 PaintType::CUSTOM(fills)=0 PaintType::CUSTOM(strokes)=0`
  - Confirmed zero custom shader effects or fills exist in the local kit, so the synthetic `.fig` and Kiwi round-trip fixtures in `crates/designos-fig/src/fig.rs:4238-4465` and `crates/designos-fig/src/kiwi.rs:1461-1495` provide full coverage.
- **Coordinator Ruling on `crates/designos-app/src/widgets/picker.rs:97`**:
  - Approved adding `PaintKind::Solid(_) | PaintKind::Shader(_) => Mode::Solid` at `crates/designos-app/src/widgets/picker.rs:97` (`msg_bab45df7604b`), verified by `panels::properties::tests::shader_inspector_rows_render_label_and_toggle_visibility` (`crates/designos-app/src/panels/properties.rs:2978-2998`).

---

## 4. Verification & Evidence

All evidence logs and captures are committed under `plans/reports/agy-39-shader-refs-evidence/`:

| Check | Evidence File & Line | Result |
|---|---|---|
| `.designos` JSON compatibility & shader round-trip | `plans/reports/agy-39-shader-refs-evidence/doc-shader-tests.log:14-15` | `old_designos_file_without_shaders_loads_and_saves_unchanged ... ok`, `designos_json_round_trip_keeps_shader ... ok` (8 passed) |
| `.fig` & Kiwi shader round-trip, byte equality, effect styles, slot order, unknown fields, and `FigError::Format` on bad field | `plans/reports/agy-39-shader-refs-evidence/fig-shader-tests.log:9-16`, `25-26` | 6 passed in `fig::tests` + 1 passed in `kiwi::tests::field_from_json_reconstructs_code_component_id_and_prop_assignments` |
| CPU shadow silhouette (`silhouette_alpha == 0.0`) | `plans/reports/agy-39-shader-refs-evidence/render-shader-tests.log:9-11` | `shadow_tests::shader_paint_has_zero_silhouette_alpha ... ok` |
| CPU vs GPU pixel parity (`0.0000%` diff, `ssim 1.0000`) | `plans/reports/agy-39-shader-refs-evidence/gpu-shader-parity.log:7-14` | `shader-skip-parity: mismatch 0.0000% (0 px of 14400) at tolerance 0, ssim 1.0000` (7 passed) |
| SVG, PDF & CSS export skip shader fill/effect | `plans/reports/agy-39-shader-refs-evidence/export-shader-tests.log:9-11` | `tests::shader_effect_and_paint_skipped_by_svg_pdf_and_css_export ... ok` |
| Inspector `"Shader (not rendered)"` rows, visibility toggle, remove button, and picker no-op | `plans/reports/agy-39-shader-refs-evidence/app-shader-inspector.log:4-7` | `panels::effects::tests::shader_inspector_rows_render_label_and_toggle_visibility ... ok`, `panels::properties::tests::shader_inspector_rows_render_label_and_toggle_visibility ... ok` |
| Cropped inspector visual capture | `plans/reports/agy-39-shader-refs-evidence/shader-inspector.png` | 40 KB PNG showing Fill (`Shader (no... 75 %`) and Effects (`Shader (not rendered)`) rows |
| Zero Fake UI Gate | `plans/reports/agy-39-shader-refs-evidence/zero-fake-ui-gate.log:26` | `Gate Verdict: PASS (17/17 invariants satisfied)` |
| Clippy & Rustfmt | `plans/reports/agy-39-shader-refs-evidence/clippy.log:111`, `fmt-check.log:1` | 0 warnings, 0 formatting diffs |

---

## 5. Build Target & Temporary File Cleanup

Removed `out/cargo`, `out/kit-clone.fig`, and the sibling temporary home directory `../_home/agy-39-shader-refs`:

```text
$ ls out/cargo
ls: out/cargo: No such file or directory
```
