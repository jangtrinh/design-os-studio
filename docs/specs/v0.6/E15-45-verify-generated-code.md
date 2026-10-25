# E15-45 Verify generated code by rendering it and pixel-diffing against our render

- Issue: #635 (task E15-45, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-export/src/verify/{mod.rs,harness.rs,bounds.rs,report.rs}` (new), `crates/designos-export/verify-runtime/` (new: pinned Node package with the render harness; reuses the capture runtime from #276 when present), `crates/designos-export/tests/verify_fixtures/` (new), `crates/designos-diff/src/lib.rs` (use only), `crates/designos-mcp/src/engine.rs` tool list (`designos_verify_code`), `.github/workflows/ci.yml` (verify job).

## 1. Summary and user story

As a developer or agent, I want proof that generated HTML/CSS, Tailwind and React code looks like the design: render the code in a headless browser, pixel-diff it against our bit-exact render, and get the mismatching nodes named.

Codegen correctness (fill and hug to flex mapping, tokens) is unproven without rendering.

## 2. Current state

- FACT: `designos-export` generates CSS (`css_for`, `crates/designos-export/src/css.rs:10`), Tailwind (`tailwind_for`, `tailwind.rs:12`), React (`react_for` / `react_with`, `react.rs:17,24`), SwiftUI, UIKit, Compose, Android XML; the MCP tool `designos_generate_code` lists `css`, `tailwind`, `react`, `swiftui`, `uikit`, `compose`, `android-xml`, `svg` (`crates/designos-mcp/src/engine.rs:112,745-750`). T7.29 (#250) adds an HTML target.
- FACT: codegen maps auto layout to flex with tests such as `golden_auto_layout_and_children` (`css.rs:517`) and sizing helpers (`codegen.rs:1007-1008`), but nothing renders the output.
- FACT: `designos-diff` compares images (`compare`, `diff_image`, `crop`; `crates/designos-diff/src/lib.rs:301-345`) with `DiffOptions { tolerance, detect_aa, ignore }`.
- FACT: #276 (T8.25) introduces a Playwright capture runtime (headless Chromium).

## 3. Behaviour rules

1. **Targets verified:** `html` (from #250), `css` (wrapped in the HTML skeleton codegen uses for CSS output), `tailwind`, `react`. Native targets (SwiftUI, UIKit, Compose, Android XML) are out of scope (no headless renderer).
2. **Codegen option.** Codegen gains `data_ids: bool` (default false); when true every element carries `data-designos-id="<node id>"`. Verification always enables it; normal export output is unchanged.
3. **Harness** (`verify-runtime/`, pinned `package.json` and lockfile: Playwright, Chromium revision, `react` and `react-dom` for server rendering, `@babel/standalone` for JSX, `tailwindcss` v4 CLI):
   - HTML/CSS: loaded as a static page;
   - Tailwind: classes compiled with the pinned Tailwind CLI (content = the generated markup) into a stylesheet, then loaded;
   - React: JSX transformed with Babel, rendered with `react-dom/server` `renderToStaticMarkup` to HTML, then loaded like HTML.
   - Fonts: an `@font-face` block is injected for every family used, pointing at the exact font files `designos-fonts` used for our render (file URLs from its cache), so both sides use the same font binaries.
   - Viewport = the frame's size at device scale factor 1; page background transparent; animations off.
   - Output: a full-page PNG clipped to the frame size, plus `getBoundingClientRect()` for every `[data-designos-id]` element.
4. **Reference render.** `render_node(doc, frame, 1.0)` on the CPU path (`crates/designos-render/src/lib.rs:162`), composited over the same transparent background.
5. **Comparison.**
   - Image: `designos-diff::compare` with `tolerance = 32`, `detect_aa = true`; pass when `ratio ≤ 0.02`.
   - Per node: for each node with an element, compare the element rect with the node's world bounds relative to the frame; a node **mismatches** when any edge differs by more than 1 px, or when the mismatched-pixel ratio inside its bounds exceeds 0.05.
   - The report lists mismatching nodes sorted by mismatched pixels inside their bounds, each with node id, name, expected and actual rect, and pixel ratio. AC2: a broken flex mapping names the node whose layout broke.
6. **Verdict.** `passed = image_ratio ≤ 0.02 && mismatching_nodes == 0`. Thresholds are in `verify.toml` and only change by reviewed commits.
7. **Fixture set (AC1).** `tests/verify_fixtures/` holds our own documents built with `apply_spec`: auto layout row/column/wrap, hug/fill/fixed combinations, grid, padding and gaps bound to variables, text styles, corner radii, strokes inside/center/outside, drop shadows, image fills, nested instances. Each fixture × each of the four targets must pass in CI.
8. **Broken mapping fixture (AC2).** A test-only codegen switch (`cfg(test)`) swaps `justify-content` and `align-items` for horizontal auto layout; the verifier must fail and name the affected child nodes.

## 4. Data model / schema

```rust
pub struct VerifyOptions { pub target: CodeTarget, pub tolerance: u8, pub max_ratio: f64, pub edge_px: f32, pub node_ratio: f64 }
pub struct VerifyReport { pub target: CodeTarget, pub frame: NodeId, pub image_ratio: f64, pub ssim: f64, pub passed: bool,
    pub mismatches: Vec<NodeMismatch>, pub harness_version: String, pub artefacts: VerifyArtefacts }
pub struct NodeMismatch { pub node: NodeId, pub name: String, pub expected: [f32; 4], pub actual: [f32; 4], pub pixel_ratio: f64 }
```

## 5. Contracts

- MCP `designos_verify_code({ node_id, language: "html" | "css" | "tailwind" | "react", write_artefacts?: bool })` → `{ "passed": false, "image_ratio": 0.071, "mismatches": [ { "node_id": 88, "name": "Price row", "expected": [16, 120, 288, 24], "actual": [16, 132, 288, 24], "pixel_ratio": 0.42 } ], "artefacts": { "ours": "verify/88-ours.png", "theirs": "verify/88-html.png", "diff": "verify/88-diff.png" } }` (artefact paths inside the workspace, only when `write_artefacts`).
- Errors: `verify_runtime_missing` (Node or the pinned runtime not installed; details give the install command `designos verify-runtime install`), `verify_target_unsupported` (native targets), `verify_failed_to_render` (browser error text), `node_not_frame` (corrected example uses the nearest frame).
- CLI: `designos verify-runtime install` installs the pinned runtime into the user cache; `designos verify-code <doc> --frame <name|id> --language react [--out dir]`.

## 6. File-format mapping

None.

## 7. UI

None in this task (Dev Mode may show the verdict later).

## 8. Edge cases

1. Fonts missing in the cache: our render fetches them first; if a font cannot be obtained, verification stops with `verify_fonts_unavailable` rather than comparing different faces.
2. Effects the browser renders differently (background blur, glass, noise, texture): nodes with those effects are excluded from the per-node check and their bounds added to `ignore`; the report lists them as `not_comparable`.
3. Text rasterisation differences: the tolerance and AA detection absorb them; per-node checks use rects, which are robust.
4. Image fills: the harness serves image bytes from a temp folder with the same names codegen references.
5. Frames larger than 4096 px: rendered in 4096 px tiles on both sides and compared per tile.
6. No network: the runtime is installed once; verification itself loads only local files (`page.route` blocks all other requests and records them as warnings).

## 9. Non-goals

- Native platform verification.
- Changing codegen output beyond the `data_ids` option.

## 10. Test plan

Seam: browser pixel diff in CI (new `verify` job on `macos-14`, runtime cached), unit tests for the report logic with stored PNG pairs.

Fail-before: the verifier does not exist.

| Test | Input | Expected |
|---|---|---|
| `fixture_set_passes` (AC1) | every fixture × html, css, tailwind, react | `passed = true` |
| `broken_flex_mapping_fails_with_node_named` (AC2) | swapped justify/align switch | `passed = false`; mismatches include the children of the row |
| `rect_tolerance_one_px` | element offset by 1 px vs 2 px (stored pair) | pass vs mismatch |
| `uncomparable_effects_listed` | node with background blur | excluded, listed `not_comparable` |
| `no_external_requests` | fixture | zero non-local requests |

Visual evidence: ours/theirs/diff PNG triple for one passing fixture and for the broken mapping.

## 11. Performance budgets

- One verification ≤ 4 s per frame (browser start amortised across a batch; CI reuses one browser for all fixtures).
- CI job ≤ 10 min. Nothing runs on the Studio UI thread.

## 12. Dependencies and build order

- Needs #250 (T7.29 HTML target), #276 (T8.25 capture runtime).
- Independent of the other E15 tasks.

## 13. Owner rulings that apply

- Full scope: four web targets, per-node mismatch report, CI coverage, MCP tool.
- Licence review: the runtime's npm packages and the Chromium build are dev tooling installed into the user cache, never shipped in the app. Each pinned package gets an E15-23 register entry with its published licence before the CI job lands.
- R4.13 is unaffected: no model is involved.
