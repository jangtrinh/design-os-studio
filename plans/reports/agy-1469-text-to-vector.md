# Issue #1469 — Convert text to vector paths (Flatten and Outline stroke on text)

## 1. Re-anchor Findings & What Could Not Be Verified First (Round 1)

### What Could Not Be Verified Against Live Figma (`TO BE CONFIRMED AGAINST FIGMA`)
- **Mixed-paint Flatten result on multi-run text**: Per Ruling 3 of the task brief, **Flatten** (`⌘E` / `Ctrl+E`) on a single-paint text layer produces a single `NodeKind::Vector` holding every glyph, whereas when styled text runs have distinct fill paint sets, Flatten produces a `NodeKind::Group` named after the text layer containing one `NodeKind::Vector` child per distinct fill paint set in first-appearance order so no run colour is lost. Because opening live Figma is forbidden by the worker boundary, **this mixed-paint Flatten grouping behavior is to be confirmed against Figma**.
- **GPU vs CPU pixel comparison**: Headless Metal/Vulkan GPU rendering is unavailable in the sandboxed test environment without a display server, so visual pixel verification was performed deterministically using `designos_render::render` (`tiny-skia` CPU reference renderer) on both synthetic multi-run/decorated text fixtures and an imported Untitled UI PRO VARIABLES v8.0 `.fig` copy.

### Re-anchored Code Sites at Base (`93c8e407~2`)
1. **`crates/designos-app/src/vecops.rs:35-37` (`has_geometry`)**:
   - Defined `fn has_geometry(app: &App, id: NodeId) -> bool { app.doc.get(id).is_some_and(|n| !matches!(n.kind, NodeKind::Text(_) | NodeKind::Page { .. } | NodeKind::Unknown { .. })) }`, explicitly excluding `NodeKind::Text(_)` from `flatten_selection`.
2. **`crates/designos-app/src/vecops.rs:70-108` (`flatten_selection`)**:
   - Filtered `self.same_parent_tops()` with `has_geometry(self, *id)`, rejecting text selections with `"Select shapes under the same parent to flatten"`. For non-text selections, it called `flatten_nodes(&self.doc, &ids, &self.doc.world_transform(parent))` and replaced all selected nodes with a single `NodeKind::Vector` copying properties from `ids[0]`.
3. **`crates/designos-app/src/vecops.rs:111-159` (`outline_stroke_selection`)**:
   - Iterated `self.top_selection()`, skipped any node where `!n.stroke.paints.iter().any(|p| p.visible()) || n.stroke.weight <= 0.0`, and called `outline_stroke(&paths, &n.stroke, net.as_ref())`. For a filled text layer without a visible stroke, `outline_stroke_selection` was a no-op with `"Selection has no visible strokes"`.
4. **`crates/designos-render/src/vectorops.rs:48-66` (`node_paths`)**:
   - Documented `/// Local-space outline paths of a single shape node. Text and containers have none.` and delegated to `node_shape(doc, n, 0.05)?`, returning `None` for `NodeKind::Text(_)`. Consequently `collect_paths` (`vectorops.rs:437-466`) and `flatten_nodes` (`vectorops.rs:475-496`) produced no geometry for text layers.
5. **`crates/designos-render/tests/vectorops.rs:171-178` (`flatten_skips_text_and_empty_selections`)**:
   - Asserted `assert!(flatten_nodes(&d, &[t], &Affine::IDENTITY).is_none());`, codifying that `flatten_nodes` ignored text nodes.
6. **`crates/designos-app/src/fonts.rs:67-93` (`FontLoader`)**:
   - Provided `load_for(&mut self, doc: &Document)`, `load_for_with`, and `request_new_faces` to spawn background font loads, with no check or deferral path in `flatten_selection` / `outline_stroke_selection` for unloaded or failed text fonts.

---

## 2. Implementation Summary (Round 1 — `93c8e407`)

- **`crates/designos-render/src/text.rs:611-695`**:
  - Added `pub struct ShapedGlyphOutline { pub style: usize, pub text: String, pub path: sk::Path }` (`text.rs:612-619`) and `pub struct TextGlyphOutlines { pub styles: Vec<d::TextStyle>, pub glyphs: Vec<ShapedGlyphOutline> }` (`text.rs:623-627`).
  - Added `pub fn glyph_outlines(props: &TextProps, size: Vec2) -> TextGlyphOutlines` (`text.rs:631-633`) and `fn compute_glyph_outlines` (`text.rs:635-695`), reusing `compute_layout` so `cosmic-text` shaping, list markers (bullet/number glyphs and checkbox rounded rects), and underline/strikethrough bars are emitted in paint order with their style index (`0` = base style, `i + 1` = `runs[i].style`).
