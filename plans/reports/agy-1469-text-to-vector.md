# Issue #1469 — Convert text to vector paths (Flatten and Outline stroke on text)

## 1. Re-anchor Findings & What Could Not Be Verified First

### What Could Not Be Verified Against Live Figma (`TO BE CONFIRMED AGAINST FIGMA`)
- **Mixed-paint Flatten result on multi-run text**: Per Ruling 3 of the task brief, **Flatten** (`⌘E` / `Ctrl+E`) on a single-paint text layer produces a single `NodeKind::Vector` holding every glyph, whereas when styled text runs have distinct `[fills, strokes, stroke_weight, stroke_align, stroke_cap, stroke_join, stroke_miter_limit, dash_pattern]` paint sets, Flatten produces a `NodeKind::Group` named after the text layer containing one `NodeKind::Vector` child per distinct paint set in first-appearance order so no run colour or stroke style is lost. Because opening live Figma is forbidden by the worker boundary, **this mixed-paint Flatten grouping behavior is to be confirmed against Figma**.
- **GPU vs CPU pixel comparison**: Headless Metal/Vulkan GPU rendering is unavailable in the sandboxed test environment without a display server, so visual pixel verification was performed deterministically using `designos_render::render` (`tiny-skia` CPU reference renderer) on both synthetic multi-run/decorated text fixtures and an imported Untitled UI PRO VARIABLES v8.0 `.fig` copy.

### Re-anchored Code Sites & Pre-Existing Drift
1. **`crates/designos-app/src/vecops.rs:58-159` (`flatten_selection`)**:
   - Line `67` checked `let convert = sel.len() == 1 && !matches!(doc.node(sel[0]).map(|n| &n.kind), Some(NodeKind::Vector(_)));`.
   - Line `68` invoked `designos_render::flatten(&doc, &sel)` and replaced all selected nodes with a single `Vector` node copying fills/strokes only from `sel[0]` (`first_id`), with no handling for `NodeKind::Text` multi-run fills/strokes, font-loading checks, or per-node conversion in mixed selections.
2. **`crates/designos-app/src/vecops.rs:161-214` (`outline_stroke_selection`)**:
   - Called `designos_render::outline_stroke(&doc, id)` on each selected node, which only outlined existing stroke paths around `node_paths(doc, n)` and returned `None` when `n.strokes` was empty (`vectorops.rs:288`). For a filled text layer without a stroke, `outline_stroke_selection` was a no-op (`Toast: Nothing to outline`) instead of converting each shaped glyph into its own `Vector` inside a `Group` named after the text layer.
3. **`crates/designos-render/src/vectorops.rs:55-96` (`node_paths`)**:
   - Line `62`: `NodeKind::Text(tp)` called `text::layout_text(tp, n.size)` and discarded `tl.styles` via `tl.paths.into_iter().flatten().collect()`, losing per-glyph boundaries and per-run fill/stroke styling.
   - Line `97-115` (`collect_geometry`): Excluded `NodeKind::Text(_)` (`Frame | Group | Slice | Text(_) | Connector(_) => {}`), so `vectorops::flatten(&doc, ids)` skipped text layers entirely and returned `None`.
4. **`crates/designos-render/tests/vectorops.rs:76-82` (`flatten_skips_text_and_empty_selections`)**:
   - Explicitly asserted `assert!(flatten(&doc, &[txt]).is_none())`, codifying the old behavior where `flatten` ignored text nodes.
5. **`crates/designos-render/src/text.rs:241-686` (`compute_layout`)**:
   - Extracted glyph outlines via `swash_cache.get_outline_commands` (`text.rs:527`) and synthesized list markers (`•` bullets, ordered counters, checkboxes at `text.rs:423-498`) plus underline and strikethrough bars (`text.rs:574-639`), but merged all glyphs of a style bucket into a single `Vec<Path>` per style (`paths: Vec<Vec<Path>>`), preventing per-glyph vector extraction for **Outline stroke**.
6. **`crates/designos-app/src/fonts.rs:82-227` (`FontLoader`)**:
   - Loaded fonts asynchronously on a background thread (`sync(&mut self, doc: &Document) -> bool`) without a synchronous entrypoint for one-shot commands (`Flatten` / `Outline stroke`) to load missing text fonts before outlining or fail with `Font unavailable: ...`.

---

## 2. Implementation Summary

