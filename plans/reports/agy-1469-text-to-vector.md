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
- **GPU vs CPU pixel comparison**: Headless tests run on the deterministic `designos_render::render_node` (`tiny-skia`) CPU reference renderer (`crates/designos-app/src/vecops.rs:1632, 1642, 1662`). CPU text rendering strokes glyph paths with `self.make_stroke(n, n.stroke.weight)` and never reads `stroke.align` (`crates/designos-render/src/lib.rs:795-801`), while GPU text rendering fills a prebuilt centered stroke mesh (`crates/designos-gpu/src/compile.rs:1102-1108`).

### 2. Round 2 Rulings & Code Changes (`B1, B2, M1, M2, M3, m1, m2, m3, L1, M4`)
- **B1 — Preserve fixed glyph geometry inside Auto-Layout & run layout before `run_tx` (`crates/designos-app/src/vecops.rs:153-158, 228, 247, 270, 332-336, 370, 418-419`)**:
  - Added `set_fixed_layout_child(lc: &mut LayoutChild, counter: Option<bool>)` (`crates/designos-app/src/vecops.rs:153-158`), which calls `lc.set_axis_sizing(true, Sizing::Fixed, counter)` and `lc.set_axis_sizing(false, Sizing::Fixed, counter)` (`crates/designos-doc/src/layout_solver_types.rs:151-167`), `lc.align_self = AlignSelf::Auto`, and `lc.grow = 0.0`.
  - Applied `set_fixed_layout_child` using `LayoutChild::counter_axis_is_horizontal` (`crates/designos-doc/src/layout_solver_types.rs:109-112`) to every vector and group produced from text in `text_run_to_single_vector` (`crates/designos-app/src/vecops.rs:213, 228`), `text_runs_to_group` (`crates/designos-app/src/vecops.rs:236, 247, 270`), and the mixed-flatten text branch in `flatten_ids` (`crates/designos-app/src/vecops.rs:369-370`), so `LayoutChild::sizing_for` (`crates/designos-doc/src/layout_solver_types.rs:117-149`) returns `Sizing::Fixed` on both axes regardless of parent Auto-Layout direction.
  - Called `crate::layout::fit_group(&mut self.doc, &mut tx.ed, ...)` (`crates/designos-app/src/layout.rs:124-160`) and `crate::layout::relayout_up(&mut self.doc, &mut tx.ed, ...)` (`crates/designos-app/src/layout.rs:30-35`) before `self.run_tx(tx)` in `flatten_ids` (`crates/designos-app/src/vecops.rs:332-336`) and `outline_stroke_ids` (`crates/designos-app/src/vecops.rs:418-419`), so the initial commit and Undo -> Redo produce the exact same Auto-Layout state (`crates/designos-app/src/vecops.rs:1425, 1458`).
- **B2 — Centered stroke alignment on converted text vectors (`crates/designos-app/src/vecops.rs:219, 258, 358`)**:
  - CPU text rendering strokes glyph paths with `self.make_stroke(n, n.stroke.weight)` without reading `stroke.align` (`crates/designos-render/src/lib.rs:795-801`), and GPU text rendering fills a prebuilt stroke mesh (`crates/designos-gpu/src/compile.rs:1102-1108`), whereas `NodeKind::Vector` masks `StrokeAlign::Inside` and `StrokeAlign::Outside` (`crates/designos-render/src/lib.rs:894-914`).
  - Set `stroke.align = StrokeAlign::Center` on every vector produced from text (`template.stroke.align = StrokeAlign::Center` at `crates/designos-app/src/vecops.rs:219`, `child_template.stroke.align = StrokeAlign::Center` at `crates/designos-app/src/vecops.rs:258`, and `base.stroke.align = StrokeAlign::Center` at `crates/designos-app/src/vecops.rs:358`). On the default-Inside test (`r2-02-green-vecops.log:36-37`) and across `stroke_inside`, `stroke_outside`, and `stroke_center` (`r2-04-visual-and-kit.log:10-15`), Flatten renders with `max=15, mean_ink=0.001390, hard_px=0, non_edge_max=0, ink_px=44594` and Outline stroke renders with `max=43, mean_ink=0.002209, hard_px=0, non_edge_max=0, ink_px=44594`.
- **M1 — Non-blocking font loading for Flatten / Outline stroke (`crates/designos-app/src/fonts.rs:30, 48, 162-177`, `crates/designos-app/src/vecops.rs:87-98, 161-209`, `crates/designos-app/src/app.rs:388-397`)**:
  - Removed the synchronous `FontLoader::ensure_loaded` method that called `designos_fonts::ensure_font_requests_with` on the UI thread.
  - Added `FontLoader::check_requests(&self, requests: &BTreeSet<FontRequest>) -> (Vec<FontRequest>, Vec<String>)` (`crates/designos-app/src/fonts.rs:162-177`), which checks `self.available(requests)` (`crates/designos-app/src/fonts.rs:139-151, 163`) and `self.failed` (`crates/designos-app/src/fonts.rs:170-171`) and returns `(missing, failed)` without network or disk I/O on the UI thread.
  - Added `pending_vecop` on `FontLoader` (`crates/designos-app/src/fonts.rs:30, 48`) and `App::ensure_text_fonts` / `App::finish_pending_vecop` (`crates/designos-app/src/vecops.rs:161-209`). When `failed` is non-empty on invocation, `ensure_text_fonts` immediately calls `self.show_toast(format!("Font unavailable: {first}{suffix}"), true)` and returns `false` (`crates/designos-app/src/vecops.rs:166-171`). When `missing` faces are needed, `ensure_text_fonts` queues the operation in `self.fonts.pending_vecop`, requests the missing faces via `self.fonts.request_new_faces(&self.doc, missing)`, and returns `false` (`crates/designos-app/src/vecops.rs:175-179`).
  - In `App::poll_fonts` (`crates/designos-app/src/app.rs:381-398`), if a background font load fails, `poll_fonts` displays `"Font unavailable: {first}{suffix}"` (`crates/designos-app/src/app.rs:388-392`), and once `!self.fonts.is_loading() && !self.text_refit_pending()`, `poll_fonts` calls `self.finish_pending_vecop()` (`crates/designos-app/src/app.rs:395-397`).