- **`crates/designos-render/src/vectorops.rs:48-116` & `crates/designos-render/tests/vectorops.rs:171-181`**:
  - Added `pub struct TextVectorRun { pub name: String, pub path: d::Path, pub fills: Vec<d::Paint>, pub uses_base_fill: bool }` (`vectorops.rs:48-54`).
  - Added `pub fn text_flatten_runs(doc: &Document, n: &Node) -> Vec<TextVectorRun>` (`vectorops.rs:58-80`), which resolves variable-bound base fills/strokes on the text node via `resolve_node_paints` and groups shaped glyph/marker/decoration outlines by distinct `style.fills` (`r.fills == style.fills`) in first-appearance order.
  - Added `pub fn text_glyph_runs(doc: &Document, n: &Node) -> Vec<TextVectorRun>` (`vectorops.rs:84-103`), which emits one `TextVectorRun` per shaped glyph, list marker, and decoration bar.
  - Updated `pub fn node_paths(doc: &Document, n: &Node) -> Option<Vec<d::Path>>` (`vectorops.rs:107-116`) to return `text_flatten_runs(doc, n)` paths for `NodeKind::Text(_)`, enabling `flatten_nodes` (`vectorops.rs:487-508`) to include text geometry in mixed shape+text flattens.
  - Updated `crates/designos-render/tests/vectorops.rs:171-181` (`flatten_converts_text_and_skips_empty_selections`) to assert `flatten_nodes(&d, &[t], &Affine::IDENTITY)` returns non-empty vector geometry for text while `flatten_nodes(&d, &[], &Affine::IDENTITY)` returns `None`.
- **`crates/designos-app/src/fonts.rs:162-215` (at `93c8e407`)**:
  - In Round 1 (`93c8e407`), added `FontLoader::ensure_loaded` checking `self.available(&requests)` (superseded in Round 2 by non-blocking `FontLoader::check_requests` + `pending_vecop`; see `## Round 2` below).
- **`crates/designos-app/src/vecops.rs` (at `93c8e407`)**:
  - Added `remap_paint_gradients` and `carry_paint_bindings` to remap gradient handles from the text box to each vector's local bounding box and carry `fills/{i}/color`, `strokes/{i}/color`, and `style_refs` ("fill" / "stroke") onto converted vectors.
  - Updated `flatten_selection`: pure text selections convert each text node into a single `NodeKind::Vector` (when all runs share one paint set) or a `NodeKind::Group` with one `NodeKind::Vector` child per distinct fill paint set; mixed text+shape selections under the same parent merge into a single `NodeKind::Vector` via `flatten_nodes`.
  - Updated `outline_stroke_selection`: converts each selected `NodeKind::Text` layer into a `NodeKind::Group` named after the text layer containing one `NodeKind::Vector` child per glyph/marker/decoration, alongside existing stroke outlining for non-text shapes, in a single `Command::Batch` undo step.

---

## 3. Round 1 Acceptance Criteria & Verification Table (Corrected from `01-fmt.log` .. `06-test-vecops-nocapture.log`)

| # | Criterion | Command | Result / Output |
|---|---|---|---|
| 1 | Unit tests in `vecops::tests` (`93c8e407`) | `cargo test -p designos-app --bin designos --locked vecops::tests:: -- --nocapture` | `23 passed; 0 failed; 0 ignored; 0 measured; 3984 filtered out` (`06-test-vecops-nocapture.log`) |
| 2 | Pixel test (`93c8e407`) | `cargo test -p designos-app --bin designos --locked vecops::tests::pixel_test_converted_text_vectors_match_original_text_render -- --nocapture` | `PIXEL_DIFF_FLATTEN: max=29 mean=0.005257 non_edge_max=0`, `PIXEL_DIFF_OUTLINE_STROKE: max=29 mean=0.005257 non_edge_max=0` (`06-test-vecops-nocapture.log`) |
| 3 | Kit test (`93c8e407`) | Headless CPU render on an APFS copy of Untitled UI PRO VARIABLES v8.0 `.fig` | Wrote `out/evidence/1469-kit-before.png`, `out/evidence/1469-kit-after-flatten.png`, `out/evidence/1469-kit-after-outline-stroke.png` |
| 4 | Round 1 Gates (`01-fmt.log` .. `05-test-workspace-lib.log`) | `cargo fmt --all -- --check`, `cargo clippy --workspace --locked`, `cargo test -p designos-doc -p designos-render --locked`, `cargo test -p designos-app --bin designos --locked -- --test-threads=8`, `cargo test --workspace --locked --lib` | `01-fmt.log`: exit 0; `02-clippy.log`: exit 0 (2 pre-existing warnings in `agent_permissions.rs`); `03-test-doc-render.log`: `designos-doc` lib `706 passed`, `designos-render` lib `56 passed; 2 ignored`, `tests/vectorops.rs` `23 passed` (plus other render integration test binaries); `04-test-app-bin.log`: `4007 passed; 0 failed`; `05-test-workspace-lib.log`: `designos-diff: 13`, `designos-doc: 706`, `designos-eval: 1`, `designos-export: 153 (1 ignored)`, `designos-fig: 143`, `designos-fonts: 30`, `designos-gpu: 4`, `designos-library: 37`, `designos-mcp: 322`, `designos-render: 56 (2 ignored)`, `designos-svg: 1`, `designos-webcapture: 8` (`designos-app` is bin-only and does not run under `--workspace --lib`) |

