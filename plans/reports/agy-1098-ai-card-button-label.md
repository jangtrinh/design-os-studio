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

Status: DONE