- **M2 — Scope `collect_text_font_requests` by operation (`crates/designos-app/src/vecops.rs:100-122, 295, 391`)**:
  - `collect_text_font_requests(doc: &Document, ids: &[NodeId], walk_subtree: bool)` (`crates/designos-app/src/vecops.rs:100-122`) walks descendant subtrees via `doc.walk(*id, ...)` only when `walk_subtree` is `true` (`crates/designos-app/src/vecops.rs:115-116`, called by `flatten_selection` at `crates/designos-app/src/vecops.rs:295`) and inspects only `doc.get(*id)` when `walk_subtree` is `false` (`crates/designos-app/src/vecops.rs:117-119`, called by `outline_stroke_selection` at `crates/designos-app/src/vecops.rs:391`).
- **M3 — Preserve fill variable bindings and style links after gradient handle remapping (`crates/designos-app/src/vecops.rs:64-84, 229-230, 250, 271`)**:
  - `carry_paint_bindings` (`crates/designos-app/src/vecops.rs:64-84`) checks `let fill_matches = resolved_base_fills.is_empty() || dst.fills == resolved_base_fills;` (`crates/designos-app/src/vecops.rs:66`). Passing `let base_fills: &[Paint] = &[];` and `run.uses_base_fill` in `text_run_to_single_vector` (`crates/designos-app/src/vecops.rs:229-230`) and `text_runs_to_group` (`crates/designos-app/src/vecops.rs:250, 271`) preserves `"fill"` style links and `"fills/..."` variable bindings after gradient handle remapping without leaking bindings to recoloured runs (`crates/designos-app/src/vecops.rs:1566-1567, 1576-1577, 1617-1620`).
- **m1 — Preserve `visible` and `locked` flags (`crates/designos-app/src/vecops.rs:226-227, 241-242, 366-367`)**:
  - Copied `visible` and `locked` onto the single vector in `text_run_to_single_vector` (`crates/designos-app/src/vecops.rs:226-227`), the parent group in `text_runs_to_group` (`crates/designos-app/src/vecops.rs:241-242`), and the mixed-flatten vector in `flatten_ids` (`crates/designos-app/src/vecops.rs:366-367`).
- **m2 — Fit Outline stroke and multi-run Flatten group to children's union at commit (`crates/designos-app/src/vecops.rs:332-334, 418`)**:
  - `flatten_ids` (`crates/designos-app/src/vecops.rs:332-334`) and `outline_stroke_ids` (`crates/designos-app/src/vecops.rs:418`) call `crate::layout::fit_group(&mut self.doc, &mut tx.ed, ...)` (`crates/designos-app/src/layout.rs:124-160`) before `self.run_tx(tx)` so the group's bounding box tightly wraps its glyph children's union at commit (`crates/designos-app/src/vecops.rs:1442-1453`).
- **m3 — Strengthened pixel & unit tests + mutation matrix (`crates/designos-app/src/vecops.rs:1297-1351, 1623-1777`)**:
  - `assert_text_scenario_pixels` (`crates/designos-app/src/vecops.rs:1623-1669`) asserts that original text `NodeId`s are removed from `app.doc` (`crates/designos-app/src/vecops.rs:1638, 1656`) and replaced by `NodeKind::Vector` / `NodeKind::Group` nodes (`crates/designos-app/src/vecops.rs:1640, 1660`), computes `mean_ink` over non-white pixels where `!is_white(b_data) || !is_white(a_data)` with `is_white = |d: &[u8]| d[idx] == 255 && d[idx + 1] == 255 && d[idx + 2] == 255 && d[idx + 3] == 255` (`crates/designos-app/src/vecops.rs:1311, 1321-1324, 1349`), asserts `mean_ink < 1.0`, `hard_px == 0`, and `non_edge_max == 0` (`crates/designos-app/src/vecops.rs:1646-1648, 1666-1668`), and tests 5 scenarios in `pixel_test_converted_text_vectors_match_original_text_render` (`crates/designos-app/src/vecops.rs:1672-1777`):
    1. `decor_and_runs` (`crates/designos-app/src/vecops.rs:1674-1694`; `r2-02-green-vecops.log:34-35`: `max=59, mean_ink=0.478733, hard_px=0, non_edge_max=0, ink_px=9134`).
    2. `stroked_default_inside` (`crates/designos-app/src/vecops.rs:1697-1711`; `r2-02-green-vecops.log:36-37`: Flatten `max=15, mean_ink=0.001390, hard_px=0, non_edge_max=0, ink_px=44594`, Outline `max=43, mean_ink=0.002209, hard_px=0, non_edge_max=0, ink_px=44594`).
    3. `linear_gradient` (`crates/designos-app/src/vecops.rs:1714-1728`; `r2-02-green-vecops.log:38-39`: Flatten `max=56, mean_ink=0.293098, hard_px=0, non_edge_max=0, ink_px=25117`, Outline `max=56, mean_ink=0.296293, hard_px=0, non_edge_max=0, ink_px=25117`).
    4. `lists` (`crates/designos-app/src/vecops.rs:1731-1751`; `r2-02-green-vecops.log:40-41`: `max=59, mean_ink=0.653704, hard_px=0, non_edge_max=0, ink_px=6628`).
    5. `opacity_shadow` with `n.opacity = 0.5` (`crates/designos-app/src/vecops.rs:1754-1776`, line `1764`; `r2-02-green-vecops.log:42-43`: `max=28, mean_ink=0.099395, hard_px=0, non_edge_max=0, ink_px=42233`).