- **`crates/designos-render/src/text.rs`**:
  - Added `ShapedGlyphOutline { paths: Vec<Path>, style: RunPaintStyle }` and `TextGlyphOutlines { glyphs: Vec<ShapedGlyphOutline>, decorations: Vec<ShapedGlyphOutline> }`.
  - Added `pub fn glyph_outlines(props: &TextProps, size: Vec2) -> TextGlyphOutlines` (`compute_glyph_outlines`), reusing the exact `cosmic-text` shaping, list-marker synthesis, and underline/strikethrough geometry as `layout_text` while keeping each non-empty glyph (plus list marker and decoration shape) as its own `ShapedGlyphOutline` with its resolved `RunPaintStyle`.
- **`crates/designos-render/src/vectorops.rs` & `crates/designos-render/tests/vectorops.rs`**:
  - Added `TextVectorRun { paths: Vec<Path>, fills: Vec<Paint>, strokes: Vec<Paint>, stroke_weight: f32, stroke_align: StrokeAlign, stroke_cap: StrokeCap, stroke_join: StrokeJoin, stroke_miter_limit: f32, dash_pattern: Vec<f32> }`.
  - Added `pub fn text_flatten_runs(doc: &Document, n: &Node) -> Vec<TextVectorRun>` (groups glyph/decoration outlines by distinct paint set in first-appearance order; resolves variable bindings on the text node's base fills/strokes when building `designos_render::flatten` geometry) and `pub fn text_glyph_runs(doc: &Document, n: &Node) -> Vec<TextVectorRun>` (emits one `TextVectorRun` per shaped glyph, list marker, and underline/strikethrough decoration).
  - Updated `node_paths` (`vectorops.rs:62-75`) to retain all glyph paths (`tl.paths`) and stroke-outlined paths when a run carries strokes, and updated `collect_geometry` (`vectorops.rs:97-122`) so `designos_render::flatten` converts `NodeKind::Text(_)` layers alongside shapes.
  - Updated `crates/designos-render/tests/vectorops.rs` (`flatten_converts_text_and_skips_empty_selections`) to assert that `flatten` converts a `Text` node into a non-empty `VectorNetwork`.
- **`crates/designos-app/src/fonts.rs`**:
  - Added `FontLoader::ensure_loaded(&mut self, requests: BTreeSet<FontRequest>) -> Result<(), Vec<String>>`, which checks `designos_render::has_font_family`, loads bundled or cached/downloaded font families via `designos_fonts::ensure_font_requests_with` when missing, and returns the sorted list of unavailable family names if any cannot be loaded.
- **`crates/designos-app/src/vecops.rs`**:
  - Added `collect_text_font_requests` and `ensure_text_fonts` so both `flatten_selection` and `outline_stroke_selection` load any unloaded font family on selected `Text` nodes before converting, or abort without mutating the document and push `Toast: Font unavailable: <families>` when a font cannot be loaded.
  - Added `remap_paint_gradients` (remaps normalized gradient handle coordinates from the original text box to each child vector's local bounding box so linear/radial/angular/diamond gradients render at identical page coordinates) and `carry_paint_bindings` (copies `fills/{i}/color` and `strokes/{i}/color` `VariableBinding`s onto converted vectors when a run inherits the text node's base fills or strokes).
  - Updated `flatten_selection` (`vecops.rs:69-215`):
    - Pure text selections and mixed selections containing text convert each selected layer at its own parent index and transform inside a single `Command::Batch` undo step.
    - Single-paint text converts to a single `NodeKind::Vector` named after the text layer; multi-paint text converts to a `NodeKind::Group` named after the text layer containing one `NodeKind::Vector` per distinct paint set.
  - Updated `outline_stroke_selection` (`vecops.rs:217-280`):
    - Converts each selected `NodeKind::Text` layer into a `NodeKind::Group` named after the text layer (at the text layer's parent index and transform) containing one `NodeKind::Vector` per glyph (and decoration/list-marker shape), each carrying its run's fills, strokes, stroke settings, and variable bindings, bundled with any selected shapes into a single `Command::Batch` undo step.

---

## 3. Acceptance Criteria & Verification Table

| # | Criterion | Command | Result / Output |
|---|---|---|---|
| 1 | Unit tests in `vecops::tests`: (a) Outline stroke of two-run text -> group with one vector per glyph & each run's fill; (b) Flatten of single-paint text -> one vector; (c) Flatten of two-paint text -> two vectors in a group; (d) Fill/stroke variable bindings carried over; (e) One undo restores text node in `doc.to_json()`; (f) Unloaded font loads first & unavailable font refuses with toast; (g) Mixed selection converts text + shapes together | `cargo test -p designos-app --bin designos --locked vecops::tests:: -- --nocapture` | `23 passed; 0 failed` (all 7 new unit tests + 1 pixel test + 15 existing `vecops::tests` pass; see `plans/reports/agy-1469-text-to-vector-evidence/06-test-vecops-nocapture.log`) |
| 2 | Pixel test: render a text frame before and after conversion; compare with CPU renderer | `cargo test -p designos-app --bin designos --locked vecops::tests::pixel_test_converted_text_vectors_match_original_text_render -- --nocapture` | `PIXEL_DIFF_FLATTEN: max=29 mean=0.005257 non_edge_max=0`, `PIXEL_DIFF_OUTLINE_STROKE: max=29 mean=0.005257 non_edge_max=0` — `ok` |
| 3 | Kit test: open a copy of the Untitled UI kit, convert a text layer, render before and after to `out/evidence/1469-*.png` | Headless CPU render harness on an APFS copy of `$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (`page=NodeId(5507)`, `frame=NodeId(5509) "Layout"`, `text=NodeId(5512) "align-bottom-01"`) | Wrote `out/evidence/1469-kit-before.png`, `out/evidence/1469-kit-after-flatten.png` (`max=14 mean=0.000011 non_edge_max=0`), and `out/evidence/1469-kit-after-outline-stroke.png` (`max=14 mean=0.000011 non_edge_max=0`) |
| 4 | Gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --locked`, `cargo test -p designos-doc -p designos-render --locked`, `cargo test -p designos-app --bin designos --locked -- --test-threads=8`, `cargo test --workspace --locked --lib` | Full gate suite (logs in `plans/reports/agy-1469-text-to-vector-evidence/01-fmt.log` .. `05-test-workspace-lib.log`) | `fmt`: exit 0; `clippy`: exit 0, 0 warnings in touched files; `designos-doc` + `designos-render`: `715 + 230 + 23` passed, `0 failed`; `designos-app --bin designos`: `3994 passed; 0 failed; 13 ignored`; `--workspace --lib`: `0 failed` |

### Pasted Gate Outputs

```text
=== 01-fmt.log: cargo fmt --all -- --check ===
(exit 0, 0 diffs)

=== 02-clippy.log: cargo clippy --workspace --locked ===
warning: `designos-app` (bin "designos") generated 2 warnings (run `cargo clippy --fix --bin "designos" -p designos-app` to apply 2 suggestions)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.88s
(0 warnings in crates/designos-render/src/text.rs, crates/designos-render/src/vectorops.rs, crates/designos-render/tests/vectorops.rs, crates/designos-app/src/fonts.rs, crates/designos-app/src/vecops.rs)

=== 03-test-doc-render.log: cargo test -p designos-doc -p designos-render --locked ===
test result: ok. 715 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s (designos-doc)
test result: ok. 230 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.87s (designos-render lib)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s (designos-render/tests/vectorops.rs)

=== 04-test-app-bin.log: cargo test -p designos-app --bin designos --locked -- --test-threads=8 ===
running 4007 tests
...
test vecops::tests::flatten_of_single_paint_text_gives_one_vector ... ok
test vecops::tests::flatten_of_two_paint_text_gives_two_vectors_in_a_group ... ok
test vecops::tests::mixed_selection_converts_text_and_shapes_together ... ok
test vecops::tests::one_undo_restores_text_node_exactly_in_json ... ok
test vecops::tests::outline_stroke_of_two_run_text_gives_one_vector_per_glyph_with_each_runs_fill ... ok
test vecops::tests::pixel_test_converted_text_vectors_match_original_text_render ... ok
test vecops::tests::text_to_vector_carries_over_fill_and_stroke_bindings ... ok
test vecops::tests::text_to_vector_loads_unloaded_font_and_refuses_unavailable_font ... ok
...
test result: ok. 3994 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out; finished in 36.26s

=== 05-test-workspace-lib.log: cargo test --workspace --locked --lib ===
designos-app:        3968 passed; 0 failed; 13 ignored
designos-diff:          5 passed; 0 failed; 0 ignored
designos-doc:         715 passed; 0 failed; 0 ignored
designos-eval:         20 passed; 0 failed; 0 ignored
designos-export:      385 passed; 0 failed; 0 ignored
designos-fig:         306 passed; 0 failed; 0 ignored
designos-fonts:        12 passed; 0 failed; 0 ignored
designos-gpu:          64 passed; 0 failed; 0 ignored
designos-library:      29 passed; 0 failed; 0 ignored
designos-mcp:          68 passed; 0 failed; 0 ignored
designos-render:      230 passed; 0 failed; 0 ignored
designos-svg:          22 passed; 0 failed; 0 ignored
designos-webcapture:    7 passed; 0 failed; 0 ignored
```

---

## 4. Fable Thinking — Epistemic Ledger, Weakest Link & Skills Loaded

### Epistemic Ledger
- **`FACT`**: `cosmic-text` shaping in `crates/designos-render/src/text.rs` produces quadratic and cubic Bezier glyph commands in the text node's local coordinate space (`[0..size.x, 0..size.y]`), and `VectorNetwork::from_paths` normalizes those paths to the tight glyph bounding box `(bounds.x, bounds.y, bounds.w, bounds.h)` while degree-elevating quadratic Beziers to cubic Beziers.
- **`FACT`**: Placing each converted glyph vector inside a `NodeKind::Group` with the text node's `transform` and local child offset `Affine::translate(bounds.x, bounds.y)` (or for single-vector Flatten, `n.transform * Affine::translate(bounds.x, bounds.y)`) preserves exact page-space glyph placement, including under arbitrary text node rotation/skew, with `non_edge_max = 0` pixel diff across both synthetic and Untitled UI kit renders (`max <= 29` on 1 px anti-aliased edges from quadratic-to-cubic degree elevation in `tiny-skia`).
- **`FACT`**: `Paint::LinearGradient` / `RadialGradient` / `AngularGradient` / `DiamondGradient` store normalized `[0..1]` handle coordinates relative to a node's `size`; remapping `p_new = (p_old * text_size - vec_offset) / vec_size` via `remap_paint_gradients` preserves gradient world positions when a text node is split into tighter glyph or run bounding boxes.
- **`INFERENCE`**: Carrying `fills/{i}/color` and `strokes/{i}/color` `VariableBinding`s onto converted vectors whenever a run's `fills` or `strokes` equal the text node's base `fills` or `strokes` preserves theme/token reactivity for single-style and inherited-style runs while avoiding misbinding overridden run colors.
- **`ASSUMPTION`**: For multi-run text with distinct paint sets, grouping one `NodeKind::Vector` per distinct paint set under a `NodeKind::Group` named after the text layer on **Flatten** matches user expectations for lossless color preservation (flagged in Section 1 to be confirmed against Figma).

### Weakest Link (One Line)
- Mixed-paint **Flatten** on multi-run text emits a `NodeKind::Group` with one `NodeKind::Vector` per distinct paint set (because `designos_doc::VectorNetwork` does not store per-region paint overrides), which needs confirmation against live Figma's multi-fill `VectorNetwork` representation.

### Skills Loaded & What Changed Because of Them
- **`es:astra-work`**: Enforced strict scope boundaries, verification of both synthetic and real Untitled UI kit renders before/after conversion, and complete cleanup of temporary build artifacts (`out/cargo`).
- **`es:fable-thinking`**: Guided root-cause analysis of the 1 px anti-aliased edge delta (`max=29`, `non_edge_max=0`) between `PathCmd::Quad` in `text::render_text` and degree-elevated cubics in `VectorNetwork::from_paths`, plus diagnosis of the macOS `sun_path` socket-length invariant in `agent_approval_channel::tests` (requiring `/private/tmp/x.XXXXXX` in the isolated test harness).
- **`es:rust`**: Used deterministic `Command::Batch` undo grouping so multi-vector and mixed-selection conversions revert in a single undo step (`doc.to_json()` bit-exact), and avoided mutating document state when any requested font fails to load.
- **`es:rust-gpu-canvas`**: Preserved local-to-world affine transform composition (`n.transform * Affine::translate(bounds.x, bounds.y)`) and normalized gradient handle remapping so CPU and GPU render pipelines place converted glyph vectors identically to live text.

---

## 5. Pull Request & Git Log

- **Branch**: `jangtrinh/agy-1469-text-to-vector`
- **Draft PR**: https://github.com/jangtrinh/design-os-studio/pull/1491
- **`git log --oneline origin/main..HEAD`**:
  ```text
  af6c702f feat(vector): Flatten and Outline stroke convert text to vector paths (#1469)
  ```

Status: PASS
