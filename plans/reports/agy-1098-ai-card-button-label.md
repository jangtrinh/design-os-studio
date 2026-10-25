# Report — #1098 (AI-08): Button Label Sits at Top-Left Instead of Centre in Agent-Built Cards

## 1. What Could NOT Be Verified
- Live LLM stochastic compliance in a fresh interactive Claude/Codex agent session (i.e., whether an external LLM always chooses to pass `preset: "easeui-button-primary"` or `layout: {"direction": "row", "justify": "center", "align": "center"}` without ever emitting a bare `"kind": "frame"` without `preset` or `layout`) could not be verified here because worker rules prohibit invoking `claude`, `codex`, `gemini`, `grok`, or `agy` binaries. All deterministic MCP engine, spec, layout solver, renderer, and rules-pack paths were verified directly via `designos-mcp`, `designos-doc`, `designos-render`, and `designos-eval`.

---

## 2. Diagnosis Numbers & Cause

### Reproduction & Exact Numbers on Base (`origin/main`)

1. **QA Login Card Button with `preset: "easeui-button-primary"` (or plain frame with `layout: {"direction": "row", "align": "center", "justify": "center"}`) and child `{"kind": "text", "name": "Button label", "text": "Sign in", "font_size": 14, "font_weight": 600}`**:
   - **Button frame rect**: `pos = (24.00, 136.00)` (or `(0.00, 0.00)` at root), `size = (312.00, 44.00)`, `button_centre = (156.00, 22.00)`.
   - **Button `auto_layout`**: `direction = Horizontal, primary_align = Center, counter_align = Center, primary_sizing = Fixed, counter_sizing = Fixed, padding = [10.0, 16.0, 10.0, 16.0]`.
   - **Text child rect (`node.transform.tx/ty`, `node.size`)**: `pos = (130.00, 12.00)`, `size = (52.00, 20.00)` (from `estimate_intrinsic_text_size`), `box_centre = (156.00, 22.00)`.
   - **Text child alignment fields**: `auto_resize = WidthAndHeight`, `style.align_h = Left`, `style.align_v = Top`, `layout_child = (Hug, Hug)`.
   - **Shaped glyph geometry (`designos_render::text_geometry`)**: `width = 43.85, height = 16.94`, placed at `(0.00, 0.00)` (top-left) inside the `52.00 × 20.00` text box -> `shaped_geom_centre = (151.92, 20.47)` vs `button_centre = (156.00, 22.00)`, **`(dx_geom, dy_geom) = (-4.08 px, -1.53 px)`** (fails `<= 1 px` on both axes).
   - **Rendered glyph pixel bbox (`designos_render::render_node` / `designos_render_png`)**: `x = [130, 172], y = [15, 28]` -> `pixel_bbox_centre = (151.00, 21.50)`, **`(dx_px, dy_px) = (-5.00 px, -0.50 px)`**.
   - **After `designos_update_node` (`"Sign in"` -> `"Log in"`, as in QA `v061-ai-mcp-log.json`)**: `node.size` remained stale at `(52.00, 20.00)` while `text_geometry` shrank to `width = 40.10, height = 16.94` at `(0.00, 0.00)` -> `shaped_geom_centre = (150.05, 20.47)`, **`(dx_geom, dy_geom) = (-5.95 px, -1.53 px)`**.

2. **Button with `preset: "easeui-button-primary"` and `text: "Continue"` shorthand (`Submit Button`)**:
   - **Button frame rect**: `pos = (24.00, 196.00)`, `size = (312.00, 38.00)`, `button_centre = (156.00, 19.00)`.
   - **Text child rect**: `pos = (125.50, 10.00)`, `size = (61.00, 18.00)`, `box_centre = (156.00, 19.00)`.
   - **Text alignment fields**: `auto_resize = WidthAndHeight`, `style.align_h = Left`, `style.align_v = Top`.
   - **Shaped glyph geometry**: `width = 56.18, height = 15.73` at `(0.00, 0.00)` inside `61.00 × 18.00` -> `shaped_geom_centre = (153.59, 17.87)`, **`(dx_geom, dy_geom) = (-2.41 px, -1.13 px)`**; rendered pixel bbox centre = `(153.50, 17.50)`, **`(dx_px, dy_px) = (-2.50 px, -1.50 px)`**.

3. **Raw `v061-ai-mcp-log.json` Login Card where the agent omitted both `preset` and `layout` on `"Button"`**:
   - **Button frame rect**: `pos = (24.00, 182.00)`, `size = (312.00, 44.00)`, `auto_layout = None`.
   - **Text child rect**: `pos = (0.00, 0.00)`, `size = (52.00, 20.00)`, `align = (Left, Top)`, `box_centre = (26.00, 10.00)` vs `button_centre = (156.00, 22.00)`, **`(dx_box, dy_box) = (-130.00 px, -12.00 px)`**.

