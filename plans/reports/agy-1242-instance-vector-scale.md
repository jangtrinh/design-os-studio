# DEV Report · #1242 (P1): Vectors inside a resized instance keep the main component's geometry

## 1. Spec vs. HEAD Drift

| Item | Spec / Brief Claim | Actual Repo State at `HEAD` (`a39e402f6` -> `0a8225851`) | Resolution |
|---|---|---|---|
| Base branch / commit | `jangtrinh/agy-1242-instance-vector-scale` from `origin/release/v0.6.4` at `6f0c3bb2` | `origin/release/v0.6.4` is at `a39e402f6` (`6f0c3bb2` is `origin/release/v0.6.3`, 3 commits earlier: `3ef09a8b`, `511c264b`, `a39e402f`). Worktree came pre-checked-out on `jangtrinh/agy-1242-instance-vector-scale` at `a39e402f6`. | Kept branch at `a39e402f6` (`origin/release/v0.6.4`). |
| `rest_import.rs` derived-geometry line numbers | Brief mentions derived handling around `2420–2480` and `2600–2660` in `crates/designos-fig/src/rest_import.rs`. | At `a39e402f6`, `apply_overrides` is at `crates/designos-fig/src/rest_import.rs:2367-2389`, `apply_fast_derived` is at `2391-2479`, and `apply_override` is at `2577-2661` (with `size`/`relativeTransform`/`fillGeometry`/`strokeGeometry` handling at `2620-2658`). | Applied changes at the actual locations in `crates/designos-fig/src/rest_import.rs` and `crates/designos-fig/src/fig.rs`. |

---

## 2. Step 0 — Root-Cause Analysis (Recorded Before Any Fix)

### Symptom
When a component containing vector sublayers with `Scale` constraints (such as a `24×24` `chevron-selector-vertical` component containing two `10×5` `Icon` open-path vectors with `stroke.weight = 1.5` and `Scale` constraints, or a nested icon instance inside a resized button instance) is instantiated at `12×12`, the child vectors have `size = 5×2.5`, but their `stroke_geometry` and `NodeKind::Vector::paths` retain the main component's unscaled `10×5` outline. Because `designos-render/src/geom.rs:430-432` (`node_shape`) renders `Node::stroke_geometry` directly as the precomputed stroke outline, the `12×12` instance draws the `10×5` chevron at `2×` size and clips it to `<`.

### Root Causes (with `file:line` evidence)

1. **Large-file fast-derived path drops `fillGeometry` and `strokeGeometry` for every sublayer addressed by `overrideKey` (`crates/designos-fig/src/fig.rs:193-260`):**
   - In `extract_fast_derived` (`crates/designos-fig/src/fig.rs:194-201`), `needs_derived_geo` collects IDs from `raw_changes` for `"VECTOR" | "BOOLEAN_OPERATION" | "STAR" | "REGULAR_POLYGON" | "ELLIPSE" | "LINE" | "BRUSH"` nodes, but only inserts `nc.get("guid").and_then(kiwi_guid_str)` (`fig.rs:197-199`).
   - However, as documented at `crates/designos-fig/src/fig.rs:1400-1402` and `crates/designos-fig/src/FIELDS.md:77`, Figma's `derivedSymbolData` (`guidPath.guids`) addresses component sublayers by their **`overrideKey`** whenever `overrideKey` is present on the node (and only falls back to `guid` when `overrideKey` is absent).
   - At `crates/designos-fig/src/fig.rs:246`, `extract_fast_derived` checks:
     ```rust
     let keep_geo = path.last().is_some_and(|last| needs_derived_geo.contains(last));
     ```
   - Because `needs_derived_geo` only contains `"guid"` and not `"overrideKey"`, `keep_geo` evaluates to `false` whenever the target vector has an `overrideKey` (which is standard in Figma component kits). Consequently, `extract_fast_derived` strips `fillGeometry` and `strokeGeometry` (`None`) at `fig.rs:249-250`, leaving only `d.size` and `d.transform` in `FastDerivedEntry`!
   - Then in `apply_fast_derived` (`crates/designos-fig/src/rest_import.rs:2452-2463`), when `d.size` (`5×2.5`) is applied to the vector (`is_vec_or_bool == true`), `n.size` is overwritten with `5×2.5` while `n.stroke_geometry`, `n.fill_geometry`, and `NodeKind::Vector { paths, network }` remain unscaled at `10×5`.

