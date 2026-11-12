# E12 Canvas, Accessibility & Text: Master Execution Plan (Epic #124)

**Date:** 2026-11-12  
**Milestone:** v0.8 Canvas Polish, Accessible Studio & Rich Typography  
**Base Commit:** `origin/main@e0e44f3c2`  
**Evidence Dossier:** `plans/261112-epic-figma-rea-specs/evidence/rea-e1-e2-e12-vars-layout-text.md`  
**Complementary Specs:** `specs/e12-canvas-a11y-text/` (9 UI bug specs #828, #869, #904, #915, #935, #944, #951, #969, #1019)  
**Clean-Room Standard:** Clean room reverse engineering. Zero proprietary Figma code copied. All Kiwi tags, schema numbers, JS/WASM offsets, and UI strings in own words.

---

## Answer first

- **Epic #124 Scope Clarification:** While `specs/e12-canvas-a11y-text/` addressed the initial wave of 9 UI focus and pointer gesture bugs, this master plan addresses the core engine features of Epic #124:
  1. OpenType Variable Fonts (`FontVariation` axis tags, `opsz` optical sizing, CSS output).
  2. Mixed Inline Rich Text & Text Decorations (`styleOverrideTable`, wavy/dotted underlines, descender skip-ink).
  3. Smart Symbols & Typography Automation (auto-quotes, dashes, symbols, fractions, math symbols, single-step undo).
  4. Missing Fonts Dialog & Font Embedding License Validation (`fsType` permissions).
  5. AccessKit Spatial Canvas Tree (`< 1 ms` culling on 723k-node files, zero `to_json` in `doc_hash`, zero `TileView::rev` bumps).
  6. Enhanced-Contrast Theme Tokens & Native macOS Eyedropper (`NSColorSampler` Display P3).
  7. On-Canvas Video Playback (indirect GPU texture slot without `FrameKey` cache invalidation).
- **Execution Waves & Model Routing:**
  - `Model: "flash"` (`Gemini 3.8 Flash`): Smart symbol substitutions, missing font mapping, theme contrast tokens, unit tests.
  - `Model: "pro"` (`Gemini 4 Argon`): Variable font OpenType shaping with `cosmic-text`, skip-ink bezier path clipping, AccessKit tree synthesis via `SceneIndex`, video frame GPU texture updates.
- **Studio Invariants Upheld:**
  - AccessKit tree generation never calls `doc.to_json()` and never bumps `TileView::rev` on focus changes.
  - No `ctx.load_texture` inside `ctx.data_mut` (locks avoided).
  - Headless test parity preserved in `designos-gpu/src/compile.rs` via `self.interactive()` guards.

---

## 1. What could NOT be verified (read first)

1. **Composite Wavy Underline Shader Parameters:** Whether Figma renders wavy underlines (`textDecorationStyle = WAVY = 2`) via a WGSL procedural sine-wave shader or pre-tessellated cubic bezier spline meshes in GPU mode was not visible in static wasm strings; Studio implements analytical spline tessellation in `tiny-skia` and `lyon` `[INFERENCE]`.
2. **AccessKit Canvas Subtree Pruning Depth:** The exact bounding-box padding threshold Figma's web canvas uses for off-screen accessibility object culling was not identified; Studio culls elements outside the visible viewport rect plus a $200\text{px}$ margin `[INFERENCE]`.
3. **WebM Alpha Channel Support:** Whether Figma supports WebM with VP9 alpha channel video textures or H.264/H.265 only is inferred as standard AVFoundation/GStreamer pipeline support `[ASSUMPTION]`.

---

## 2. Subsystem Architecture

```text
                  ┌────────────────────────────────────────────────────────┐
                  │              Epic #124 Core Subsystems                 │
                  └──────────────────────────┬─────────────────────────────┘
                                             │
             ┌───────────────────────────────┴───────────────────────────────┐
             ▼                                                               ▼
┌─────────────────────────┐                                     ┌─────────────────────────┐
│     Text & Fonts        │                                     │  Accessibility & Canvas │
├─────────────────────────┤                                     ├─────────────────────────┤
│ • Variable Fonts (E12-A)│                                     │ • AccessKit Tree (E12-D)│
│ • Rich Text Runs (E12-B)│                                     │ • F6 Landmarks (E12-D)  │
│ • Smart Symbols (E12-C) │                                     │ • High Contrast (E12-E) │
│ • Missing Fonts (E12-C) │                                     │ • P3 Eyedropper (E12-E) │
│ • Font Licensing (E12-C)│                                     │ • Canvas Video (E12-F)  │
└─────────────────────────┘                                     └─────────────────────────┘
```

---

## 3. Slices & Worker Tiering Table

| Slice ID | Title & Scope | Files Owned | Size (h) | Model Tier | Dependencies |
|---|---|---|---|---|---|
| **E12-S1** | Variable Fonts (`axisTag`, `cosmic-text`, `opsz`, CSS settings) | `designos-doc/src/paint.rs`, `designos-render/src/text.rs`, `designos-export/src/css.rs` | 8 | Argon | None |
| **E12-S2** | Text Decorations (`WAVY`, `DOTTED`, `skipInk`, fill paints) | `designos-doc/src/paint.rs`, `designos-render/src/text.rs`, `designos-gpu/src/compile.rs` | 8 | Argon | E12-S1 |
| **E12-S3** | Mixed Inline Rich Text (`styleOverrideTable` round-trip) | `designos-doc/src/node.rs`, `designos-fig/src/fig.rs`, `designos-fig/src/fig_export.rs` | 10 | Argon | Hot-File Queue |
| **E12-S4** | Smart Symbols & Text Substitutions (`textedit.rs` + undo) | `designos-app/src/textedit.rs`, `designos-app/src/panels/typography.rs` | 4 | Flash | None |
| **E12-S5** | Missing Fonts Replace-All & `fsType` Embedding Permissions | `designos-fonts/src/lib.rs`, `designos-app/src/home/dialog.rs` | 5 | Flash | None |
| **E12-S6** | AccessKit Spatial Canvas Tree (`SceneIndex` fast traversal) | `designos-app/src/agent_a11y.rs`, `designos-render/src/scene_index.rs` | 8 | Argon | None |
| **E12-S7** | F6 5-Region Landmark Cycling & Keyboard Nudge/Resize | `designos-app/src/shell.rs`, `designos-app/src/canvas/mod.rs` | 4 | Flash | E12-S6 |
| **E12-S8** | Enhanced Contrast Theme Tokens & WCAG 7:1 Verification | `designos-app/src/theme.rs`, `design/design.tokens.json` | 3 | Flash | None |
| **E12-S9** | Native macOS P3 Eyedropper (`NSColorSampler` bridging) | `designos-app/src/eyedropper.rs`, `crates/designos-app/Cargo.toml` | 4 | Argon | None |
| **E12-S10**| On-Canvas Video Playback & GPU Texture Streaming | `designos-doc/src/node.rs`, `designos-gpu/src/video.rs`, `designos-app/src/canvas/` | 12 | Argon | None |

---

## 4. Hot-File & Concurrency Rules

1. **`fig.rs` & `fig_export.rs` Serialization:** Slice `E12-S3` touches Kiwi import/export for rich text runs and text decorations. It MUST wait for preceding E1/E2 slices to vacate the hot-file slot.
2. **Text Rendering Concurrency:** Slices `E12-S1` and `E12-S2` both modify `designos-render/src/text.rs`. They must be executed serially by the same worker or sequenced `E12-S1` $\to$ `E12-S2`.
3. **GPU Interactive Constraint:** `E12-S2` (decorations) and `E12-S10` (video) must keep all LOD/dynamic program allocations strictly guarded by `self.interactive()` in `designos-gpu/src/compile.rs` so headless rendering stays bit-exact.

---

## 5. Rollback & Parity Gates

Every slice must pass the standard quality gates:
```bash
# 1. Workspace Unit & Integration Tests (excluding heavy VSF benchmark)
cargo test --workspace --lib -- --skip vsf_pcp

# 2. Text & Font Specific Tests
cargo test -p designos-render text -- --skip vsf_pcp
cargo test -p designos-fonts

# 3. Accessibility & A11y Tree Tests
cargo test -p designos-app a11y -- --skip vsf_pcp

# 4. Zero Fake UI Gate
python3 scripts/zero_fake_ui_gate.py
```