### Root Cause Classification (`(b)` + `(d)`)
- **Cause `(b)` (Primary Engine Cause)**:
  1. `designos_doc::spec::estimate_intrinsic_text_size` estimates text node dimensions heuristically (`ems * fs * weight_boost + 4.0`, min `16.0` wide; `fs * 1.38` tall), which is wider and taller than the actual `cosmic-text` shaped glyph box (`designos_render::text_geometry`), while `TextStyle::default()` leaves `align_h = TextAlignH::Left` and `align_v = TextAlignV::Top`. When `layout_solver` centres `node.size` inside a centred button, `designos_render` places the shaped glyph run at `(0, 0)` (top-left) of that overestimated box — or at the far left of the full button width if `width_sizing: "fill"` is used.
  2. When a text node is created inside an auto-layout container that centres children horizontally and/or vertically (including `easeui-button-primary`, `easeui-button-secondary`, `easeui-badge`, or any frame with `align/justify: "center"`), setting `tp.style.align_h = TextAlignH::Center` and `tp.style.align_v = TextAlignV::Center` centres the shaped glyph run inside `node.size`, so `(button_size - node.size) / 2 + (node.size - glyph_size) / 2 == (button_size - glyph_size) / 2`, making the shaped glyph centre match the button centre to `0.00 px` on both axes regardless of box estimation error.
  3. In `crates/designos-mcp/src/tools/mod.rs` (`build_updated_node`), updating a text node's `text` via `designos_update_node` did not recompute `node.size` when `auto_resize == TextAutoResize::WidthAndHeight` and no explicit `width`/`height` was provided, leaving the old text box width in place.
- **Cause `(d)` (Guideline Text Cause)**:
  1. `LayoutSpec::primary_align` and `LayoutSpec::counter_align` docstrings in `crates/designos-doc/src/spec.rs` did not mention their serde aliases `"justify"` and `"align"`, so the schema-generated `LayoutSpec Fields` table in `generated/rules.md` hid both aliases.
  2. `crates/designos-mcp/rules/design-quality.md` did not warn agents that a plain `"kind": "frame"` without `preset` or `layout` has no auto-layout (placing labels at top-left `(0, 0)`), and that buttons/badges must use `"preset": "easeui-button-primary"` / `"easeui-button-secondary"` / `"easeui-badge"` or `"layout": {"direction": "row", "justify": "center", "align": "center"}`.

### Numbers After Fix (`green.log`)
- **`Button / Button label` (`"Sign in"`, `312×44`)**: `btn_c = (156.0, 22.0)`, `box_c = (156.0, 22.0)`, `geom_c = (156.0, 22.0)`, `px_c = (155.0, 22.5)` -> **`dx_geom = 0.00, dy_geom = 0.00, dx_px = 1.00, dy_px = 0.50`**.
- **`Submit Button / Continue` (`"Continue"`, `312×38`)**: `btn_c = (156.0, 19.0)`, `box_c = (156.0, 19.0)`, `geom_c = (156.0, 19.0)`, `px_c = (155.5, 18.5)` -> **`dx_geom = 0.00, dy_geom = 0.00, dx_px = 0.50, dy_px = 0.50`**.
- **`Button / Log in` (after `designos_update_node`, `312×44`)**: `btn_c = (156.0, 22.0)`, `box_c = (156.0, 22.0)`, `geom_c = (156.0, 22.0)`, `px_c = (155.5, 22.5)` -> **`dx_geom = 0.00, dy_geom = 0.00, dx_px = 0.50, dy_px = 0.50`**.
- **`Plain Row Button` (`layout: {"direction": "row", "align": "center", "justify": "center"}`, `312×44`)**: `btn_c = (156.0, 22.0)`, `box_c = (156.0, 22.0)`, `geom_c = (156.0, 22.0)`, `px_c = (155.0, 22.5)` -> **`dx_geom = 0.00, dy_geom = 0.00, dx_px = 1.00, dy_px = 0.50`**.
- **`Plain Col Button` (`layout: {"direction": "column", "align": "center", "justify": "center"}`, `width_sizing: "fill"`, `240×44`)**: `btn_c = (120.0, 22.0)`, `box_c = (120.0, 22.0)`, `geom_c = (120.0, 22.0)`, `px_c = (119.5, 21.5)` -> **`dx_geom = 0.00, dy_geom = 0.00, dx_px = 0.50, dy_px = 0.50`**.

---

## 3. Skills Loaded & What Changed Because of Them
- **`es:dev` & `es:feature-dev`**: Followed the DEV phase directly from the controller's ruling without pausing for brainstorm or spawning a verifier subagent; captured red-first (`red.log`, `before.png`) and green (`green.log`, `after.png`) evidence before committing.
- **`es:fable-thinking` & `es:astra-work`**: Used Fable Loop (`FRAME` / `MODEL` / `DECIDE` + `PREDICT:` before tool calls) to test all four hypotheses `(a)`, `(b)`, `(c)`, `(d)` against actual geometry/pixel measurements instead of guessing, discovering that `(b)` (`estimate_intrinsic_text_size` overestimation combined with `TextAlignH::Left` / `TextAlignV::Top`, plus stale `node.size` on `designos_update_node`) and `(d)` (`rules.md` omitting `"justify"`/`"align"` aliases and button auto-layout guidance) both contributed.
- **`es:rust` & `es:rust-gpu-canvas`**: Kept `designos-doc` free of any dependency on `designos-render`, avoided extra allocations by computing `(centre_h, centre_v)` via a zero-allocation `Document::parent_centres_children(NodeId) -> (bool, bool)` query, and factored `CentringMetrics` in the test to satisfy `-D clippy::type-complexity`.