### Pasted Round 1 Gate Outputs (`01-fmt.log` .. `05-test-workspace-lib.log`)

```text
=== 01-fmt.log: cargo fmt --all -- --check ===
(exit 0, 0 diffs)

=== 02-clippy.log: cargo clippy --workspace --locked ===
warning: `designos-app` (bin "designos") generated 2 warnings (run `cargo clippy --fix --bin "designos" -p designos-app` to apply 2 suggestions)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.88s

=== 03-test-doc-render.log: cargo test -p designos-doc -p designos-render --locked ===
Running unittests src/lib.rs (designos_doc): test result: ok. 706 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (designos_render): test result: ok. 56 passed; 0 failed; 2 ignored
Running tests/vectorops.rs: test result: ok. 23 passed; 0 failed; 0 ignored

=== 04-test-app-bin.log: cargo test -p designos-app --bin designos --locked -- --test-threads=8 ===
test result: ok. 4007 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 52.87s

=== 05-test-workspace-lib.log: cargo test --workspace --locked --lib ===
designos-diff:        13 passed; 0 failed; 0 ignored
designos-doc:        706 passed; 0 failed; 0 ignored
designos-eval:         1 passed; 0 failed; 0 ignored
designos-export:     153 passed; 0 failed; 1 ignored
designos-fig:        143 passed; 0 failed; 0 ignored
designos-fonts:       30 passed; 0 failed; 0 ignored
designos-gpu:          4 passed; 0 failed; 0 ignored
designos-library:     37 passed; 0 failed; 0 ignored
designos-mcp:        322 passed; 0 failed; 0 ignored
designos-render:      56 passed; 0 failed; 2 ignored
designos-svg:          1 passed; 0 failed; 0 ignored
designos-webcapture:   8 passed; 0 failed; 0 ignored
```

---

## Round 2

### 1. What Could Not Be Verified First (Round 2)
- **Live Figma comparison (`TO BE CONFIRMED AGAINST FIGMA`)**: Confirming Figma's exact node structure on multi-paint text Flatten (Group of per-paint Vectors vs single Vector with multi-fill regions) and non-center text stroke alignment remains `TO BE CONFIRMED AGAINST FIGMA` because live Figma is outside the worker boundary.
- **GPU vs CPU pixel comparison**: Headless tests run on the deterministic `designos_render::render` (`tiny-skia`) CPU reference renderer. Both `designos-render` (`crates/designos-render/src/text.rs:424-441`) and `designos-gpu` (`crates/designos-gpu/src/compile.rs:1079-1089`) stroke text glyph outlines with centered strokes (`StrokeAlign::Center`).

### 2. Round 2 Rulings & Code Changes (`B1, B2, M1, M2, M3, m1, m2, m3, L1, M4`)
- **B1 — Preserve fixed glyph geometry inside Auto-Layout & run layout before `run_tx` (`crates/designos-app/src/vecops.rs:145-150, 211, 232, 237, 250-251, 304, 323-326, 369-372`)**:
  - Added `set_fixed_layout_child(lc: &mut LayoutChild, counter: Option<bool>)` (`vecops.rs:145-150`), which calls `lc.set_axis_sizing(false, Sizing::Fixed, counter)`, `lc.set_axis_sizing(true, Sizing::Fixed, counter)`, `lc.align_self = AlignSelf::Auto`, and `lc.grow = 0.0`.
  - Applied `set_fixed_layout_child` to every vector and group produced from text in `text_run_to_single_vector` (`vecops.rs:211`), `text_runs_to_group` (`vecops.rs:232, 237`), and the mixed-flatten text template (`vecops.rs:304`), so `LayoutChild::sizing_for` (`crates/designos-doc/src/model.rs:766-795`) never stretches or collapses converted glyphs on either axis when an Auto-Layout parent had `Sizing::Fill` or `Sizing::Hug` (`main_sizing` or `counter_sizing`).
  - Called `crate::layout::fit_group(&mut self.doc, new_id)` (when the result is a `Group`) and `crate::layout::relayout_up(&mut self.doc, new_id)` before `self.run_tx` in both `flatten_selection` (`vecops.rs:323-326`) and `outline_stroke_selection` (`vecops.rs:369-372`), and inside `text_runs_to_group` (`vecops.rs:250-251`), so the initial commit and Undo -> Redo produce the exact same Auto-Layout state.