- **L1 — Clean test string & non-empty run lookup (`crates/designos-app/src/vecops.rs:357, 1685`)**:
  - Replaced `"1469 Vector Text"` with `"Vector Text"` (`crates/designos-app/src/vecops.rs:1685`) and changed mixed-selection text run lookup from `tp.runs.first()` to `tp.runs.iter().find(|r| r.start == 0 && r.end > 0)` (`crates/designos-app/src/vecops.rs:357`).
- **M4 — Re-anchored report from git history & actual log files**:
  - Rewrote Round 1 §1–§3 above directly from `git show 93c8e407~2:...`, `git show 93c8e407:...`, and `01-fmt.log` .. `06-test-vecops-nocapture.log`.

---

### 3. Round 2 Acceptance Criteria & Verification Table

| # | Criterion | Command | Result / Output |
|---|---|---|---|
| 1a | RED on `93c8e407`: new/updated Round 2 tests fail on Round 1 production code | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test -p designos-app --bin designos --locked vecops::tests::` (with Round 2 tests on `93c8e407` code) | `20 passed; 6 failed; 0 ignored; 0 measured; 4022 filtered out; finished in 0.61s` (`r2-01-red-on-93c8e407.log:73`) |
| 1b | GREEN on Round 2 `HEAD`: all `vecops::tests` pass on Round 2 production code | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test -p designos-app --bin designos --locked vecops::tests:: -- --nocapture` | `26 passed; 0 failed; 0 ignored; 0 measured; 4022 filtered out; finished in 1.48s` (`r2-02-green-vecops.log:47`) |
| 1c | Mutation check: `N0-null`, `N1`..`N13` (13 review mutants), and `R2-B1`, `R2-B2`, `R2-M2`, `R2-M3`, `R2-m1`, `R2-m2` (6 Round 2 mutants) | One-mutant-at-a-time test harness (`r2-03-mutants.log:1-20`) | `N0-null`: `SURVIVED` (`r2-03-mutants.log:1`); `N1`..`N13`: all `13 KILLED` (`r2-03-mutants.log:2-14`); `R2-B1`..`R2-m2`: all `6 KILLED` (`r2-03-mutants.log:15-20`) |
| 2a | Visual pixel sheet (`plans/reports/agy-1469-text-to-vector-evidence/r2-pixel-sheet.png`) | Render before / after Flatten / after Outline stroke / diff across `stroke_inside`, `stroke_outside`, `stroke_center`, Auto-Layout `Fill` + sibling edit, and `gradient` (`r2-04-visual-and-kit.log:10-18`) | `hard_px=0, non_edge_max=0` on every row (`r2-04-visual-and-kit.log:10-15, 17-18`); Auto-Layout `Fill` vector stays `(10.00, 10.00, Vec2 { x: 242.9414, y: 23.28125 }, Fixed)` before and after sibling edit (`r2-04-visual-and-kit.log:16`) |
| 2b | Untitled UI kit 36px heading (`r2-04-visual-and-kit.log:19-21`) | Convert 36px heading `NodeId(5699) "Layout"` in frame `NodeId(5509) "Layout"` on an APFS copy of Untitled UI PRO VARIABLES v8.0 `.fig` (`r2-04-visual-and-kit.log:19-21`) | Flatten (`NodeId(225113)`, `Discriminant(9)`) & Outline (`NodeId(225113)`, `Discriminant(2)`, `kids=6`): `size=Vec2 { x: 110.72813, y: 33.671875 }, max=29, mean_all=0.000178, mean_ink=0.003809, hard_px=0, non_edge_max=0, ink_px=45882` (`r2-04-visual-and-kit.log:20-21`) |
| 3a | Gate 1: Rustfmt | `cargo fmt --all -- --check` | Exit `0`, 0 diffs (`r2-05-fmt.log`) |
| 3b | Gate 2: Clippy | `CARGO_TARGET_DIR="$PWD/out/cargo" cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::needless_return` (`r2-06-clippy.log:53-71`) | Exit `0`, 0 warnings in `38.81s` (`r2-06-clippy.log:70-71`; raw `-D warnings` exits `101` at `r2-06-clippy.log:51` on `crates/designos-app/src/agent_permissions.rs:1176:9, 1194:9` at `r2-06-clippy.log:19, 34`) |
| 3c | Gate 3: App `vecops` tests | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test -p designos-app --bin designos --locked vecops` | `26 passed; 0 failed; 0 ignored; 0 measured; 4022 filtered out; finished in 1.51s` (`r2-07-test-vecops.log:37`) |
| 3d | Gate 4: `designos-render` tests | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test -p designos-render --locked` | `56 passed; 0 failed; 2 ignored` in `src/lib.rs` (`r2-08-test-render.log:68`) + `23 passed; 0 failed` in `tests/vectorops.rs` (`r2-08-test-render.log:452`) + all other `designos-render` test binaries `0 failed` (`r2-08-test-render.log:78-458`) |
| 3e | Gate 5: Workspace `--lib` tests | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test --workspace --locked --lib` | `13 + 708 + 1 + 153 + 143 + 30 + 4 + 37 + 322 + 56 + 1 + 8 = 1,476 passed; 0 failed; 3 ignored` across all 12 library crates (`r2-09-test-workspace-lib.log:101, 815, 822, 982, 1131, 1167, 1177, 1220, 1548, 1612, 1619, 1633`) |

### Pasted Round 2 Evidence Logs (`r2-01` .. `r2-09`)

```text
=== r2-01-red-on-93c8e407.log:65-73 (RED on 93c8e407 production code) ===
failures:
    vecops::tests::flatten_of_two_paint_text_gives_two_vectors_in_a_group
    vecops::tests::outline_stroke_of_two_run_text_gives_one_vector_per_glyph_with_each_runs_fill
    vecops::tests::pixel_test_converted_text_vectors_match_original_text_render
    vecops::tests::text_in_auto_layout_keeps_fixed_size_after_relayout_and_redo_matches_commit
    vecops::tests::text_to_vector_preserves_gradient_style_links_and_merges_shared_paint_runs_without_leaking_bindings
    vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope

test result: FAILED. 20 passed; 6 failed; 0 ignored; 0 measured; 4022 filtered out; finished in 0.61s

=== r2-02-green-vecops.log:9-47 & r2-07-test-vecops.log:9-37 (GREEN on Round 2 HEAD) ===
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

=== r2-03-mutants.log:1-20 (13 review mutants N1..N13 + 6 Round 2 mutants R2-*) ===
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

=== r2-04-visual-and-kit.log:10-21 (Visual pixel sheet & Untitled UI 36px heading) ===
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

=== r2-06-clippy.log:53-71 ===
$ cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::needless_return
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 38.81s
WORKSPACE_CLIPPY_NO_NEEDLESS_RETURN_EXIT=0

=== r2-08-test-render.log:68, 452: cargo test -p designos-render --locked ===
Running unittests src/lib.rs: test result: ok. 56 passed; 0 failed; 2 ignored
Running tests/vectorops.rs: test result: ok. 23 passed; 0 failed; 0 ignored
(all other render test binaries in r2-08-test-render.log:78-458: 0 failed)

=== r2-09-test-workspace-lib.log: cargo test --workspace --locked --lib ===
designos-diff (line 101):        13 passed; 0 failed; 0 ignored
designos-doc (line 815):        708 passed; 0 failed; 0 ignored
designos-eval (line 822):         1 passed; 0 failed; 0 ignored
designos-export (line 982):     153 passed; 0 failed; 1 ignored
designos-fig (line 1131):       143 passed; 0 failed; 0 ignored
designos-fonts (line 1167):      30 passed; 0 failed; 0 ignored
designos-gpu (line 1177):         4 passed; 0 failed; 0 ignored
designos-library (line 1220):    37 passed; 0 failed; 0 ignored
designos-mcp (line 1548):       322 passed; 0 failed; 0 ignored
designos-render (line 1612):     56 passed; 0 failed; 2 ignored
designos-svg (line 1619):         1 passed; 0 failed; 0 ignored
designos-webcapture (line 1633):  8 passed; 0 failed; 0 ignored
```

### 4. Epistemic Ledger, Weakest Link & Skills Loaded (Round 2)

#### Epistemic Ledger
- **`FACT`**: In `crates/designos-doc/src/layout_solver_types.rs:117-167` (`LayoutChild::sizing_for` and `LayoutChild::set_axis_sizing`), `LayoutChild` stores `horizontal`, `vertical`, and `stretched` axis overrides. Calling `set_axis_sizing(true, Sizing::Fixed, counter)` and `set_axis_sizing(false, Sizing::Fixed, counter)` alongside `align_self = AlignSelf::Auto` and `grow = 0.0` (`crates/designos-app/src/vecops.rs:153-158`) guarantees `Sizing::Fixed` on both axes regardless of parent Auto-Layout direction.
- **`FACT`**: Both `designos-render` (`crates/designos-render/src/lib.rs:795-801`) and `designos-gpu` (`crates/designos-gpu/src/compile.rs:1102-1108`) stroke glyph paths with centered strokes; forcing `stroke.align = StrokeAlign::Center` on converted text vectors (`crates/designos-app/src/vecops.rs:219, 258, 358`) eliminates the `StrokeAlign::Inside` / `StrokeAlign::Outside` inset/outset discrepancy (`non_edge_max = 0`, `mean_ink = 0.001390` on Flatten and `0.002209` on Outline stroke at `r2-02-green-vecops.log:36-37` and `r2-04-visual-and-kit.log:10-15`).
- **`FACT`**: `FontLoader::check_requests` (`crates/designos-app/src/fonts.rs:162-177`) + `pending_vecop` in `App::poll_fonts` (`crates/designos-app/src/app.rs:395-397`) runs `finish_pending_vecop()` only after `!self.fonts.is_loading() && !self.text_refit_pending()`, so Auto-Width/Auto-Height text nodes are refitted with the newly loaded font before glyph outlining runs, without blocking the UI thread.
- **`INFERENCE`**: Keying queued entries by `self.doc.runtime_id()` (`crates/designos-app/src/vecops.rs:162-163, 184-185`) prevents a deferred Flatten or Outline stroke from firing on a different document if the user switches or closes documents while fonts are downloading.
- **`ASSUMPTION` (falsified in Round 3)**: Round 2 assumed the user would not change `self.selection` while a font download was in flight; Round 3 replaces selection re-reading with stored `ids: Vec<NodeId>` in `QueuedVecOp` (`crates/designos-app/src/vecops.rs:93-98, 204-207`).

#### Weakest Link (One Line)
- In Round 2 (`bb0c2cea`), `finish_pending_vecop` re-invoked `flatten_selection()` / `outline_stroke_selection()` on `self.selection` when the background font load finished instead of converting the node IDs the command was invoked on.

#### Skills Loaded & What Changed Because of Them
- **`es:astra-work`**: Enforced strict RED-before-GREEN verification on `93c8e407` (`r2-01-red-on-93c8e407.log:73`), 19-mutant runtime kill verification (`r2-03-mutants.log:1-20`), synthetic and Untitled UI 36px heading renders (`r2-04-visual-and-kit.log:10-21`), and exact log citations.
- **`es:fable-thinking`**: Guided root-cause verification of the 5 Round 1 defects (`LayoutChild::sizing_for` dual-axis sizing, text centered stroke parity, non-blocking `FontLoader` queue + refit ordering, subtree-scoped font collection, and gradient `uses_base_fill` preservation).
- **`es:rust`**: Kept `FontLoader::check_requests` (`crates/designos-app/src/fonts.rs:162-177`) zero-I/O on the UI thread and preserved single-step `Command::Batch` undo/redo invariants across `fit_group` and `relayout_up` (`crates/designos-app/src/vecops.rs:332-336, 418-419`).
- **`es:rust-gpu-canvas`**: Verified that `designos-gpu` (`crates/designos-gpu/src/compile.rs:1102-1108`) and `designos-render` (`crates/designos-render/src/lib.rs:795-801`) share centered text stroke semantics, ensuring `StrokeAlign::Center` on converted text vectors preserves CPU/GPU parity.

---

## Round 3

### 1. What Could Not Be Verified First (Round 3)
- **Live Figma comparison (`TO BE CONFIRMED AGAINST FIGMA`)**: Confirming whether live Figma queues deferred Flatten / Outline stroke operations or blocks synchronously when a font is still downloading remains `TO BE CONFIRMED AGAINST FIGMA` because live Figma is outside the worker boundary.
- **Interactive multi-second network stall in live desktop GUI**: Verified deterministically in headless `designos-app` tests using `crate::fonts::testkit::serving_with_delay(&[...], Duration::from_millis(250))` (`crates/designos-app/src/vecops.rs:1791, 1824, 1850, 1905, 1936`) rather than a live Fontsource HTTP stall.

### 2. Round 3 Rulings & Code Changes
- **Ruling 1 — Store invoked node IDs in `QueuedVecOp` and share `flatten_ids` / `outline_stroke_ids` (`crates/designos-app/src/vecops.rs:92-98, 161-209, 282-471`)**:
  - Defined `QueuedVecOp { pub doc_id: u64, pub op: PendingVecOp, pub ids: Vec<NodeId>, pub requests: BTreeSet<designos_fonts::FontRequest> }` (`crates/designos-app/src/vecops.rs:93-98`).
  - Refactored `flatten_selection` (`crates/designos-app/src/vecops.rs:282-299`) + `flatten_ids(&mut self, raw_ids: &[NodeId], deferred: bool)` (`crates/designos-app/src/vecops.rs:301-383`) and `outline_stroke_selection` (`crates/designos-app/src/vecops.rs:386-395`) + `outline_stroke_ids(&mut self, raw_ids: &[NodeId], deferred: bool)` (`crates/designos-app/src/vecops.rs:397-471`) so the selection-based entry points and the deferred `finish_pending_vecop` path share one implementation per command that takes `raw_ids: &[NodeId]`.
  - Neither `flatten_ids` nor `outline_stroke_ids` reads `self.selection` to decide what to convert (`crates/designos-app/src/vecops.rs:302-307, 400-401`); both filter `raw_ids` to nodes that still exist in `self.doc` and remain convertible (`can_flatten(self, *i) && !linked(self, *i)` on a shared parent at `crates/designos-app/src/vecops.rs:302-307`; `self.doc.get(id)` and `!linked(self, id)` at `crates/designos-app/src/vecops.rs:401-405`), return without editing if none of the stored IDs remain (`crates/designos-app/src/vecops.rs:308-310, 462-468`), and overwrite `self.selection` only with the newly produced node IDs (`self.selection = vec![out_id]` at `crates/designos-app/src/vecops.rs:337`, `self.selection = vec![id]` at `crates/designos-app/src/vecops.rs:382`, `self.selection = made` at `crates/designos-app/src/vecops.rs:470`).
  - If a vector pen edit (`self.canvas.vedit.is_some()`) is open at the time a deferred operation's fonts arrive in `finish_pending_vecop`, the deferred operation does not exit `self.canvas.vedit` and does not run; it drops the queued entry and shows `"Flatten cancelled"` or `"Outline stroke cancelled"` (`crates/designos-app/src/vecops.rs:196-203`).
- **Ruling 2 — FIFO `VecDeque<QueuedVecOp>` queue for multiple deferred commands (`crates/designos-app/src/fonts.rs:30, 48`, `crates/designos-app/src/vecops.rs:161-209`)**:
  - Changed `FontLoader::pending_vecop` to `VecDeque<crate::vecops::QueuedVecOp>` (`crates/designos-app/src/fonts.rs:30, 48`).
  - In `App::ensure_text_fonts` (`crates/designos-app/src/vecops.rs:161-180`), stale entries from another document are pruned (`self.fonts.pending_vecop.retain(|q| q.doc_id == doc_id)` at `crates/designos-app/src/vecops.rs:163`). If any requested face is already in `self.fonts.failed`, `ensure_text_fonts` toasts `"Font unavailable: {first}{suffix}"` and returns `false` immediately (`crates/designos-app/src/vecops.rs:166-171`), leaving `self.fonts.pending_vecop` untouched. If `missing.is_empty() && self.fonts.pending_vecop.is_empty()`, it returns `true` to run immediately (`crates/designos-app/src/vecops.rs:172-174`); otherwise it pushes `QueuedVecOp` onto `self.fonts.pending_vecop` and requests `missing` faces (`crates/designos-app/src/vecops.rs:175-179`).
  - In `App::finish_pending_vecop` (`crates/designos-app/src/vecops.rs:182-209`), entries are popped from the front in FIFO order (`while let Some(queued) = self.fonts.pending_vecop.pop_front()` at `crates/designos-app/src/vecops.rs:183`). If an earlier load was superseded and `missing` is non-empty, `finish_pending_vecop` pushes `queued` back to the front, calls `self.fonts.request_new_faces(&self.doc, missing)`, and breaks until that load finishes (`crates/designos-app/src/vecops.rs:191-195`).
- **Ruling 3 — Five Round 3 tests (RED on `bb0c2cea`, GREEN on Round 3 `HEAD`) + selection-fallback mutant kill (`crates/designos-app/src/vecops.rs:1779-1953`)**:
  - `deferred_flatten_converts_invoked_text_not_later_selected_rectangle` (`crates/designos-app/src/vecops.rs:1780-1806`): invokes `flatten_selection` on `tid` (`SlowvecFnt1`), switches `app.selection = vec![rid]` to a rectangle while loading, settles fonts, and verifies `app.doc.get(rid) == Some(&rect)`, `app.doc.get(tid).is_none()`, and `app.selection` is the produced `NodeKind::Vector` (`crates/designos-app/src/vecops.rs:1801-1805`).
  - `deferred_outline_stroke_converts_invoked_text_not_later_selected_rectangle` (`crates/designos-app/src/vecops.rs:1809-1839`): invokes `outline_stroke_selection` on `tid` (`SlowvecFnt1`), switches `app.selection = vec![rid]` to a stroked rectangle while loading, settles fonts, and verifies `app.doc.get(rid) == Some(&rect)`, `app.doc.get(tid).is_none()`, and `app.selection` is the produced `NodeKind::Group` (`crates/designos-app/src/vecops.rs:1834-1838`).
  - `two_deferred_text_commands_both_convert_in_order` (`crates/designos-app/src/vecops.rs:1842-1891`): queues `flatten_selection` on `t1` (`SlowvecFnt1`) and `outline_stroke_selection` on `t2` (`SlowvecFnt2`), invokes `flatten_selection` on `t_fail` (`MissingFont`, already failed) and verifies it toasts immediately while leaving the queue intact (`crates/designos-app/src/vecops.rs:1873-1878`), settles fonts, and verifies both `t1` and `t2` convert in order as 2 undo steps (`app.hist.undo_depth() == 2` at `crates/designos-app/src/vecops.rs:1883`) where undoing once restores `t2` while keeping `t1` converted (`crates/designos-app/src/vecops.rs:1888-1890`).
  - `deferred_command_on_deleted_text_edits_nothing_without_panic` (`crates/designos-app/src/vecops.rs:1894-1922`): queues `flatten_selection` on `tid`, deletes `tid` and selects `rid` before the font arrives, settles fonts, and verifies `app.hist.undo_depth() == undo_after_delete`, `app.doc.get(rid) == Some(&rect)`, and `app.selection == vec![rid]` (`crates/designos-app/src/vecops.rs:1919-1921`).
  - `deferred_command_cancelled_when_pen_edit_is_open_at_arrival` (`crates/designos-app/src/vecops.rs:1925-1953`): queues `flatten_selection` on `tid`, opens pen edit on `rid` via `crate::canvas::vector::enter(&mut app, rid)` (`crates/designos-app/src/vecops.rs:1943`), settles fonts, and verifies `app.canvas.vedit.as_ref().map(|v| v.id) == Some(rid)`, `app.doc.get(tid).is_some()`, `app.doc.get(rid) == Some(&rect)`, `app.hist.undo_depth() == 0`, and the cancellation toast (`crates/designos-app/src/vecops.rs:1948-1952`).
- **Ruling 4 & Ruling 5 — Report citation audit & test comment cleanup (`crates/designos-app/src/vecops.rs:1427-1953`)**:
  - Removed all review ID tags (`m2`, `M2`, `M3`, `N1`, `N2`, `L1`, `N5`, `N8`, `N12`, `B2`, `N4`, `N6`, `N10`, `N13`) from test comments in `crates/designos-app/src/vecops.rs` (`crates/designos-app/src/vecops.rs:1427, 1500, 1545, 1580, 1588, 1600, 1673, 1696, 1713, 1730, 1753`).

---

### 3. Round 3 Acceptance Criteria & Verification Table

| # | Criterion | Command | Result / Output |
|---|---|---|---|
| 1a | RED on `bb0c2cea`: 5 new Round 3 tests fail on Round 2 production code (`bb0c2cea`) | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test -p designos-app --bin designos --locked vecops` (with Round 3 tests on `bb0c2cea` production code) | `26 passed; 5 failed; 0 ignored; 0 measured; 4038 filtered out; finished in 1.48s` (`r3-01-red-on-bb0c2cea.log:80-86`) |
| 1b | GREEN on Round 3 `HEAD`: all 31 `vecops` tests pass | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test -p designos-app --bin designos --locked vecops` | `31 passed; 0 failed; 0 ignored; 0 measured; 4038 filtered out; finished in 1.46s` (`r3-02-green-vecops.log:42`) |
| 1c | Mutant check: `R3-N0-null` (unmutated control) vs `R3-M1-selection-fallback` (`finish_pending_vecop` passes `&self.selection.clone()` instead of `&queued.ids`) | Full Cargo test transcript in `r3-03-mutants.log:1-145` | `R3-N0-null`: `31 passed; 0 failed` in `1.50s`, `exit=0 (SURVIVED)` (`r3-03-mutants.log:45, 47`); `R3-M1-selection-fallback`: `27 passed; 4 failed` in `1.48s`, `exit=101 (KILLED)` (`r3-03-mutants.log:137-142, 145`) |
| 2a | Visual RED-to-GREEN terminal render (`plans/reports/agy-1469-text-to-vector-evidence/r3-red-to-green.png`) | Rendered `r3-01-red-on-bb0c2cea.log:35-86` and `r3-02-green-vecops.log:9-42` to `r3-red-to-green.png` and inspected | Shows 5 `FAILED` tests on `bb0c2cea` (`r3-01-red-on-bb0c2cea.log:80-86`) and `31 passed; 0 failed` on Round 3 `HEAD` (`r3-02-green-vecops.log:42`) |
| 3a | Gate 1: Rustfmt | `cargo fmt --all -- --check` | Exit `0`, 0 bytes (`r3-04-fmt.log`) |
| 3b | Gate 2: Clippy | `CARGO_TARGET_DIR="$PWD/out/cargo" cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::needless_return` (`r3-05-clippy.log:55-75`) | `CLEAN_CLIPPY_EXIT=0` in `24.40s` (`r3-05-clippy.log:74-75`; raw `-D warnings` exits `101` at `r3-05-clippy.log:53` on `crates/designos-app/src/agent_permissions.rs:1176:9, 1194:9` at `r3-05-clippy.log:20, 35`) |
| 3c | Gate 3: Workspace `--lib` tests | `CARGO_TARGET_DIR="$PWD/out/cargo" TMPDIR=/tmp cargo test --workspace --locked --lib` | `13 + 714 + 1 + 158 + 143 + 30 + 4 + 37 + 322 + 56 + 1 + 8 = 1,487 passed; 0 failed; 3 ignored` across all 12 library crates (`r3-06-test-workspace-lib.log:101, 821, 828, 993, 1142, 1178, 1188, 1231, 1559, 1623, 1630, 1644`) |

### Pasted Round 3 Evidence Logs (`r3-01` .. `r3-06`)

```text
=== r3-01-red-on-bb0c2cea.log:79-86 (RED on bb0c2cea production code) ===
failures:
    vecops::tests::deferred_command_cancelled_when_pen_edit_is_open_at_arrival
    vecops::tests::deferred_command_on_deleted_text_edits_nothing_without_panic
    vecops::tests::deferred_flatten_converts_invoked_text_not_later_selected_rectangle
    vecops::tests::deferred_outline_stroke_converts_invoked_text_not_later_selected_rectangle
    vecops::tests::two_deferred_text_commands_both_convert_in_order