---

## 4. Epistemic Ledger & Fable Self-Attack
- **[FACT]** On `origin/main`, a button built with `preset: "easeui-button-primary"` or `layout: {"direction": "row", "align": "center", "justify": "center"}` places its `node.size` text box at the button's centre (`box_c = (156.0, 22.0)`), but `estimate_intrinsic_text_size` overestimates `node.size` (`52×20` vs shaped `43.85×16.94` for `"Sign in"`, `61×18` vs `56.18×15.73` for `"Continue"`) while `tp.style.align_h = Left` and `tp.style.align_v = Top`, shifting the shaped glyphs top-left by `(-4.08 px, -1.53 px)` (`(-5.00 px, -0.50 px)` in rendered pixels).
- **[FACT]** On `origin/main`, calling `designos_update_node` to change `"Sign in"` to `"Log in"` did not update `node.size` on `TextAutoResize::WidthAndHeight` text nodes, increasing the horizontal offset to `-5.95 px`.
- **[FACT]** In `v061-ai-mcp-log.json`, the agent also built `"Button"` as a bare `"kind": "frame"` without `preset` or `layout`, leaving `auto_layout = None` and placing `"Button label"` at `(0.0, 0.0)`.
- **[FACT]** With `Document::parent_centres_children` propagating `TextAlignH::Center` / `TextAlignV::Center` to text children of centred auto-layout containers and `build_updated_node` refreshing `node.size`, `dx_geom = 0.00 px, dy_geom = 0.00 px` and `dx_px <= 1.00 px, dy_px <= 0.50 px` across all tested buttons.
- **[INFERENCE]** Documenting `(alias: "justify")` and `(alias: "align")` in `LayoutSpec` and adding explicit button preset/auto-layout guidance in `rules/design-quality.md` (`generated/rules.md`, `23,866` chars <= `24,000` budget) prevents agents reading `designos_get_guidelines` from omitting `preset`/`layout` on buttons.
- **[ASSUMPTION]** None remaining on engine behaviour.
- **[UNKNOWN]** Live external LLM compliance rate without running an evaluation loop against a live model API.
- **Weakest link**: Whether an external LLM ignores the updated `design-quality.md` rule and emits a raw `"kind": "frame"` without `preset` or `layout` when building a custom button.
- **Self-attack**: Could setting `tp.style.align_h = Center` / `align_v = Center` on text children inside centred frames break `export_spec` -> `apply_spec` round-trip determinism (`spec_apply_and_export_roundtrip` or `easeui_alias_bit_identical`)? Verified: both `apply_spec` and re-applied `export_spec` run `build_spec_node_bound` with the same parent `AutoLayout`, so all 649 `designos-doc` tests and 223 `designos-mcp` unit tests pass bit-identically.

---

## 5. Acceptance Table

| # | Check | Command | Result / Output |
|---|---|---|---|
| 1a | Red-first test on base (`red.log`) | `DESIGNOS_DUMP_CARD_PNG=.../before.png cargo test -p designos-mcp --locked --test button_label_centre -- --nocapture` | Failed as expected on base (`HEAD`): `[Plain Row Button] btn_c=(156.0, 22.0) box_c=(156.0, 22.0) geom_c=(151.92432, 20.469727) px_c=(151.0, 21.5) dx_geom=4.08 dy_geom=1.53 dx_px=5.00`; `[Button/Button label] ... dx_geom=4.08 dy_geom=1.53 dx_px=5.00 dy_px=0.50`. Saved in `plans/reports/agy-1098-ai-card-button-label-evidence/red.log`. |
| 1b | Green test on head (`green.log`) | `DESIGNOS_DUMP_CARD_PNG=.../after.png cargo test -p designos-mcp --locked --test button_label_centre -- --nocapture` | `running 2 tests` — `[Plain Row Button] dx_geom=0.00 dy_geom=0.00 dx_px=1.00 dy_px=0.50`, `[Plain Col Button] dx_geom=0.00 dy_geom=0.00 dx_px=0.50 dy_px=0.50`, `[Button/Button label] dx_geom=0.00 dy_geom=0.00 dx_px=1.00 dy_px=0.50`, `[Submit Button/Continue] dx_geom=0.00 dy_geom=0.00 dx_px=0.50 dy_px=0.50`, `[Button/Log in] dx_geom=0.00 dy_geom=0.00 dx_px=0.50 dy_px=0.50` -> `test result: ok. 2 passed; 0 failed`. Saved in `plans/reports/agy-1098-ai-card-button-label-evidence/green.log`. |
| 2a | Rustfmt check | `cargo fmt --all -- --check` | Exit `0` (0 diffs). |
| 2b | Workspace Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | `Finished dev profile [unoptimized + debuginfo] target(s) in 17.04s` — Exit `0` (0 warnings). |
| 3a | `designos-mcp` test suite | `cargo test -p designos-mcp --locked` | `223 passed` (lib) + all integration test binaries (`button_label_centre`: 2 passed, `session`, `spec_text_colour`, `text_fill`, `tools_snapshot`: 5 passed, `version`: 3 passed) — Exit `0`. |
| 3b | `designos-eval` test suite | `cargo test -p designos-eval --locked` | `1 passed` (lib) + `16 passed` (`harness`) + `8 passed` (`real_engine`) — Exit `0`. |
| 3c | `designos-doc` test suite | `cargo test -p designos-doc --locked` | `649 passed; 0 failed` (lib) + integration tests — Exit `0`. |
| 3d | `designos-render` test suite | `cargo test -p designos-render --locked` | All unit and integration tests passed — Exit `0`. |
| 4 | Visual evidence (`before.png`, `after.png`) | Rendered via `designos_render_png` (`scale: 2.0`) | Saved at `plans/reports/agy-1098-ai-card-button-label-evidence/before.png` and `plans/reports/agy-1098-ai-card-button-label-evidence/after.png`. |