- **B2 — Centered stroke alignment on converted text vectors (`crates/designos-app/src/vecops.rs:213, 233, 305`)**:
  - Both `designos-render` (`crates/designos-render/src/text.rs:424-441`) and `designos-gpu` (`crates/designos-gpu/src/compile.rs:1079-1089`) stroke text outlines with `tiny_skia` / `lyon` centered strokes regardless of `n.stroke.align`, whereas `NodeKind::Vector` renders `StrokeAlign::Inside` and `StrokeAlign::Outside` via boolean inset/outset (`crates/designos-render/src/render.rs:661-687`).
  - Set `stroke.align = StrokeAlign::Center` on every vector produced from text (`template.stroke.align = StrokeAlign::Center` at `vecops.rs:213`, `child_template.stroke.align = StrokeAlign::Center` at `vecops.rs:233`, and `base.stroke.align = StrokeAlign::Center` at `vecops.rs:305`) so stroked text with `Inside`, `Outside`, or `Center` renders identically before and after conversion (`max=15, mean_ink=0.001390, non_edge_max=0` on Flatten; `max=43, mean_ink=0.002209, non_edge_max=0` on Outline stroke).
- **M1 — Non-blocking font loading for Flatten / Outline stroke (`crates/designos-app/src/fonts.rs:30, 48, 162-215, 219-221`, `crates/designos-app/src/vecops.rs:64-68, 153-189`, `crates/designos-app/src/app.rs:395-397`)**:
  - Removed the synchronous `FontLoader::ensure_loaded` method that called `designos_fonts::ensure_font_requests_with` on the UI thread.
  - Added `FontLoader::check_requests(&self, requests: &BTreeSet<FontRequest>) -> (Vec<FontRequest>, Vec<String>)` (`fonts.rs:162-215`), which checks `self.failed` first (immediately returning failed family names without re-requesting), checks `self.loaded` and `designos_render::has_font_family`, and returns `(missing, failed)` without network or disk I/O on the UI thread.
  - Added `pending_vecop: Option<(u64, crate::vecops::PendingVecOp, BTreeSet<FontRequest>)>` on `FontLoader` (`fonts.rs:30`) and `App::ensure_text_fonts` / `App::finish_pending_vecop` (`vecops.rs:153-189`). When missing fonts are needed, `ensure_text_fonts` queues them via `self.fonts.request_new_faces(&self.doc, missing)`, stores `(self.doc.runtime_id(), op, requests)` in `self.fonts.pending_vecop`, shows `"Loading fonts…"`, and returns `false` without mutating the document.
  - In `App::poll_fonts` (`crates/designos-app/src/app.rs:395-397`), once `!self.fonts.is_loading() && !self.text_refit_pending()`, `self.finish_pending_vecop()` runs the deferred `Flatten` or `OutlineStroke` if `self.doc.runtime_id() == doc_id` and all requested faces succeeded, or displays `"Font unavailable: ..."` if any face landed in `self.fonts.failed`.
- **M2 — Scope `collect_text_font_requests` by operation (`crates/designos-app/src/vecops.rs:70-101, 265, 349`)**:
  - `collect_text_font_requests(doc: &Document, ids: &[NodeId], walk_subtree: bool)` (`vecops.rs:70-101`) walks descendant subtrees only when `walk_subtree` is `true` (`flatten_selection` at `vecops.rs:265`, where `flatten_nodes` recurses into containers) and checks only the directly selected top-level nodes when `walk_subtree` is `false` (`outline_stroke_selection` at `vecops.rs:349`, which only outlines top-level selected nodes).
- **M3 — Preserve fill variable bindings and style links after gradient handle remapping (`crates/designos-app/src/vecops.rs:118-142, 206-210, 238-246`)**:
  - In `text_run_to_single_vector` (`vecops.rs:206-210`) and `text_runs_to_group` (`vecops.rs:238-246`), `run.uses_base_fill` is already computed before gradient handle remapping. Passing `let base_fills: &[Paint] = &[];` into `carry_paint_bindings(raw, base_fills, &mut ..., run.uses_base_fill, true)` ensures `inherit_fill` is taken directly from `run.uses_base_fill` instead of comparing remapped gradient handle coordinates against un-remapped `resolved_base_fills`.
- **m1 — Preserve `visible` and `locked` flags (`crates/designos-app/src/vecops.rs:209-210, 248-249`)**:
  - Copied `raw.visible` and `raw.locked` onto both the single vector in `text_run_to_single_vector` (`vecops.rs:209-210`) and the parent group in `text_runs_to_group` (`vecops.rs:248-249`).
- **m2 — Fit Outline stroke group to children's union at commit (`crates/designos-app/src/vecops.rs:250-251, 323-325, 369-371`)**:
  - `text_runs_to_group` calls `crate::layout::fit_group(doc, gid)` (`vecops.rs:251`) and `flatten_selection` / `outline_stroke_selection` call `fit_group` + `relayout_up` before `run_tx`, so the group's bounding box tightly wraps its glyph children's union immediately upon conversion.