test result: FAILED. 26 passed; 5 failed; 0 ignored; 0 measured; 4038 filtered out; finished in 1.48s

=== r3-02-green-vecops.log:35-42 (GREEN on Round 3 HEAD) ===
test vecops::tests::deferred_command_on_deleted_text_edits_nothing_without_panic ... ok
test vecops::tests::deferred_outline_stroke_converts_invoked_text_not_later_selected_rectangle ... ok
test vecops::tests::deferred_command_cancelled_when_pen_edit_is_open_at_arrival ... ok
test vecops::tests::deferred_flatten_converts_invoked_text_not_later_selected_rectangle ... ok
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok
test vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 4038 filtered out; finished in 1.46s

=== r3-03-mutants.log:1-47, 49-145 (Null control + selection-fallback mutant) ===
=== R3-N0-null: null control (unmutated Round 3 HEAD) ===
running 31 tests
...
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 4038 filtered out; finished in 1.50s
R3-N0-null exit=0 (SURVIVED)

=== R3-M1-selection-fallback: mutant where finish_pending_vecop falls back to self.selection ===
running 31 tests
...
failures:
    vecops::tests::deferred_command_on_deleted_text_edits_nothing_without_panic
    vecops::tests::deferred_flatten_converts_invoked_text_not_later_selected_rectangle
    vecops::tests::deferred_outline_stroke_converts_invoked_text_not_later_selected_rectangle
    vecops::tests::two_deferred_text_commands_both_convert_in_order