---

## 6. Changes Per File
- `crates/designos-doc/src/spec.rs`:
  - Added `Document::parent_centres_children(NodeId) -> (bool, bool)` to inspect whether an auto-layout container centres its children horizontally and/or vertically.
  - In `Document::build_spec_node_bound`, propagated `TextAlignH::Center` and `TextAlignV::Center` to text children (`kind: "text"`) and `spec.text` shorthand labels when their parent container centres children on the corresponding axis.
  - Made `estimate_intrinsic_text_size` `pub` so `designos-mcp` can recompute intrinsic text box dimensions when text content changes.
  - Documented `(alias: "justify")` on `LayoutSpec::primary_align` and `(alias: "align")` on `LayoutSpec::counter_align`.
- `crates/designos-mcp/src/tools/mod.rs`:
  - In `build_created_node`, aligned new text nodes (`TextAlignH::Center` / `TextAlignV::Center`) when created inside a centring auto-layout parent.
  - In `build_updated_node`, recomputed `node.size = estimate_intrinsic_text_size(props)` when `text` is updated on an auto-resizing (`TextAutoResize::WidthAndHeight`) text node without explicit `width`/`height`.
- `crates/designos-mcp/rules/design-quality.md`, `crates/designos-mcp/generated/rules.md`, `skills/design-os-studio/rules.md`, `crates/designos-mcp/tests/snapshots/tools.json`:
  - Added explicit guidance that buttons, pills, and badges must use `"preset": "easeui-button-primary"` / `"easeui-button-secondary"` / `"easeui-badge"` or `"layout": {"direction": "row", "justify": "center", "align": "center"}`, regenerated `rules.md` and `tools.json` via `gen-tool-docs` (`23,866` chars <= `24,000` budget).
- `crates/designos-mcp/tests/button_label_centre.rs`:
  - Added regression tests covering the QA Login Card buttons (`easeui-button-primary` with child text node, with `text` shorthand, and after `designos_update_node`) and plain spec frames with `align/justify: center` in both row and column directions.
- `plans/reports/agy-1098-ai-card-button-label-evidence/`:
  - Added `red.log`, `green.log`, `before.png`, and `after.png`.

---

## Round 2