2. **Missing derived geometry / size-only override on a vector or parent container (`crates/designos-fig/src/rest_import.rs:2383-2479` and `2620-2658`):**
   - **2a. Direct size override or size-only derived entry on a vector:** In `apply_fast_derived` (`rest_import.rs:2453-2463`) and `apply_override` (`rest_import.rs:2620-2632`), when `d.size` or `set["size"]` changes a node's size from `old_size` (`10×5`) to `sz` (`5×2.5`), if the node was not already scaled by `plan_constraint_pass` on its root instance (for example, when the vector's parent has `Min`/`Center` constraints or `auto_layout`, or when an override resizes a nested instance/container or vector directly), `n.size = sz` is assigned without scaling `paths`, `network`, `fill_geometry`, or `stroke_geometry` via `designos_doc::apply_constrained_geometry`.
   - **2b. Nested instance or container resized by `symbolOverrides` / `derivedSymbolData`:** When an outer instance (e.g., a `Button` with `auto_layout`) is resized or swaps/overrides a nested icon instance (`12×12` vs. its `24×24` main component), `expand_instance` (`rest_import.rs:2330-2349`) runs `plan_constraint_pass` on the outer `Button` — which skips in-flow children of an `auto_layout` frame (`crates/designos-doc/src/layout_constraints.rs:64`). The nested icon instance's `12×12` size arrives via `symbolOverrides` or `derivedSymbolData` (`apply_override` at `rest_import.rs:2630` or `apply_fast_derived` at `2462`), which sets `n.size = 12×12` on the nested instance **without** running `plan_constraint_pass` on the nested instance's children! Moreover, if `swap_nested` (`rest_import.rs:2105`) runs on a nested instance, it unconditionally resets `dn.size = target_node.size`. As a result, when child derived entries are absent (or stripped in large-file mode), the nested icon's vector children stay at `10×5` (`24px` main component geometry) or keep `10×5` outlines inside a `5×2.5` box.
   - **2c. Open-path stroked vectors do not update `NodeKind::Vector::paths` when `strokeGeometry` is applied without `fillGeometry` (`crates/designos-fig/src/rest_import.rs:2467-2478` and `2638-2658`):**
     - For an open-path vector (like a chevron with `fillGeometry: []` and `strokeGeometry: [...]`), `build_node` (`rest_import.rs:1073-1077`) initializes `NodeKind::Vector::paths` from `stroke_geo` because `fill_geo` is empty (`None`).
     - However, in `apply_fast_derived` (`rest_import.rs:2467-2478`) and `apply_override` (`rest_import.rs:2638-2658`), `NodeKind::Vector::paths` is only updated inside the `if let Some(g) = fill_geo` / `if has("fillGeometry")` branch! When `fillGeometry` is absent or empty on an open-path vector and `strokeGeometry` is present, `n.stroke_geometry` is updated (or in `apply_override`, if `fillGeometry` is `[]`, `paths` is wiped to `vec![]` so `derived_shape` in `designos-render/src/geom.rs:465` returns `None`), while `NodeKind::Vector::paths` either becomes empty or retains the wrong outline.
   - **2d. Shallow-vs-deep ordering of `derivedSymbolData` entries (`crates/designos-fig/src/rest_import.rs:2378-2387`):**
     - When `derivedSymbolData` contains both a resized parent container (e.g. `path = ["10:3"]`, `size = 12×12`) and its child vectors (`path = ["10:3", "1:2"]`, `size = 5×2.5`, `strokeGeometry = ...`), applying `ov.derived` and `ov.fast_derived` in ascending `path.len()` order (shallowest first) and tracking `explicitly_sized: HashSet<NodeId>` ensures parent container constraint propagation runs before child-specific derived overrides and never overwrites or double-scales an explicitly sized descendant.

---

## 3. Changes Made