- **m3 — Strengthened pixel & unit tests + mutation matrix (`crates/designos-app/src/vecops.rs:1441-1917`)**:
  - `pixel_test_converted_text_vectors_match_original_text_render` asserts that original text `NodeId`s are removed from `app.doc` and replaced by `NodeKind::Vector` / `NodeKind::Group` nodes after conversion, computes `mean_ink` (mean channel delta over non-white ink pixels where `min_chan < 250` in either image) alongside `mean_all`, `max_diff`, `hard_pixels`, and `non_edge_max`, and verifies:
    1. Multi-run styled text with underline, bullet list marker, `opacity = 0.6`, drop shadow, and stroke (`mean_ink <= 0.02`, `non_edge_max == 0`).
    2. LinearGradient-filled text (`mean_ink <= 0.03`, `non_edge_max == 0`).
    3. Stroked text with `StrokeAlign::Inside`, `StrokeAlign::Outside`, and `StrokeAlign::Center` under both Flatten and Outline stroke (`mean_ink <= 0.02`, `non_edge_max == 0`).
- **L1 — Clean test string & non-empty run lookup (`crates/designos-app/src/vecops.rs:297, 1749`)**:
  - Replaced `"1469 Vector Text"` with `"Vector Text"` (`vecops.rs:1749`) and changed mixed-selection text run lookup from `tp.runs.first()` to `tp.runs.iter().find(|r| r.start == 0 && r.end > 0)` (`vecops.rs:297`).
- **M4 — Re-anchored report from git history & actual log files**:
  - Rewrote §1–§3 above directly from `git show 93c8e407~2:...`, `git show 93c8e407:...`, and `01-fmt.log` .. `06-test-vecops-nocapture.log`.

---

### 3. Round 2 Acceptance Criteria & Verification Table