### 1. What Could NOT Be Verified
- Stochastic compliance of external LLMs in live interactive sessions (whether an LLM might still emit a bare `"kind": "frame"` with no `preset` and no `layout`) could not be tested against live external model APIs because worker rules forbid invoking `claude`, `codex`, `gemini`, `grok`, or `agy`. In a frame with no `layout`, positions are explicit, so the engine must not move the text; the rules-pack sentence stays the fix (documented under `Known limit` on PR #1151).

### 2. Conclusion & Exact Conditions Now Coded for Centring
- **Conclusion**: Restricted parent-driven text alignment propagation in `crates/designos-doc/src/spec.rs` to hugging labels with default alignment only, removed alignment propagation from `build_created_node` (`designos_create_node`), updated `build_created_node` and `build_updated_node` in `crates/designos-mcp/src/tools/mod.rs` to set `TextAutoResize::None` / `TextAutoResize::Height` when `width` or `height` is set so user-sized text is never resized on subsequent text edits, and updated `crates/designos-mcp/tests/button_label_centre.rs` with 1 px geometry / 2 px raster tolerances plus regression tests for fill-width columns, `create_node` in a centred row, and sized-text updates.
- **Exact Conditions Coded for Centring (`crates/designos-doc/src/spec.rs`)**:
  1. **Direct `"kind": "text"` node (`spec.rs:1330-1340`)**:
     - `let (parent_centre_h, parent_centre_v) = self.parent_centres_children(parent);`
     - `let hugs_h = spec.width.is_none() && spec.width_sizing.as_deref().is_none_or(|s| parse_sizing(s) == Sizing::Hug);`
     - `let hugs_v = spec.height.is_none() && spec.height_sizing.as_deref().is_none_or(|s| parse_sizing(s) == Sizing::Hug);`
     - Horizontal centring (`tp.style.align_h = TextAlignH::Center`) is set iff `parent_centre_h && spec.text_style.is_none() && tp.style.align_h == TextAlignH::Left && hugs_h`.
     - Vertical centring (`tp.style.align_v = TextAlignV::Center`) is set iff `parent_centre_v && spec.text_style.is_none() && tp.style.align_v == TextAlignV::Top && hugs_v`.
  2. **Frame `spec.text` shorthand child (`spec.rs:1445-1453`, always a hugging child label)**:
     - Removed pre-style centring at `spec.rs:1376`.
     - After `spec.text_style` and `apply_text_overrides` run:
       - `tp.style.align_h = TextAlignH::Center` iff `self_centre_h && spec.text_style.is_none() && tp.style.align_h == TextAlignH::Left`.
       - `tp.style.align_v = TextAlignV::Center` iff `self_centre_v && spec.text_style.is_none() && tp.style.align_v == TextAlignV::Top`.
  3. **`designos_create_node` (`crates/designos-mcp/src/tools/mod.rs:138-149`)**:
     - Does **not** set `align_h` or `align_v` from `parent_centres_children`.
     - Sets `props.auto_resize = TextAutoResize::None` because `designos_create_node` constructs a fixed `width × height` box (`Rect::new(x, y, width, height)`).
  4. **`designos_update_node` (`crates/designos-mcp/src/tools/mod.rs:241-269`)**:
     - When `u.height.is_some()` on a text node whose `auto_resize` is `WidthAndHeight` or `Height`, sets `props.auto_resize = TextAutoResize::None` (matching `ops.rs:53`).
     - Else when `u.width.is_some()` on a text node whose `auto_resize` is `WidthAndHeight`, sets `props.auto_resize = TextAutoResize::Height`.
     - Recomputes `node.size = estimate_intrinsic_text_size(props)` on a `text` change only while `props.auto_resize == TextAutoResize::WidthAndHeight`.

### 3. Epistemic Ledger
- **[FACT]** On Round 1 head (`b4ea30bd4`), `r2-red.log` confirms all 3 regression scenarios failed:
  1. Fill-width column (`width_sizing: "fill"` on `"Get started"` and Grok's `"Welcome back to your account"` with `text_style: "Body"`): `plain_col_align_h=Center, welcome_align=(Center, Top)` instead of `Left` / `(Left, Top)`.
  2. `designos_create_node` in `easeui-row`: `align=(Left, Center), auto_resize=WidthAndHeight` instead of `(Left, Top)`.
  3. Sized-text update sequence: `create_node` with `width: 240, height: 32` followed by `update_node { text: "Hello world" }` shrank `n1.size` to `(69.0, 17.0)`, and `update_node { width: 240 }` on a hugging text followed by `update_node { text: "Hello world" }` shrank `n2.size` to `(80.0, 20.0)`.
- **[FACT]** On Round 2 head (`r2-green.log`), all 4 tests in `button_label_centre.rs` pass (`4 passed; 0 failed`), and `r2-after.png` confirms the preset buttons (`"Sign in"` and `"Continue"`) remain centred (`dx_geom = 0.00, dy_geom = 0.00, dx_px <= 1.00, dy_px = 0.50`).
- **[FACT]** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test -p designos-doc --locked` (649 + 3 + 5 passed), `cargo test -p designos-mcp --locked` (223 unit + 70 integration passed), `cargo test -p designos-eval --locked` (1 + 16 + 8 passed), `cargo test -p designos-render --locked` (55 unit + 169 integration passed), and `cargo test -p designos-app --bin designos --locked` (2950 passed, 0 failed) all exit `0`.
- **[INFERENCE]** None required for engine behaviour; all paths are directly covered by unit and integration tests.
- **[ASSUMPTION]** None.
- **Weakest link**: External LLM adherence to the rules-pack guidance against emitting a bare `"kind": "frame"` with no `preset` and no `layout` for buttons.
- **Unresolved questions**: None.

### 4. Skills Loaded & What Changed Because of Them
- **`es:dev` & `es:feature-dev`**: Executed Round 2 directly in the DEV phase from the controller's rulings without stopping for re-planning or launching a verifier; captured red-first (`r2-red.log`) before modifying `spec.rs` and `tools/mod.rs`, then captured green (`r2-green.log`, `r2-after.png`).
- **`es:fable-thinking` & `es:astra-work`**: Stated `PREDICT:` before each tool batch, verified all 3 failure modes in `r2-red.log` before touching production code, and verified all workspace gates and visual output (`r2-after.png`) before reporting.
- **`es:rust` & `es:rust-gpu-canvas`**: Applied Law 1 (`es:rust`) in `button_label_centre.rs` by extracting `Copy` values (`TextAlignH`, `(Vec2, TextAutoResize)`) before subsequent mutable `engine.call` borrows.

### 5. Changes Per File (Round 2)
- `crates/designos-doc/src/spec.rs`:
  - Guarded `tp.style.align_h = TextAlignH::Center` and `tp.style.align_v = TextAlignV::Center` on `"kind": "text"` nodes with `spec.text_style.is_none()`, default axis alignment (`Left` / `Top`), and axis hugging (`spec.width.is_none()` / `spec.height.is_none()` and no `fill` or `fixed` sizing).
  - Removed pre-style alignment assignment on `spec.text` shorthand labels and guarded post-style alignment assignment with `spec.text_style.is_none()` and default axis alignment (`Left` / `Top`).
- `crates/designos-mcp/src/tools/mod.rs`:
  - Removed `parent_centres_children` alignment propagation from `build_created_node` and set `props.auto_resize = TextAutoResize::None` on created text nodes.
  - Updated `build_updated_node` to transition `props.auto_resize` to `TextAutoResize::None` when `height` is set (or both `width` and `height` are set) and `TextAutoResize::Height` when only `width` is set on a `WidthAndHeight` text node, and to recompute `node.size` on text edits only while `props.auto_resize == TextAutoResize::WidthAndHeight`.
- `crates/designos-mcp/tests/button_label_centre.rs`:
  - Updated raster-pixel centre tolerance to `<= 2.0` px while keeping shaped-glyph geometry centre tolerance at `<= 1.0` px.
  - Changed the `width_sizing: "fill"` column case (`Plain Col Button`) and added Grok's `"Welcome back to your account"` (`width_sizing: "fill", text_style: "Body"`) case to assert alignment stays `Left` / `(Left, Top)`.
  - Added `create_node_in_centred_row_keeps_default_alignment` and `sized_text_update_node_never_resizes_user_sized_text`.
- `plans/reports/agy-1098-ai-card-button-label-evidence/`:
  - Added `r2-red.log`, `r2-green.log`, and `r2-after.png` (and refreshed `after.png`).

### 6. Round 2 Acceptance Table

| # | Check | Command | Result |
|---|---|---|---|
| 1a | Red-first (`r2-red.log`) | `cargo test -p designos-mcp --locked --test button_label_centre -- --nocapture` (on `b4ea30bd4`) | `FAILED. 1 passed; 3 failed` (`plain_spec_frame_with_align_and_justify_center_centres_label_within_1px`, `create_node_in_centred_row_keeps_default_alignment`, `sized_text_update_node_never_resizes_user_sized_text`). |
| 1b | Green (`r2-green.log`) | `DESIGNOS_DUMP_CARD_PNG=.../r2-after.png cargo test -p designos-mcp --locked --test button_label_centre -- --nocapture` | `ok. 4 passed; 0 failed`. |
| 2a | Rustfmt | `cargo fmt --all -- --check` | Exit `0`. |
| 2b | Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Exit `0` (`Finished dev profile [unoptimized + debuginfo] target(s) in 33.17s`). |
| 3a | `designos-doc` tests | `cargo test -p designos-doc --locked` | `649 passed` (lib) + `3 passed` + `5 passed` — Exit `0`. |
| 3b | `designos-mcp` tests | `cargo test -p designos-mcp --locked` | `223 passed` (lib) + `70 passed` across 12 integration test binaries (`button_label_centre`: 4 passed) — Exit `0`. |
| 3c | `designos-eval` tests | `cargo test -p designos-eval --locked` | `1 passed` (lib) + `16 passed` (`harness`) + `8 passed` (`real_engine`) — Exit `0`. |
| 3d | `designos-render` tests | `cargo test -p designos-render --locked` | `55 passed` (lib) + `169 passed` across 20 integration test binaries — Exit `0`. |
| 3e | `designos-app --bin designos` tests | `cargo test -p designos-app --bin designos --locked` | `ok. 2950 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 165.64s` — Exit `0`. |
| 4 | Visual check (`r2-after.png`) | Inspected `plans/reports/agy-1098-ai-card-button-label-evidence/r2-after.png` | Preset buttons `"Sign in"` and `"Continue"` remain centred. |

### 7. Exact Commands Run and Exact Pasted Output

#### A. Red-first (`r2-red.log`)
```text
$ export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"; export CARGO_TARGET_DIR="$PWD/out/cargo"; env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked --test button_label_centre -- --nocapture
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1098-button-label/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.57s
     Running tests/button_label_centre.rs (out/cargo/debug/deps/button_label_centre-f387cf5f43ec35ff)

running 4 tests
[create_node in easeui-row] align=(Left, Center) auto_resize=WidthAndHeight

thread 'create_node_in_centred_row_keeps_default_alignment' (56328996) panicked at crates/designos-mcp/tests/button_label_centre.rs:414:5:
assertion `left == right` failed: designos_create_node must not overwrite text alignment inside a centring parent
  left: (Left, Center)
 right: (Left, Top)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test create_node_in_centred_row_keeps_default_alignment ... FAILED
[sized_text_update] n1.size=Vec2 { x: 69.0, y: 17.0 } ar1=WidthAndHeight n2.size=Vec2 { x: 80.0, y: 20.0 } ar2=WidthAndHeight

thread 'sized_text_update_node_never_resizes_user_sized_text' (56328999) panicked at crates/designos-mcp/tests/button_label_centre.rs:503:5:
assertion `left == right` failed: text created with width: 240, height: 32 must keep 240x32 on a text-only update_node
  left: Vec2 { x: 69.0, y: 17.0 }
 right: Vec2 { x: 240.0, y: 32.0 }
test sized_text_update_node_never_resizes_user_sized_text ... FAILED
[Plain Row Button] btn_c=(156.0, 22.0) box_c=(156.0, 22.0) geom_c=(156.0, 22.0) px_c=(155.0, 22.5) dx_geom=0.00 dy_geom=0.00 dx_px=1.00 dy_px=0.50
[Button/Button label] btn_c=(156.0, 22.0) box_c=(156.0, 22.0) geom_c=(156.0, 22.0) px_c=(155.0, 22.5) dx_geom=0.00 dy_geom=0.00 dx_px=1.00 dy_px=0.50
[Fill-Width Column] plain_col_align_h=Center welcome_align=(Center, Top)

thread 'plain_spec_frame_with_align_and_justify_center_centres_label_within_1px' (56328997) panicked at crates/designos-mcp/tests/button_label_centre.rs:366:5:
assertion `left == right` failed: width_sizing: fill text in a centred column must keep Left horizontal alignment
  left: Center
 right: Left
test plain_spec_frame_with_align_and_justify_center_centres_label_within_1px ... FAILED
[Submit Button/Continue] btn_c=(156.0, 19.0) box_c=(156.0, 19.0) geom_c=(156.0, 19.0) px_c=(155.5, 18.5) dx_geom=0.00 dy_geom=0.00 dx_px=0.50 dy_px=0.50
[Button/Log in] btn_c=(156.0, 22.0) box_c=(156.0, 22.0) geom_c=(156.0, 22.0) px_c=(155.5, 22.5) dx_geom=0.00 dy_geom=0.00 dx_px=0.50 dy_px=0.50
test qa_card_button_label_is_centred_within_1px ... ok

failures:

failures:
    create_node_in_centred_row_keeps_default_alignment
    plain_spec_frame_with_align_and_justify_center_centres_label_within_1px
    sized_text_update_node_never_resizes_user_sized_text

test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

#### B. Green (`r2-green.log`)
```text
$ export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"; export CARGO_TARGET_DIR="$PWD/out/cargo"; env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" DESIGNOS_DUMP_CARD_PNG="$PWD/plans/reports/agy-1098-ai-card-button-label-evidence/r2-after.png" cargo test -p designos-mcp --locked --test button_label_centre -- --nocapture
   Compiling designos-doc v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1098-button-label/crates/designos-doc)
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1098-button-label/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.49s
     Running tests/button_label_centre.rs (out/cargo/debug/deps/button_label_centre-f387cf5f43ec35ff)

running 4 tests
[create_node in easeui-row] align=(Left, Top) auto_resize=None
test create_node_in_centred_row_keeps_default_alignment ... ok
[sized_text_update] n1.size=Vec2 { x: 240.0, y: 32.0 } ar1=None n2.size=Vec2 { x: 240.0, y: 20.0 } ar2=Height
test sized_text_update_node_never_resizes_user_sized_text ... ok
[Plain Row Button] btn_c=(156.0, 22.0) box_c=(156.0, 22.0) geom_c=(156.0, 22.0) px_c=(155.0, 22.5) dx_geom=0.00 dy_geom=0.00 dx_px=1.00 dy_px=0.50
[Fill-Width Column] plain_col_align_h=Left welcome_align=(Left, Top)
test plain_spec_frame_with_align_and_justify_center_centres_label_within_1px ... ok
[Button/Button label] btn_c=(156.0, 22.0) box_c=(156.0, 22.0) geom_c=(156.0, 22.0) px_c=(155.0, 22.5) dx_geom=0.00 dy_geom=0.00 dx_px=1.00 dy_px=0.50
[Submit Button/Continue] btn_c=(156.0, 19.0) box_c=(156.0, 19.0) geom_c=(156.0, 19.0) px_c=(155.5, 18.5) dx_geom=0.00 dy_geom=0.00 dx_px=0.50 dy_px=0.50
[Button/Log in] btn_c=(156.0, 22.0) box_c=(156.0, 22.0) geom_c=(156.0, 22.0) px_c=(155.5, 22.5) dx_geom=0.00 dy_geom=0.00 dx_px=0.50 dy_px=0.50
test qa_card_button_label_is_centred_within_1px ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

#### C. Rustfmt & Clippy
```text
$ cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.17s
```

#### D. Crate & Binary Test Suites under Isolated Harness
```text
$ cargo test -p designos-doc --locked && cargo test -p designos-mcp --locked && cargo test -p designos-eval --locked && cargo test -p designos-render --locked
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)
test result: ok. 649 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.99s
     Running tests/auto_layout_wrap_fields_compat.rs (out/cargo/debug/deps/auto_layout_wrap_fields_compat-d19362047bc102bf)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/changelog_pillars.rs (out/cargo/debug/deps/changelog_pillars-c7fad459a80a2bad)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-bccb00bdb05568d2)