| File | Lines | Description |
|---|---|---|
| `crates/designos-fig/src/fig.rs` | `195–203` | In `extract_fast_derived`, insert both `"guid"` and `"overrideKey"` of shape/vector `nodeChanges` into `needs_derived_geo` so `keep_geo` preserves `fillGeometry` and `strokeGeometry` blobs for component sublayers addressed by `overrideKey` in large `.fig` files (`> 5,000` nodes). |
| `crates/designos-fig/src/rest_import.rs` | `2093–2111` | In `swap_nested`, preserve `dn.size` when `dn` was already resized relative to its previous component `current` before calling `expand_instance(d, target)`. |
| `crates/designos-fig/src/rest_import.rs` | `2355–2405` | Increase `override_key_of` (and `source_tail_of` at `2512`) hop limit from `8` to `64`. Add `resize_override_node`, which records explicitly sized nodes, scales vector/boolean geometry (`paths`, `network`, `fill_geometry`, `stroke_geometry`) via `designos_doc::apply_constrained_geometry` whenever an override changes `n.size`, and propagates `designos_doc::plan_constraint_pass` to non-explicitly-sized descendants when a container (`Frame`, `Group`, `Instance`, etc.) is resized. |
| `crates/designos-fig/src/rest_import.rs` | `2409–2435` | In `apply_overrides`, track `explicitly_sized: HashSet<NodeId>` across `ov.set`, `ov.derived`, and `ov.fast_derived`, and sort `ov.derived` and `ov.fast_derived` by ascending `path.len()` so parent container resizes propagate constraints before child-specific derived entries run. |
| `crates/designos-fig/src/rest_import.rs` | `2437–2509`, `2611–2733` | In both `apply_fast_derived` and `apply_override`, route `size` changes through `resize_override_node`, guard against empty `fillGeometry: []` wiping open-path vector `paths`, and update `NodeKind::Vector::paths` from derived `strokeGeometry` whenever `fillGeometry` is absent/empty and `n.fill_geometry.is_none()`. |
| `crates/designos-fig/tests/instance_vector_scale.rs` | `1–560` | New red-first integration test suite covering all 5 required scenarios in both normal (`<= 5,000` nodes) and large (`> 5,000` nodes) `.fig` modes, including `designos_render::render_node` pixel assertions verifying both chevrons render unclipped. |

### Commits on `jangtrinh/agy-1242-instance-vector-scale`
1. `10876f873` — `test(fig): add red tests for vector geometry inside resized instances (#1242)`
2. `0a8225851` — `fix(fig): scale and apply derived vector geometry inside resized instances (#1242)`

---

## 4. Red -> Green Test Evidence

- **Red log (before fix, commit `10876f873`):** `plans/reports/agy-1242-instance-vector-scale-evidence/red.log`
  - `unchanged_instance_at_main_component_size_keeps_byte_identical_geometry`: `ok`
  - `resized_instance_with_derived_stroke_geometry_applies_scaled_outline_in_small_and_large_files`: `FAILED` (`pad_large=false IconTop: expected vector paths bounds (-0.75..5.75, -0.75..3.25), got (-0.375..5.375, -0.375..2.875)`)
  - `resized_instance_without_derived_geometry_scales_geometry_to_fit_child_size`: `FAILED` (`pad_large=false: expected scaled stroke_geometry max (5.375, 2.875), got (10.75, 5.75)`)
  - `nested_icon_instance_inside_resized_button_instance_scales_child_vectors`: `FAILED` (`pad_large=false Button-small-derived/IconTop left: Vec2 { x: 10.0, y: 5.0 }, right: Vec2 { x: 5.0, y: 2.5 }`)
- **Green log (after fix, commit `0a8225851`):** `plans/reports/agy-1242-instance-vector-scale-evidence/green.log`
  - `resized_instance_with_derived_stroke_geometry_applies_scaled_outline_in_small_and_large_files`: `ok`
  - `resized_instance_without_derived_geometry_scales_geometry_to_fit_child_size`: `ok`
  - `nested_icon_instance_inside_resized_button_instance_scales_child_vectors`: `ok`
  - `unchanged_instance_at_main_component_size_keeps_byte_identical_geometry`: `ok`
  - Result: `4 passed; 0 failed` in `0.18s`.

---

## 5. Render Check & Big-File Import Guard