| # | Criterion | Command | Result / Output |
|---|---|---|---|
| 1a | RED on `93c8e407`: new/updated Round 2 tests fail on Round 1 production code | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test -p designos-app --bin designos --locked vecops::tests::` (with Round 2 tests on `93c8e407` code) | `20 passed; 6 failed` — all 6 Round 2 tests failed on `93c8e407` (`r2-01-red-on-93c8e407.log`) |
| 1b | GREEN on `HEAD`: all `vecops::tests` pass on Round 2 production code | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test -p designos-app --bin designos --locked vecops::tests:: -- --nocapture` | `26 passed; 0 failed` in `1.48s` (`r2-02-green-vecops.log`) |
| 1c | Mutation check: `N0-null`, `N1`..`N13` (13 review mutants), and `R2-B1`, `R2-B2`, `R2-M2`, `R2-M3`, `R2-m1`, `R2-m2` (6 Round 2 mutants) | One-mutant-at-a-time test harness (`r2-03-mutants.log`) | `N0-null`: `SURVIVED`; `N1`..`N13`: all `13 KILLED` at runtime; `R2-B1`..`R2-m2`: all `6 KILLED` at runtime (`19/19` real mutants killed) |
| 2a | Visual pixel sheet (`plans/reports/agy-1469-text-to-vector-evidence/r2-pixel-sheet.png`) | Render before / after Flatten / after Outline stroke / 8x diff across `Inside`, `Outside`, `Center` strokes, Auto-Layout `Fill` + sibling edit, and `LinearGradient` (`r2-04-visual-and-kit.log`) | Composed `1624x976` contact sheet `r2-pixel-sheet.png` and visually inspected; `hard_px=0, non_edge_max=0` on every row; Auto-Layout `Fill` vector stays `(10.00, 10.00, 242.9414 x 23.28125, Fixed)` before and after sibling edit |
| 2b | Untitled UI kit 36px heading (`out/evidence/1469-r2-kit-*.png`) | Convert 36px heading `NodeId(5699) "Layout"` in frame `NodeId(5509) "Layout"` on an APFS copy of Untitled UI PRO VARIABLES v8.0 `.fig`, render full frame & 1:1 crop (`r2-04-visual-and-kit.log`) | Saved untracked `out/evidence/1469-r2-kit-before.png`, `1469-r2-kit-after-flatten.png`, `1469-r2-kit-after-outline.png`, `1469-r2-kit-heading-crop.png` (`972x180`); Flatten & Outline (6 glyph kids): `max=29, mean_all=0.000178, mean_ink=0.003809, hard_px=0, non_edge_max=0, ink_px=45882` |
| 3a | Gate 1: Rustfmt | `cargo fmt --all -- --check` | Exit `0`, 0 diffs (`r2-05-fmt.log`) |
| 3b | Gate 2: Clippy | `CARGO_TARGET_DIR="$PWD/out/cargo" cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::needless_return` (plus raw `-D warnings` check in `r2-06-clippy.log`) | Exit `0`, 0 warnings in all workspace targets (`r2-06-clippy.log`; raw `-D warnings` shows only `main`'s 2 pre-existing `clippy::needless_return` lines in `crates/designos-app/src/agent_permissions.rs:1176,1194`) |
| 3c | Gate 3: App `vecops` tests | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test -p designos-app --bin designos --locked vecops` | `26 passed; 0 failed; 0 ignored; 0 measured; 4022 filtered out; finished in 1.51s` (`r2-07-test-vecops.log`) |
| 3d | Gate 4: `designos-render` tests | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test -p designos-render --locked` | `56 passed; 0 failed; 2 ignored` (lib) + `23 passed; 0 failed` (`tests/vectorops.rs`) + all 21 other integration test targets `0 failed` (`r2-08-test-render.log`) |
| 3e | Gate 5: Workspace `--lib` tests | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test --workspace --locked --lib` | `13 + 708 + 1 + 153 + 143 + 30 + 4 + 37 + 322 + 56 + 1 + 8 = 1,476 passed; 0 failed; 3 ignored` across all 12 library crates (`r2-09-test-workspace-lib.log`) |

### Pasted Round 2 Evidence Logs (`r2-01` .. `r2-09`)

```text
=== r2-01-red-on-93c8e407.log (RED on 93c8e407 production code) ===
failures:
    vecops::tests::flatten_of_two_paint_text_gives_two_vectors_in_a_group
    vecops::tests::outline_stroke_of_two_run_text_gives_one_vector_per_glyph_with_each_runs_fill
    vecops::tests::pixel_test_converted_text_vectors_match_original_text_render
    vecops::tests::text_in_auto_layout_keeps_fixed_size_after_relayout_and_redo_matches_commit
    vecops::tests::text_to_vector_preserves_gradient_style_links_and_merges_shared_paint_runs_without_leaking_bindings
    vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope

test result: FAILED. 20 passed; 6 failed; 0 ignored; 0 measured; 4020 filtered out; finished in 1.29s

=== r2-02-green-vecops.log & r2-07-test-vecops.log (GREEN on Round 2 HEAD) ===
running 26 tests
...
test vecops::tests::one_undo_restores_text_node_exactly_in_json ... ok
test vecops::tests::outline_stroke_of_two_run_text_gives_one_vector_per_glyph_with_each_runs_fill ... ok
test vecops::tests::mixed_selection_converts_text_and_shapes_together ... ok
test vecops::tests::flatten_of_two_paint_text_gives_two_vectors_in_a_group ... ok
test vecops::tests::flatten_of_single_paint_text_gives_one_vector ... ok
test vecops::tests::text_in_auto_layout_keeps_fixed_size_after_relayout_and_redo_matches_commit ... ok
test vecops::tests::text_to_vector_carries_over_fill_and_stroke_bindings ... ok
test vecops::tests::text_to_vector_preserves_gradient_style_links_and_merges_shared_paint_runs_without_leaking_bindings ... ok
test vecops::tests::text_to_vector_loads_unloaded_font_and_refuses_unavailable_font ... ok
test vecops::tests::pixel_test_converted_text_vectors_match_original_text_render ... ok
test vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 4022 filtered out; finished in 1.51s

=== r2-03-mutants.log (13 review mutants N1..N13 + 6 Round 2 mutants R2-*) ===
N0-null    | SURVIVED |  59.1s | null control
N1         | KILLED   |  13.9s | flatten never merges runs that share a paint set | test vecops::tests::text_to_vector_preserves_gradient_style_links_and_merges_shared_paint_runs_without_leaking_bindings ... FAILED
N2         | KILLED   |  17.6s | every flatten run treated as base fill (bindings leak to recoloured runs) | test vecops::tests::text_to_vector_preserves_gradient_style_links_and_merges_shared_paint_runs_without_leaking_bindings ... FAILED
N3         | KILLED   |  23.0s | stroke bindings dropped | test vecops::tests::text_to_vector_carries_over_fill_and_stroke_bindings ... FAILED
N4         | KILLED   |  29.7s | gradient handles not remapped | test vecops::tests::pixel_test_converted_text_vectors_match_original_text_render ... FAILED
N5         | KILLED   |  33.0s | outline stroke drops underline bars | test vecops::tests::pixel_test_converted_text_vectors_match_original_text_render ... FAILED
N6         | KILLED   |  47.9s | outline stroke drops list markers | test vecops::tests::pixel_test_converted_text_vectors_match_original_text_render ... FAILED
N7         | KILLED   |  36.4s | font check bypassed | test vecops::tests::text_to_vector_loads_unloaded_font_and_refuses_unavailable_font ... FAILED
N8         | KILLED   |  47.6s | group loses the text transform | test vecops::tests::outline_stroke_of_two_run_text_gives_one_vector_per_glyph_with_each_runs_fill ... FAILED
N9         | KILLED   |  13.9s | node_paths has no text arm (mixed flatten skips text) | test flatten_converts_text_and_skips_empty_selections ... FAILED
N10        | KILLED   |  92.5s | glyph vectors keep the text effects (double shadow) | test vecops::tests::pixel_test_converted_text_vectors_match_original_text_render ... FAILED
N11        | KILLED   |  37.5s | single vector drops fill bindings | test vecops::tests::text_to_vector_carries_over_fill_and_stroke_bindings ... FAILED
N12        | KILLED   |  26.2s | outline stroke glyphs shifted 1 px right | test vecops::tests::pixel_test_converted_text_vectors_match_original_text_render ... FAILED
N13        | KILLED   |  45.5s | child vectors keep the text opacity (double opacity) | test vecops::tests::pixel_test_converted_text_vectors_match_original_text_render ... FAILED
R2-B1      | KILLED   |  14.0s | converted text keeps Sizing::Fill instead of Fixed on both axes | test vecops::tests::text_in_auto_layout_keeps_fixed_size_after_relayout_and_redo_matches_commit ... FAILED
R2-B2      | KILLED   |  23.5s | converted text vector keeps StrokeAlign::Inside instead of Center | test vecops::tests::pixel_test_converted_text_vectors_match_original_text_render ... FAILED
R2-M2      | KILLED   |  42.3s | outline stroke walks subtree for font requests instead of top-level only | test vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope ... FAILED
R2-M3      | KILLED   |  25.2s | carry_paint_bindings compares remapped gradient handles against un-remapped base_fills | test vecops::tests::text_to_vector_preserves_gradient_style_links_and_merges_shared_paint_runs_without_leaking_bindings ... FAILED
R2-m1      | KILLED   |  19.1s | converted text vector drops visible and locked flags | test vecops::tests::text_in_auto_layout_keeps_fixed_size_after_relayout_and_redo_matches_commit ... FAILED
R2-m2      | KILLED   |  27.7s | outline stroke group box not fitted to children union at commit | test vecops::tests::outline_stroke_of_two_run_text_gives_one_vector_per_glyph_with_each_runs_fill ... FAILED

=== r2-04-visual-and-kit.log (Visual pixel sheet & Untitled UI 36px heading) ===
R2_SHEET stroke_inside op=Flatten max=15 mean_all=0.000271 mean_ink=0.001390 hard_px=0 non_edge_max=0 ink_px=44594
R2_SHEET stroke_inside op=Outline max=43 mean_all=0.000431 mean_ink=0.002209 hard_px=0 non_edge_max=0 ink_px=44594
R2_SHEET stroke_outside op=Flatten max=15 mean_all=0.000271 mean_ink=0.001390 hard_px=0 non_edge_max=0 ink_px=44594
R2_SHEET stroke_outside op=Outline max=43 mean_all=0.000431 mean_ink=0.002209 hard_px=0 non_edge_max=0 ink_px=44594
R2_SHEET stroke_center op=Flatten max=15 mean_all=0.000271 mean_ink=0.001390 hard_px=0 non_edge_max=0 ink_px=44594
R2_SHEET stroke_center op=Outline max=43 mean_all=0.000431 mean_ink=0.002209 hard_px=0 non_edge_max=0 ink_px=44594
R2_SHEET al_fill commit_vec=(10.00,10.00,Vec2 { x: 242.9414, y: 23.28125 },Fixed) after_edit_vec=(10.00,10.00,Vec2 { x: 242.9414, y: 23.28125 },Fixed) parent=Vec2 { x: 520.0, y: 81.28125 }->Vec2 { x: 520.0, y: 81.28125 }
R2_SHEET gradient op=Flatten max=56 mean_all=0.038383 mean_ink=0.257559 hard_px=0 non_edge_max=0 ink_px=34097
R2_SHEET gradient op=Outline max=56 mean_all=0.038383 mean_ink=0.257559 hard_px=0 non_edge_max=0 ink_px=34097
R2_KIT frame_id=NodeId(5509) frame_name="Layout" heading_id=NodeId(5699) font_size=36 text="Layout"
R2_KIT op=Flatten out_id=NodeId(225113) out_kind=Discriminant(9) size=Vec2 { x: 110.72813, y: 33.671875 } max=29 mean_all=0.000178 mean_ink=0.003809 hard_px=0 non_edge_max=0 ink_px=45882
R2_KIT op=Outline out_id=NodeId(225113) out_kind=Discriminant(2) kids=6 size=Vec2 { x: 110.72813, y: 33.671875 } max=29 mean_all=0.000178 mean_ink=0.003809 hard_px=0 non_edge_max=0 ink_px=45882

=== r2-05-fmt.log ===
(exit 0, 0 diffs)

=== r2-06-clippy.log ===
--- cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::needless_return ---
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 38.81s
(0 warnings, exit 0)

=== r2-08-test-render.log: cargo test -p designos-render --locked ===
Running unittests src/lib.rs: test result: ok. 56 passed; 0 failed; 2 ignored
Running tests/vectorops.rs: test result: ok. 23 passed; 0 failed; 0 ignored
(all 21 other render integration test binaries: 0 failed)

=== r2-09-test-workspace-lib.log: cargo test --workspace --locked --lib ===
designos-diff:        13 passed; 0 failed; 0 ignored
designos-doc:        708 passed; 0 failed; 0 ignored
designos-eval:         1 passed; 0 failed; 0 ignored
designos-export:     153 passed; 0 failed; 1 ignored
designos-fig:        143 passed; 0 failed; 0 ignored
designos-fonts:       30 passed; 0 failed; 0 ignored
designos-gpu:          4 passed; 0 failed; 0 ignored
designos-library:     37 passed; 0 failed; 0 ignored
designos-mcp:        322 passed; 0 failed; 0 ignored
designos-render:      56 passed; 0 failed; 2 ignored
designos-svg:          1 passed; 0 failed; 0 ignored
designos-webcapture:   8 passed; 0 failed; 0 ignored
```

### 4. Epistemic Ledger, Weakest Link & Skills Loaded (Round 2)

#### Epistemic Ledger
- **`FACT`**: In `crates/designos-doc/src/model.rs:766-795` (`LayoutChild::sizing_for`), `LayoutChild` stores a primary `sizing` plus an optional `counter_sizing` tied to `counter_is_vertical`. Calling `set_axis_sizing(false, Sizing::Fixed, counter)` and `set_axis_sizing(true, Sizing::Fixed, counter)` alongside `align_self = AlignSelf::Auto` and `grow = 0.0` guarantees `Sizing::Fixed` on both axes regardless of parent Auto-Layout direction.
- **`FACT`**: Both `designos-render` (`crates/designos-render/src/text.rs:424-441`) and `designos-gpu` (`crates/designos-gpu/src/compile.rs:1079-1089`) stroke glyph paths with centered strokes; forcing `stroke.align = StrokeAlign::Center` on converted text vectors eliminates the `StrokeAlign::Inside` / `StrokeAlign::Outside` inset/outset discrepancy (`non_edge_max = 0`, `mean_ink = 0.001390` on Flatten and `0.002209` on Outline stroke across all three alignments).
- **`FACT`**: `FontLoader::check_requests` + `pending_vecop` in `App::poll_fonts` (`crates/designos-app/src/app.rs:395-397`) runs `finish_pending_vecop()` only after `!self.fonts.is_loading() && !self.text_refit_pending()`, so Auto-Width/Auto-Height text nodes are refitted with the newly loaded font before glyph outlining runs, without blocking the UI thread.
- **`INFERENCE`**: Keying `pending_vecop` by `self.doc.runtime_id()` prevents a deferred Flatten or Outline stroke from firing on a different document if the user switches or closes documents while fonts are downloading.
- **`ASSUMPTION`**: If the user changes the selection while a font download is in flight on the same document, re-running `flatten_selection` / `outline_stroke_selection` on the current selection in `finish_pending_vecop()` matches expected interactive behavior (and if the new selection requires another unloaded font, it cleanly re-queues).

#### Weakest Link (One Line)
- `finish_pending_vecop` re-invokes `flatten_selection()` / `outline_stroke_selection()` on the current selection when the background font load finishes, assuming the user has not changed selection during the sub-second font load.

#### Skills Loaded & What Changed Because of Them
- **`es:astra-work`**: Enforced strict RED-before-GREEN verification on `93c8e407`, 19-mutant runtime kill verification, visual inspection of both synthetic and Untitled UI 36px heading renders, and exact log transcription in the report.
- **`es:fable-thinking`**: Guided root-cause verification of the 5 Round 1 defects (`LayoutChild::sizing_for` dual-axis sizing, text centered stroke parity, non-blocking `FontLoader` queue + refit ordering, subtree-scoped font collection, and gradient `uses_base_fill` preservation).
- **`es:rust`**: Kept `FontLoader::check_requests` zero-I/O on the UI thread and preserved single-step `Command::Batch` undo/redo invariants across `fit_group` and `relayout_up`.
- **`es:rust-gpu-canvas`**: Verified that `designos-gpu` (`compile.rs:1079-1089`) and `designos-render` (`text.rs:424-441`) share centered text stroke semantics, ensuring `StrokeAlign::Center` on converted text vectors preserves CPU/GPU parity.

### 5. Git Log (`origin/main..HEAD`)

```text
33d7d0e6 fix(vector): preserve auto-layout sizing, centered text strokes, and async font loading (#1469)
14188742 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1469-text-to-vector
93c8e407 docs(report): record final commit hash for #1469
af6c702f feat(vector): Flatten and Outline stroke convert text to vector paths (#1469)
```

Status: DONE
Summary: Fixed Auto-Layout fixed sizing and pre-tx relayout (B1), centered stroke alignment on converted text vectors (B2), non-blocking font queueing via FontLoader::check_requests and poll_fonts (M1), operation-scoped font collection (M2), gradient fill binding/style_ref preservation (M3), visible/locked preservation (m1), Outline stroke group fitting (m2), ink-normalized pixel/mutant tests killing all 19 mutants (m3), and re-anchored the report from git history and actual logs (M4).
Concerns/Blockers: none