test result: FAILED. 27 passed; 4 failed; 0 ignored; 0 measured; 4038 filtered out; finished in 1.48s
R3-M1-selection-fallback exit=101 (KILLED)

=== r3-04-fmt.log ===
(exit 0, 0 diffs)

=== r3-05-clippy.log:55-75 ===
=== cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::needless_return ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.40s
CLEAN_CLIPPY_EXIT=0

=== r3-06-test-workspace-lib.log: cargo test --workspace --locked --lib ===
designos-diff (line 101):        13 passed; 0 failed; 0 ignored
designos-doc (line 821):        714 passed; 0 failed; 0 ignored
designos-eval (line 828):         1 passed; 0 failed; 0 ignored
designos-export (line 993):     158 passed; 0 failed; 1 ignored
designos-fig (line 1142):       143 passed; 0 failed; 0 ignored
designos-fonts (line 1178):      30 passed; 0 failed; 0 ignored
designos-gpu (line 1188):         4 passed; 0 failed; 0 ignored
designos-library (line 1231):    37 passed; 0 failed; 0 ignored
designos-mcp (line 1559):       322 passed; 0 failed; 0 ignored
designos-render (line 1623):     56 passed; 0 failed; 2 ignored
designos-svg (line 1630):         1 passed; 0 failed; 0 ignored
designos-webcapture (line 1644):  8 passed; 0 failed; 0 ignored
```

### 4. Epistemic Ledger, Weakest Link & Skills Loaded (Round 3)

#### Epistemic Ledger
- **`FACT`**: Storing `ids: Vec<NodeId>` inside each `QueuedVecOp` (`crates/designos-app/src/vecops.rs:93-98, 175`) and dispatching `self.flatten_ids(&queued.ids, true)` / `self.outline_stroke_ids(&queued.ids, true)` (`crates/designos-app/src/vecops.rs:204-207`) ensures `finish_pending_vecop` never reads `self.selection` to decide which nodes to convert (`r3-03-mutants.log:137-145`).
- **`FACT`**: Checking `if self.canvas.vedit.is_some()` in `finish_pending_vecop` (`crates/designos-app/src/vecops.rs:196-203`) before dispatching `flatten_ids` / `outline_stroke_ids` preserves an active vector pen edit session and shows `"Flatten cancelled"` / `"Outline stroke cancelled"` (`crates/designos-app/src/vecops.rs:1948-1952`).
- **`FACT`**: `FontLoader::start` (`crates/designos-app/src/fonts.rs:105`) calls `designos_fonts::requests_for(doc)` across all nodes in the document (up to 8,192 nodes), so any load started after a text node is inserted into `doc` will also fetch that text node's fonts unless that face is already in `self.failed`.
- **`INFERENCE`**: When two deferred commands are queued on the same document while fonts are loading (`crates/designos-app/src/vecops.rs:172-179`), draining `self.fonts.pending_vecop` with `pop_front()` in `finish_pending_vecop` (`crates/designos-app/src/vecops.rs:183`) executes them in FIFO invocation order and records one `Tx` undo step per command (`crates/designos-app/src/vecops.rs:1883-1890`).
- **`ASSUMPTION`**: Suppressing `"Nothing to flatten"` / `"Select a layer with a stroke to outline"` toasts when `deferred == true` (`crates/designos-app/src/vecops.rs:318-320, 342-344, 463-465`) avoids noisy stale toasts if the user deleted or modified the target layer while the font was downloading.

#### Weakest Link (One Line)
- If a queued Flatten's target nodes were reparented into different containers before the font arrived, `flatten_ids` (`crates/designos-app/src/vecops.rs:303-305`) retains only the subset still sharing `ids[0]`'s parent rather than cancelling the whole multi-node Flatten.

#### Skills Loaded & What Changed Because of Them
- **`es:astra-work`**: Enforced reproducing the 5 failures on `bb0c2cea` (`r3-01-red-on-bb0c2cea.log:80-86`) before editing production code, running the full Cargo transcript for `R3-N0-null` and `R3-M1-selection-fallback` (`r3-03-mutants.log:1-145`), and auditing every `(file:line)` and `(log:line)` citation across Round 2 and Round 3.
- **`es:fable-thinking`**: Falsified Round 2's selection-stability assumption (`crates/designos-app/src/vecops.rs:1780-1953`) and identified that `FontLoader::start` (`crates/designos-app/src/fonts.rs:105`) scans the whole document via `designos_fonts::requests_for(doc)`, shaping how `two_deferred_text_commands_both_convert_in_order` (`crates/designos-app/src/vecops.rs:1852-1865`) stages its `MissingFont` failure before inserting `SlowvecFnt1` and `SlowvecFnt2`.
- **`es:rust`**: Used `VecDeque<QueuedVecOp>` (`crates/designos-app/src/fonts.rs:30, 48`) with shared slice-taking helpers `flatten_ids(&mut self, raw_ids: &[NodeId], deferred: bool)` and `outline_stroke_ids(&mut self, raw_ids: &[NodeId], deferred: bool)` (`crates/designos-app/src/vecops.rs:301, 397`) to avoid duplicate conversion logic.
- **`es:rust-gpu-canvas`**: Preserved the `StrokeAlign::Center` and `set_fixed_layout_child` invariants inside `flatten_ids` (`crates/designos-app/src/vecops.rs:358, 370`) and `outline_stroke_ids` (`crates/designos-app/src/vecops.rs:413-419`).

### 5. Git Log (`origin/main..HEAD`)

```text
bb0c2cea docs(report): record Round 2 commit hash for #1469
33d7d0e6 fix(vector): preserve auto-layout sizing, centered text strokes, and async font loading (#1469)
14188742 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1469-text-to-vector
93c8e407 docs(report): record final commit hash for #1469
af6c702f feat(vector): Flatten and Outline stroke convert text to vector paths (#1469)
```

Status: DONE
Summary: Stored the invoked node IDs in a FIFO `VecDeque<QueuedVecOp>` queue (`crates/designos-app/src/fonts.rs:30`, `crates/designos-app/src/vecops.rs:93-98, 161-209`) and refactored `flatten_selection`/`flatten_ids` (`vecops.rs:282-383`) and `outline_stroke_selection`/`outline_stroke_ids` (`vecops.rs:386-471`) so deferred conversions convert only the invoked IDs that still exist, cancel without exiting an open pen edit (`vecops.rs:196-203`), pass all 5 new Round 3 tests (`r3-01-red-on-bb0c2cea.log:80-86` -> `r3-02-green-vecops.log:42`) and kill the selection-fallback mutant (`r3-03-mutants.log:142, 145`), and re-anchored Round 2 + Round 3 report citations (`plans/reports/agy-1469-text-to-vector.md`).
Concerns/Blockers: none