### Render Check (`designos --screenshot` & `designos_render::render_node`)
- Imported the procedural chevron `.fig` fixture into a `Document`, saved it as `plans/reports/agy-1242-instance-vector-scale-evidence/chevron-fixture.designos`, and rendered:
  1. `plans/reports/agy-1242-instance-vector-scale-evidence/chevron-studio-screenshot.png` via:
     ```sh
     cargo run -p designos-app --bin designos --locked -- \
       plans/reports/agy-1242-instance-vector-scale-evidence/chevron-fixture.designos \
       --select chevron-12px-derived --zoom 800 --no-toast \
       --screenshot plans/reports/agy-1242-instance-vector-scale-evidence/chevron-studio-screenshot.png
     ```
  2. `plans/reports/agy-1242-instance-vector-scale-evidence/main-24px-x8.png` (`24×24` main component rendered at `8×` = `192×192` px)
  3. `plans/reports/agy-1242-instance-vector-scale-evidence/instance-12px-x16.png` (`12×12` instance with `derivedSymbolData` `strokeGeometry` rendered at `16×` = `192×192` px)
  4. `plans/reports/agy-1242-instance-vector-scale-evidence/instance-12px-fallback-x16.png` (`12×12` instance without `derivedSymbolData` rendered at `16×` = `192×192` px)
- Visual verification: In all renders, the `12×12` instances display two complete, centered, unclipped chevrons (`↕`) inside the `12×12` bounds rather than a thick, clipped `<`.

### Big-File Import Guard (`±5%` requirement)
- Timed 1,001 iterations (after 50 warmup iterations) in `--release` mode on the largest fixtures in `crates/designos-fig/tests/fixtures/` before (`bench-before.log`) and after (`bench-after.log`) the fix:

| Fixture | Size | Before Median (`µs`) | After Median (`µs`) | Delta | Within `±5%`? |
|---|---:|---:|---:|---:|---|
| `crates/designos-fig/tests/fixtures/rest_file.json` | 13,976 B | `96.33 µs` | `96.21 µs` | `-0.12%` | PASS |
| `crates/designos-fig/tests/fixtures/legacy-0.4.0-rich.fig` | 11,061 B | `387.38 µs` | `381.08 µs` | `-1.63%` | PASS |
| `crates/designos-fig/tests/fixtures/legacy-0.4.0-grids.fig` | 10,831 B | `390.96 µs` | `388.17 µs` | `-0.71%` | PASS |

---

## 6. Acceptance Gate Results

All mandatory gates were run on the final commit (`0a8225851`) inside the isolated test harness:

| Gate Command | Result |
|---|---|
| `cargo fmt --all -- --check` | `PASS` (exit 0) |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | `PASS` (exit 0, 0 warnings) |
| `cargo test -p designos-fig --locked` | `PASS` (all unit + integration tests including `instance_vector_scale`: 4/4 passed) |
| `cargo test --workspace --locked --lib` | `PASS` (exit 0, 0 failures across all workspace library crates) |
| `cargo test -p designos-app --bin designos --locked` | `PASS` (`3357 passed; 0 failed; 11 ignored`) |
| `python3 scripts/zero_fake_ui_gate.py` | `PASS` (`17/17 invariants satisfied`) |
| `python3 scripts/check_spacing_relationships.py` | `PASS` (`no count rose above the baseline`) |

---

## 7. Residual Risks & Notes

- **Scope discipline:** Only `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/rest_import.rs`, and the new test file `crates/designos-fig/tests/instance_vector_scale.rs` were modified. `crates/designos-fig/src/kiwi.rs` (#1043), `style_table` / style seeding (#1165), and renderer crates (`designos-render`, `designos-gpu`) were untouched.
- **Residual risk:** When a `.fig` file has neither `derivedSymbolData` nor `Scale` constraints on a child vector inside a resized container, scaling only triggers when the vector's own `size` changes (via `Scale` constraints or an explicit `size` override), matching Figma's constraint rules.

Status: DONE — commits 10876f873, 0a8225851 on jangtrinh/agy-1242-instance-vector-scale; tests 4 passed / 0 failed; visual diff 12px chevron instances render two unclipped chevrons matching 24px main component