test result: ok. 223 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.25s
     Running tests/button_label_centre.rs (out/cargo/debug/deps/button_label_centre-f387cf5f43ec35ff)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/colour_detach.rs (out/cargo/debug/deps/colour_detach-b2991cf6df768793)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/colour_var_library.rs (out/cargo/debug/deps/colour_var_library-38d4c6fa04e68313)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/colour_var_refs.rs (out/cargo/debug/deps/colour_var_refs-0cb229ebc6a72994)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/list_lint_rules.rs (out/cargo/debug/deps/list_lint_rules-c7f3e773b00eed89)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.35s
     Running tests/live_doc_epoch.rs (out/cargo/debug/deps/live_doc_epoch-3edd95ca95398ce2)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/live_session_guards.rs (out/cargo/debug/deps/live_session_guards-db92954c1e55c6f7)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/session.rs (out/cargo/debug/deps/session-94984970815e30ab)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/spec_text_colour.rs (out/cargo/debug/deps/spec_text_colour-5c1f9526e20a2113)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/stdio.rs (out/cargo/debug/deps/stdio-2981640d0099e395)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/text_fill.rs (out/cargo/debug/deps/text_fill-69ecba9b20de0fe7)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-eece5487fd8b309c)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/version.rs (out/cargo/debug/deps/version-7b1665a385458f91)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.14s
     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-d08c6d3710e740f5)
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 32.73s
     Running tests/backdrop.rs (out/cargo/debug/deps/backdrop-3cb41bd81d306fa0)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/backdrop_index.rs (out/cargo/debug/deps/backdrop_index-55c6b0018926f285)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.93s
     Running tests/blend.rs (out/cargo/debug/deps/blend-ed0f6462473fcf08)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/board.rs (out/cargo/debug/deps/board-899c8354641ad968)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
     Running tests/board_gallery.rs (out/cargo/debug/deps/board_gallery-36b870257d3bd7db)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/corners.rs (out/cargo/debug/deps/corners-f85864ac9898fb2d)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/font_arrival.rs (out/cargo/debug/deps/font_arrival-6fd71e2bb1b1a8c6)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/fonts.rs (out/cargo/debug/deps/fonts-50ccb76fe237ec4a)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/fx.rs (out/cargo/debug/deps/fx-0ba07c6a17bdb641)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/geometry.rs (out/cargo/debug/deps/geometry-dafba65093b234cb)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gradients.rs (out/cargo/debug/deps/gradients-deefa8b5fe684212)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lint_contrast.rs (out/cargo/debug/deps/lint_contrast-c3fe09ca772ae78c)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/lint_rules.rs (out/cargo/debug/deps/lint_rules-0d7991ec45b3a6f3)
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.63s
     Running tests/mask_without_own_fill.rs (out/cargo/debug/deps/mask_without_own_fill-282ac6f966f81ae3)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/pixmap_size_cap.rs (out/cargo/debug/deps/pixmap_size_cap-22d9ee4c60f3a674)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/render.rs (out/cargo/debug/deps/render-9662c77a6ea0c065)
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/scripts.rs (out/cargo/debug/deps/scripts-33f4b7daa75e2e22)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/text_fill_source.rs (out/cargo/debug/deps/text_fill_source-ff100ca66cf7bec2)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/text_lists.rs (out/cargo/debug/deps/text_lists-4951e93aef616631)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/tile_fonts.rs (out/cargo/debug/deps/tile_fonts-0bc79cd1a9b2a2d7)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/tiles.rs (out/cargo/debug/deps/tiles-c2153534f9f5bcec)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.39s
     Running tests/tiles_edit_while_converging.rs (out/cargo/debug/deps/tiles_edit_while_converging-0aeca1757fdf2596)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/variables.rs (out/cargo/debug/deps/variables-67aa19ca7c041115)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/vector_network.rs (out/cargo/debug/deps/vector_network-d32cb49e6fa53504)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/vectorops.rs (out/cargo/debug/deps/vectorops-351ce1bafa5c40ba)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

$ cargo test -p designos-app --bin designos --locked
test result: ok. 2950 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 165.64s
```

Status: DONE
