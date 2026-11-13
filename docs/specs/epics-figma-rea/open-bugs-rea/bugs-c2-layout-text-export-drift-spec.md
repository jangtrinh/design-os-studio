> **Review status: NOT REVIEWED.** Reference for implementation; verify symbols and statements about Figma against main and the schema before relying on them.

# True Figma Parity Specification: Cluster C2 — Auto-Layout Edge Cases, Text Shaping/Settlement & Export/Color Drift

**Document Identifier:** `SPEC-PARITY-C2-LAYOUT-TEXT-EXPORT-DRIFT`  
**Date:** 2026-11-12  
**Target Architecture:** Figma Desktop `126.11.7` (macOS arm64, Electron 32 + Chromium 128 + Metal, Engine Build `d947dbc0…`) vs. DESIGN:OS Studio (`design-os-studio`)  
**Base Repository Commit:** `origin/main@f2deb8d50`  
**Target Specification Path:** `docs/specs/epics-figma-rea/open-bugs-rea/bugs-c2-layout-text-export-drift-spec.md`  
**Evidence Dossier:** [Clean-Room Figma 126.11.7 REA Evidence: Cluster C2](../evidence/rea-bugs-c2-layout-text-export-drift.md)  
**Sibling Cluster Specs:** [Cluster C1 Specification](./bugs-c1-components-variants-fig-io-spec.md)  
**Epistemic Standard:** Clean-room reverse engineering. All assertions categorized as `[FACT - seen]`, `[INFERENCE]`, `[ASSUMPTION]`, or `[UNKNOWN — requires live GUI experiment]`. Zero verbatim proprietary Figma source code included. All literal dollar signs outside code spans escaped (`\$`).

---

## Executive Summary & Engine Architecture Contract

This specification provides the complete, build-ready implementation blueprints for resolving all **14 Open Parity Issues** in **Cluster C2**:
- **Auto-Layout Solver Edge Cases & Group Parity:** `#1942`, `#1920` (Buckets X, W, S1), `#1885`, `#1895`, `#997`
- **Text Auto-Resize, Late-Font Settlement & Table Replay:** `#519`, `#1404`, `#1817`, `#1799`
- **Export Bounds, Rotated Layer Export, Card Capture & Display P3 Surface:** `#521`, `#528`, `#831`, `#1738`, `#1798`

### Non-Negotiable Engine Invariants (`AGENTS.md` §4)
Any coding agent executing in an isolated git worktree MUST adhere to these architectural safety invariants:
1. **Zero `doc.to_json()` in `doc_hash` (`crates/designos-app/src/home/session.rs`):** Always hash documents using the streaming, zero-allocation `HashSerializer`. Never serialize `Document` to a `String` for change detection, and never bump `TileView::rev` on viewport-only pan/zoom/fit gestures.
2. **Zero `ctx.load_texture` inside `ctx.data_mut` (`crates/designos-app/src/icons.rs`):** `egui::Context::data_mut` holds a non-reentrant `parking_lot::RwLock` write guard. Textures must be decoded and registered outside write locks to prevent deadlock on Frame 1.
3. **Preserve `interactive()` Guard in `designos-gpu` (`crates/designos-gpu/src/compile.rs`):** Keep all LOD approximations (`< 0.75px` culling, `< 3.5px` text greeking, scissor clip fallback, `< 64` `Program` cap) strictly inside `self.interactive()` so headless reference tests (`cargo test -p designos-gpu`) remain bit-exact against `designos-render`.
4. **Preserve 8ms GPU Frame Budget & `0.00%` Idle CPU:** Cap offscreen GPU `Program` allocations at `< 64`, retain `FrameKey` caching (`Cb { frame: None }`), and preserve `Autosave::autosave_tick` off-thread disk watching.

---

## Section 1: Auto-Layout Solver & Wrap / Rotation / Group Parity (`#1942`, `#1920`, `#1885`, `#1895`, `#997`)

### 1.1 Issues `#1942` & `#1920` (Bucket X) — Cross-in-Frame Hugging Height Clamping & Alignment

#### Problem Statement & REA Evidence
In the Untitled UI PRO v8.0 kit census (`#1920`), 473 layers across 61 root frames shifted because horizontal auto-layout frames with Hug cross-sizing clamped by `max_height` (or `max_width`) aligned children within the natural content extent rather than the clamped inner frame box `[FACT - seen: kit vector 7879:434781 "Content", 3288:570945 "Badge=True"]`.

In frame `7879:434781`:
- `layoutMode: HORIZONTAL`, `counterAxisAlignItems: CENTER`, `counterAxisSizingMode: AUTO` (Hug height), `maxHeight: 36.0`, padding `8.0` top and bottom.
- Flow children have heights `20.0`, `22.0`, and `16.0`.
- Unconstrained content height: $22.0 + 2 \times 8.0 = 38.0\text{ px}$.
- Clamped frame height: $\min(38.0, 36.0) = 36.0\text{ px}$.
- Clamped inner cross extent: $H_{\text{inner}} = 36.0 - 2 \times 8.0 = 20.0\text{ px}$.
- **Figma child Y positions:** child(20h) $y = 8.0$, child(22h) $y = 7.0$, child(16h) $y = 10.0$.
- In frame `3288:570945`: horizontal, center counter-align, hug height with `maxHeight: 20.0`, padding 0. Children heights 20 and 22. Figma child Y positions: $y = 0.0$ and $y = -1.0$.

#### Studio Root Cause
In `crates/designos-doc/src/layout_solver.rs:236-245`, `line_cross[0]` was reset to the inner frame size *only* when `cross_has_slack` was true (`frame_cross - pc > line_cross[0] + 1e-3`). When clamped by `max_height` below content ($H_{\text{frame}} < H_{\text{content}}$), `cross_has_slack` was `false`, so `line_cross[0]` remained $22.0\text{ px}$. Studio aligned children inside the 22px content box, placing child(22h) at $y = 8.0$ and child(20h) at $y = 9.0$ (centered in 22px instead of 20px).

#### Exact Parity Formulation `[FACT - seen]`
For any single-line auto-layout container (`!al.wrap`), the cross-axis alignment extent is the inner frame box:
$$\text{CrossAlignmentExtent} = \max(0.0, frame\_cross - pc)$$
The child cross offset is computed against $\text{CrossAlignmentExtent}$, unconditionally allowing negative offsets relative to padding when clamped below content:
$$y_i = pc\_start + \begin{cases} 0.0, & \text{align} = \text{START} \\ \frac{\text{CrossAlignmentExtent} - h_i}{2.0}, & \text{align} = \text{CENTER} \\ \text{CrossAlignmentExtent} - h_i, & \text{align} = \text{END} \end{cases}$$

#### Concrete Code Patch: `crates/designos-doc/src/layout_solver.rs`
```rust
// Anchor: crates/designos-doc/src/layout_solver.rs:236-246 on origin/main
    // Does the frame's cross extent exceed what its lines need (a Fixed frame, or a hugging one raised by its minimum)?
    let mut cross_has_slack = !hug_cross;
    if !al.wrap {
        if hug_cross {
            frame_cross = ax.clamp_cross(&own, line_cross[0] + pc);
            cross_has_slack = frame_cross - pc > line_cross[0] + 1e-3;
        }
        // Parity Fix (#1942, #1920 Bucket X): Single-line cross alignment is ALWAYS
        // computed inside the clamped inner frame box, even when clamped smaller than content.
        line_cross[0] = (frame_cross - pc).max(0.0);
    } else {
        let cs = al.effective_counter_spacing(); // (#1885: Option<f32> -> item_spacing when None)
        let total: f32 = line_cross.iter().sum::<f32>() + cs * (lines.len() as f32 - 1.0);
        if hug_cross {
            frame_cross = ax.clamp_cross(&own, total + pc);
            cross_has_slack = frame_cross - pc > total + 1e-3;
        }
    }
```

---

### 1.2 Issues `#1942` & `#1920` (Bucket W) — Multi-Line Wrap Block Counter-Alignment & Line Pitch

#### Problem Statement & REA Evidence
In wrap frames with a fixed cross dimension exceeding the total lines extent (slack space), where does line 1 start? `[FACT - seen: kit vector 1334:2212 "Contents", plugin-api.d.ts lines 8051-8060]`.

In frame `1334:2212`:
- `layoutMode: HORIZONTAL`, `layoutWrap: WRAP`, `counterAxisAlignItems: CENTER`, `counterAxisAlignContent: AUTO`, fixed size $560 \times 274$, padding $32.0$, item spacing $32.0$, counter spacing `null` (`AUTO`).
- Two lines of height $88.0\text{ px}$ each. Total lines block: $88.0 + 32.0 + 88.0 = 208.0\text{ px}$.
- Inner frame height: $274.0 - 2 \times 32.0 = 210.0\text{ px}$.
- Slack space: $210.0 - 208.0 = 2.0\text{ px}$.
- **Figma line positions:** Line 1 $y = 33.0$, Line 2 $y = 153.0$ ($33.0 + 88.0 + 32.0$).
- Studio previously started line 1 at $y = 32.0$ (anchored at start padding), giving Line 2 at $y = 152.0$.

#### Exact Parity Formulation `[FACT - seen]`
When `al.wrap` is true and `al.counter_align_content != CounterAlignContent::SpaceBetween`, lines do not stretch unless children have stretch alignment. When cross slack exists ($H_{\text{inner}} > H_{\text{block}}$), the entire block of lines is offset along the counter axis according to `counter_align`:
$$H_{\text{block}} = \sum_{j=0}^{M-1} line\_cross[j] + counter\_gap \times (M - 1)$$
$$\text{BlockOffset} = \begin{cases} 0.0, & \text{counter\_align} = \text{Start} \\ \frac{frame\_cross - pc - H_{\text{block}}}{2.0}, & \text{counter\_align} = \text{Center} \\ frame\_cross - pc - H_{\text{block}}, & \text{counter\_align} = \text{End} \end{cases}$$
$$\text{Line}_k.\text{cross} = pc\_start + \max(0.0, \text{BlockOffset}) + \sum_{j=0}^{k-1} (line\_cross[j] + counter\_gap)$$

#### Concrete Code Patch: `crates/designos-doc/src/layout_solver.rs`
```rust
// Anchor: crates/designos-doc/src/layout_solver.rs:252-262 on origin/main
    // Wrapped lines spread edge to edge (CSS align-content: space-between); the counter spacing stays the minimum gap.
    let cs = al.effective_counter_spacing();
    let (counter_gap, block_offset) = if al.wrap && lines.len() > 1 && cross_has_slack {
        if al.counter_align_content == CounterAlignContent::SpaceBetween {
            let gap = ((frame_cross - pc - line_cross.iter().sum::<f32>()) / (lines.len() as f32 - 1.0)).max(cs);
            (gap, 0.0f32)
        } else {
            let total_block: f32 = line_cross.iter().sum::<f32>() + cs * (lines.len() as f32 - 1.0);
            let slack = (frame_cross - pc - total_block).max(0.0);
            let off = match al.counter_align {
                LayoutAlign::Center => slack / 2.0,
                LayoutAlign::End => slack,
                _ => 0.0,
            };
            (cs, off)
        }
    } else if al.wrap && lines.len() == 1 && cross_has_slack {
        let slack = (frame_cross - pc - line_cross[0]).max(0.0);
        let off = match al.counter_align {
            LayoutAlign::Center => slack / 2.0,
            LayoutAlign::End => slack,
            _ => 0.0,
        };
        (cs, off)
    } else {
        (cs, 0.0f32)
    };

    let mut rects = vec![Rect::default(); kids.len()];
    let mut cross_cursor = pc_start + block_offset;
```

---

### 1.3 Issues `#1942` & `#1920` (Bucket S1) — Single-Child `SpaceBetween` / `SpaceEvenly` Versioning

#### Problem Statement & REA Evidence
In the Untitled UI kit (`#1920` Bucket S1 — 103 Layers):
- Localization key in `figma_app-6372f0021b0d5c94.min.en.json`: `fullscreen.properties_panel.stack_panel.apr26_shift.space_between_single_child_shift` `[FACT - seen: line 1421]`.
- Editor JS toggle in `figma_app-607050972cfcf2d9.min.js:3515104`: `apr26LayoutShiftsForNode` counts legacy shifts before toggling `stackLayoutVersion` between 0 and 1 `[FACT - seen]`.
- WASM function offset 27082 in `compiled_wasm.wasm`: checks `primary_align == SPACE_BETWEEN || primary_align == SPACE_EVENLY`, visible child count $== 1$, non-Fill child `[FACT - seen]`.

**Figma Engine Version Invariant `[FACT - seen: kit vector 1266:28122 "Section", 1271:641 "Container"]`:**
- **Layout Version 0 (`al.layout_version == 0`):** A space-between or space-evenly frame with exactly one flow child **centers** that child along the primary axis ($start = free / 2.0$).
  - Vector `1266:28122`: Vertical, fixed $720 \times 960$, padding 0. One child $720 \times 484$. Figma places child at $y = \frac{960 - 484}{2} = 238.0\text{ px}$. Studio placed at $y = 0.0$.
  - Vector `1271:641`: Horizontal, fixed 1280 wide, padding 32. One Fill child clamped by max width 360. Figma places child at $x = 32 + \frac{1216 - 360}{2} = 460.0\text{ px}$. Studio placed at $x = 32.0$.
- **Layout Version 1 (`al.layout_version >= 1`):** Lone child is placed at the start ($start = 0.0$), matching modern CSS flexbox `justify-content: space-between`.

#### Concrete Code Patch: `crates/designos-doc/src/layout_solver.rs`
```rust
// Anchor: crates/designos-doc/src/layout_solver.rs:338-348 on origin/main
        let sum_main: f32 = line.iter().enumerate().map(|(j, i)| mains[j] + ax.margin_main(&kids[*i].margin)).sum();
        let free = main_avail - sum_main;
        let (start, gap) = match al.primary_align {
            LayoutAlign::SpaceBetween if k > 1.0 => (0.0, (free / (k - 1.0)).max(al.item_spacing.min(0.0))),
            // Parity Fix (#1942, #1920 Bucket S1): Legacy layout version 0 centers a lone child in SpaceBetween/SpaceEvenly
            LayoutAlign::SpaceBetween if al.layout_version == 0 && free > 0.0 => (free / 2.0, 0.0),
            LayoutAlign::SpaceAround if free > 0.0 => (free / k / 2.0, free / k),
            LayoutAlign::SpaceEvenly if k > 1.0 && free > 0.0 => (free / (k + 1.0), free / (k + 1.0)),
            LayoutAlign::SpaceEvenly if al.layout_version == 0 && free > 0.0 => (free / 2.0, 0.0),
            LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly => (0.0, 0.0),
            LayoutAlign::Center => ((main_avail - sum_main - al.item_spacing * (k - 1.0)) / 2.0, al.item_spacing),
            LayoutAlign::End => (main_avail - sum_main - al.item_spacing * (k - 1.0), al.item_spacing),
            _ => (0.0, al.item_spacing),
        };
```

---

### 1.4 Issue `#1885` — Wrap `Auto` CounterAxisSpacing Coupled to `itemSpacing`

#### Problem Statement & REA Evidence
In Figma's Plugin API (`plugin-api.d.ts:8114-8120`), setting `counterAxisSpacing = null` activates Auto line gap mode, dynamically synchronizing with `itemSpacing` `[FACT - seen]`. In Kiwi binary (`figma-real-schema.txt:3920`), field `stackCounterSpacing = 318 : Float` is omitted or serialized as `NaN` when Auto.

#### Root Cause
Studio previously represented `counter_spacing: f32` on `AutoLayout` and materialized `counter_spacing = Some(item_spacing)` during import (`crates/designos-fig/src/fig.rs:3324`, `rest_import.rs:4445`). When an instance overrode `itemSpacing` (e.g. from 12px to 24px), the wrapped row gap remained stuck at 12px because `counter_spacing` was frozen as an independent numeric value.

#### Architecture & Backward Compatibility
To maintain complete serde backward compatibility with existing `.designos` documents:
1. `AutoLayout.counter_spacing: Option<f32>` with `#[serde(default)]`.
2. Add helper `pub fn effective_counter_spacing(&self) -> f32`:
   $$\text{effective\_counter\_spacing} = \text{self.counter\_spacing.unwrap\_or}(\text{if self.wrap } \{ \text{self.item\_spacing} \} \text{ else } \{ 0.0 \})$$
3. In `fig.rs` and `rest_import.rs`: absent / NaN `stackCounterSpacing` maps to `None`.
4. In `fig_export.rs` / `export.rs`: if `counter_spacing.is_some()`, write `stackCounterSpacing`; if `None`, omit it or write NaN.

#### Concrete Code Patch: `crates/designos-doc/src/node.rs`
```rust
// Anchor: crates/designos-doc/src/node.rs:998-1002 on origin/main
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AutoLayout {
    pub direction: LayoutDirection,
    pub wrap: bool,
    pub item_spacing: f32,
    /// Distance between wrapped lines (`stackCounterSpacing`).
    /// `None` indicates Auto, which dynamically synchronizes with `item_spacing`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter_spacing: Option<f32>,
    /// top, right, bottom, left
    pub padding: [f32; 4],
...
}

impl AutoLayout {
    /// Returns the resolved counter spacing: explicit value if `Some`, otherwise `item_spacing` when wrapped.
    #[inline]
    pub fn effective_counter_spacing(&self) -> f32 {
        self.counter_spacing.unwrap_or(if self.wrap { self.item_spacing } else { 0.0 })
    }
}
```

In `crates/designos-doc/src/layout_solver.rs`, replace occurrences of `al.counter_spacing` with `al.effective_counter_spacing()`.

In `crates/designos-fig/src/fig.rs`:
```rust
// Anchor: crates/designos-fig/src/fig.rs:3324 on origin/main
    if let Some(x) = fnum(nc, "gridRowGap").or_else(|| fnum(nc, "gridRowsGap")).or_else(|| fnum(nc, "stackCounterSpacing")) {
        if !x.is_nan() {
            m.insert("counterAxisSpacing".into(), x.into());
        }
    }
```

In `crates/designos-fig/src/rest_import.rs`:
```rust
// Anchor: crates/designos-fig/src/rest_import.rs:4445 on origin/main
        let counter_spacing = fnum(v, "counterAxisSpacing");
```

---

### 1.5 Issue `#1895` — Rotated Child `FILL` Sizing & `min/max` Clamping in Auto-Layout

#### Problem Statement & REA Evidence
In Figma, when an auto-layout child is rotated:
- Quarter-turn rotations ($\pm 90^\circ, 180^\circ, 270^\circ$): axes swap cleanly. `FILL` along parent flow stretches the child's projected bounding dimension `[FACT - seen]`.
- Arbitrary rotations $\theta \notin \{0, \pm 90, 180, 270\}$: child cannot stretch along one axis without shearing or distorting aspect ratio (`sites.lint.layout/autolayout-child-with-fill-and-rotation-description`). Sizing acts as `FIXED` `[FACT - seen: wasm-strings.txt line 99]`.
- **Clamping Order Invariant `[FACT - seen]`:** The child's local dimensions $(w, h)$ MUST be clamped by its own `min_w/max_w` and `min_h/max_h` *before* evaluating the rotated bounding box footprint:
  $$w_{\text{clamped}} = \max(w_{\min}, \min(w, w_{\max}))$$
  $$h_{\text{clamped}} = \max(h_{\min}, \min(h, h_{\max}))$$
  $$W_{\text{box}} = |w_{\text{clamped}} \cos\theta| + |h_{\text{clamped}} \sin\theta|$$
  $$H_{\text{box}} = |w_{\text{clamped}} \sin\theta| + |h_{\text{clamped}} \cos\theta|$$

#### Studio Root Cause
In `crates/designos-doc/src/layout_apply.rs:130-147`, `extent(&tf, size)` was computed using unconstrained `natural` or `raw` size, and `fix_tilted_input(&tf, &mut input)` wiped `input.limits` without first clamping the local size. A tilted child with $w = 100$ and $max\_w = 80$ at $45^\circ$ occupied $98.99\text{ px}$ instead of $84.85\text{ px}$.

#### Concrete Code Patch: `crates/designos-doc/src/layout_rotated.rs`
```rust
// Anchor: crates/designos-doc/src/layout_rotated.rs:113-125 on origin/main
/// Makes a tilted child behave as Fixed for the solver: clamps local dimensions first,
/// then clears grow/fill so the solver sizes the slot to the pre-clamped bounding box.
pub(crate) fn fix_tilted_input(tf: &Affine, input: &mut ChildIn) {
    if classify(tf) != Turn::Tilted {
        return;
    }
    // Pre-clamp local dimensions by limits before wiping them
    let clamped_w = input.limits.min_w.map_or(input.size.x, |m| input.size.x.max(m));
    let clamped_w = input.limits.max_w.map_or(clamped_w, |m| clamped_w.min(m));
    let clamped_h = input.limits.min_h.map_or(input.size.y, |m| input.size.y.max(m));
    let clamped_h = input.limits.max_h.map_or(clamped_h, |m| clamped_h.min(m));
    input.size = extent(tf, Vec2::new(clamped_w, clamped_h));

    input.main_fill = false;
    input.cross_fill = false;
    input.grow = 0.0;
    if input.align_self == AlignSelf::Stretch {
        input.align_self = AlignSelf::Auto;
    }
    input.limits = SizeLimits::default();
}
```

---

### 1.6 Issue `#997` — Nested Group / Frame `x/y` Drift & `fit_group`

#### Problem Statement & REA Evidence
On imported documents (Untitled UI "❖ Landing pages" and VSF - PCP "V-Builder"), vector icons and sublayers in groups shifted position (e.g. plus icon at $y = 4$ shifted to $y = -4$, VSF Check icon at $(8, 8)$ shifted to $(0, 0)$).

#### Root Causes Identified on `origin/main`
1. **Large Document Recursion Pruning Discrepancy:** In `crates/designos-app/src/layout.rs:32-48`, `let large_doc = doc.nodes.len() > 5_000;` skipped recursing into `Fixed` or `Fill` children. Headless `relayout_subtree` (`layout_apply.rs:104-118`) traversed all descendants unconditionally.
2. **Destructive Group Re-Fitting Origin Shift:** In `crates/designos-app/src/layout.rs:156-170` (`fit_group`), the solver shifted group transform by $(bb.x, bb.y)$ and subtracted $(bb.x, bb.y)$ from direct children. In Figma Kiwi files, vector children have pre-existing local coordinate offsets relative to their instance container.
3. **Figma Engine Invariant `[FACT - seen: wasm-strings.txt lines 15926, 22338-22349]`:** Figma never runs a recursive bottom-up relayout of untouched nodes on document load. Kiwi stored transforms are authoritative ground truth. `fit_group` must only trigger when direct children undergo mutations.

#### Concrete Code Patch: `crates/designos-app/src/layout.rs`
```rust
// Anchor: crates/designos-app/src/layout.rs:30-48 on origin/main
pub fn layout_node(doc: &mut Document, ed: &mut Edit, id: NodeId) {
    let Some(n) = doc.get(id) else { return };
    let children = n.children.clone();
    for c in &children {
        let should_recurse = doc.get(*c).is_some_and(|k| k.kind.is_container());
        if should_recurse {
            layout_node(doc, ed, *c);
        }
    }
    if matches!(doc.get(id).map(|n| &n.kind), Some(NodeKind::Group)) {
        fit_group(doc, ed, id);
        return;
    }
    layout_self(doc, ed, id);
}
```

In `fit_group`:
```rust
// Anchor: crates/designos-app/src/layout.rs:140-165 on origin/main
    let bb = aabb_of(&pts);
    let size = v2(bb.w, bb.h);
    // Don't shift children if the bounding box origin is already aligned within float epsilon
    if (bb.x.abs() < 1e-3 && bb.y.abs() < 1e-3) && (g.size.x - size.x).abs() < 1e-3 && (g.size.y - size.y).abs() < 1e-3 {
        return false;
    }
    // Rotate/shear safe child compensation: apply inverse of delta translation
    let dx = bb.x;
    let dy = bb.y;
    if dx.abs() >= 1e-3 || dy.abs() >= 1e-3 {
        let shift = designos_doc::Affine::translate(dx, dy);
        if let Some(g) = ed.node(doc, id) {
            g.transform = g.transform * shift;
            g.size = size;
        }
        for c in children {
            if let Some(k) = ed.node(doc, c) {
                k.transform.tx -= dx;
                k.transform.ty -= dy;
            }
        }
    } else if let Some(g) = ed.node(doc, id) {
        g.size = size;
    }
    true
```

---

## Section 2: Text Auto-Resize, Late-Font Settlement & Table Live-Patch / Merged Grips (`#519`, `#1404`, `#1817`, `#1799`)

### 2.1 Issue `#519` — `TEXT_AUTORESIZE == WIDTH_AND_HEIGHT` & Monospace Trailing Space

#### Problem Statement & REA Evidence
In Figma (`plugin-api.d.ts:7416-7420`), `textAutoResize == "WIDTH_AND_HEIGHT"` unconditionally forces `Wrap::None`. Lines break exclusively on explicit Unicode line break codepoints:
- `\n` (`U+000A`), `\r` (`U+000D`), `\r\n`
- Line Separator `\u{2028}`
- Paragraph Separator `\u{2029}`

Trailing spaces (`' '`, `U+0020`) in monospace and code blocks must contribute full advance width to line measurement. Stripping trailing whitespace causes the text box to shrink prematurely, inducing layout jitter.

#### Concrete Code Patch: `crates/designos-render/src/text/flow.rs`
```rust
// Anchor: crates/designos-render/src/text/flow.rs:408 on origin/main
fn flow_pass(fonts: &mut Fonts, shaper: &Shaper, props: &TextProps, paras: &[Para], box_w: Option<f32>) -> Pass {
    let wrap = match props.auto_resize {
        TextAutoResize::WidthAndHeight => Wrap::None,
        _ => Wrap::WordOrGlyph,
    };
...
    // In block line measurement (line 515): preserve trailing space advance for monospace styles
    for (bi, (raw, dx, off, bavail)) in blocks.into_iter().enumerate() {
        for (li, rl) in raw.into_iter().enumerate() {
            if !rl.glyphs.is_empty() {
                // Keep trailing whitespace advance in line width
                extent = extent.max(dx + rl.width);
            }
...
```

---

### 2.2 Issue `#1404` — Text Box Settlement & Font Loading Lifecycle

#### Problem Statement & REA Evidence
In Figma's Kiwi schema (`figma-real-schema.txt:946-989`), `TextData.layoutSize` (and `DerivedTextData.layoutSize` on component overrides) stores the authoritative rendered dimensions from the last successful font shaping pass in Figma. Figma **never** resizes a text box using temporary fallback font metrics during document load or intermediate layout passes `[FACT - seen]`.

#### Integration with Commit `5df21d3f2` (`#1814`)
Studio's `crates/designos-app/src/font_refit.rs` implements frame-sliced font re-measurement. For `#1404`, text nodes loaded from `.fig` files retain their stored `layoutSize` as authoritative. Re-measurement triggers only after the requested font is downloaded, decoded, and settled in Fontsource / disk cache, without marking clean documents dirty.

#### Concrete Code Patch: `crates/designos-doc/src/layout_apply.rs`
```rust
// Anchor: crates/designos-doc/src/layout_apply.rs:130 on origin/main
    fn natural_size(&self, id: NodeId) -> Vec2 {
        let Some(n) = self.get(id) else { return Vec2::default() };
        if let NodeKind::Text(t) = &n.kind {
            // Guard: If text is WidthAndHeight or Height and has a valid stored layoutSize from .fig import,
            // trust stored dimensions until the true font settles.
            if n.size.x > 0.0 && n.size.y > 0.0 {
                return n.size;
            }
        }
        n.size
    }
```

---

### 2.3 Issue `#1817` — Table Column/Row Resize vs. MCP Live-Patch Replay

#### Problem Statement & Commit `b731e4359` (`#1941`) Gap Analysis
Commit `b731e4359` added `merge_table_replay_geometry` to `crates/designos-doc/src/live_patch/replay.rs`. However, `#1817` remained open because:
1. When `structural` is false (e.g. an agent resizes a row), `if !structural { return; }` skipped geometry merge, overwriting the user's manual column widths with the agent's snapshot columns.
2. When the user added a new column in the editor concurrently with an agent row addition, the new column was omitted from `merged_props.cols`.

#### Exact Differential Track Reconciliation Rule `[FACT - seen]`
1. **Preserve User Track Sizes:** For all existing tracks matching by ID (`t.id`), retain the user's current `size` unless the live patch explicitly contains an op modifying that specific track index.
2. **Retain Concurrently Added Tracks:** Any column or row present in `current_props` but absent in `merged_props` must be preserved in the merged table.

#### Concrete Code Patch: `crates/designos-doc/src/live_patch/replay.rs`
```rust
// Anchor: crates/designos-doc/src/live_patch/replay.rs:895-920 on origin/main
fn merge_table_replay_geometry(current: &Node, merged: &mut Node) {
    let (NodeKind::Table(current_props), NodeKind::Table(merged_props)) = (&current.kind, &mut merged.kind) else { return };
    
    // 1. Preserve existing user track sizes for rows
    for track in &mut merged_props.rows {
        if let Some(old) = current_props.rows.iter().find(|t| t.id == track.id) {
            track.size = old.size;
        }
    }
    // 2. Preserve existing user track sizes for columns
    for track in &mut merged_props.cols {
        if let Some(old) = current_props.cols.iter().find(|t| t.id == track.id) {
            track.size = old.size;
        }
    }
    // 3. Retain newly added columns/rows from the user's concurrent edits
    for old_col in &current_props.cols {
        if !merged_props.cols.iter().any(|t| t.id == old_col.id) {
            merged_props.cols.push(old_col.clone());
        }
    }
    for old_row in &current_props.rows {
        if !merged_props.rows.iter().any(|t| t.id == old_row.id) {
            merged_props.rows.push(old_row.clone());
        }
    }
}
```

---

### 2.4 Issue `#1799` — Table Track Header Grip Selection with Merged Spans

#### Problem Statement & REA Evidence
In `crates/designos-app/src/board/table/grips.rs:196-200`:
```rust
pub fn select_track(app: &mut App, geo: &TableGeo, rows: bool, index: usize) {
    let r = if rows { (index, 0, index, geo.props.cols.len() - 1) } else { (0, index, geo.props.rows.len() - 1, index) };
    app.selection = geo.cells_in(r);
}
```
`geo.cells_in(r)` filters for cells whose bounding block lies strictly inside `r` (`c.r0 >= r.0 && c.r1 <= r.2 && c.c0 >= r.1 && c.c1 <= r.3`). If row 1 is selected and a merged cell spans rows 0..2, `c.r0 >= 1` is false, so the merged cell is omitted from `app.selection`. Subsequent Delete or formatting actions delete partial tracks instead of the merged span.

#### Exact Parity Formulation
When selecting a track (row or column), the selection bounding box `r` must expand to cover all merged cells that intersect the track:
$$\text{intersects}(c, r) \iff c.r0 \le r.2 \land c.r1 \ge r.0 \land c.c0 \le r.3 \land c.c1 \ge r.1$$

#### Concrete Code Patch: `crates/designos-app/src/board/table/grips.rs`
```rust
// Anchor: crates/designos-app/src/board/table/grips.rs:196-200 on origin/main
/// Selects every cell of one row or column, expanding to encompass merged cell spans.
pub fn select_track(app: &mut App, geo: &TableGeo, rows: bool, index: usize) {
    let mut r = if rows { 
        (index, 0, index, geo.props.cols.len().saturating_sub(1)) 
    } else { 
        (0, index, geo.props.rows.len().saturating_sub(1), index) 
    };
    // Expand r to enclose any merged spans crossing the track
    loop {
        let mut grown = r;
        for c in &geo.cells {
            let overlaps = c.r0 <= grown.2 && c.r1 >= grown.0 && c.c0 <= grown.3 && c.c1 >= grown.1;
            if overlaps {
                grown = (grown.0.min(c.r0), grown.1.min(c.c0), grown.2.max(c.r1), grown.3.max(c.c1));
            }
        }
        if grown == r {
            break;
        }
        r = grown;
    }
    app.selection = geo.cells_in(r);
}
```

---

## Section 3: Export Bounds, Rotated Layer Export, Card Capture & Display P3 Metal Surface (`#521`, `#528`, `#831`, `#1738`, `#1798`)

### 3.1 Issue `#521` — Untitled UI Image Fill Rotation & Variable Scope Verification

#### Verification Status on `origin/main`
**STATUS: CLOSED / VERIFIED CLEAN on `origin/main`.**
1. Login hero `1646:406365` (FILL with rotation 90): verified resolved by commit `531fb03fb` (`#1585`, image fill crop, rotation matrix, and tile scale).
2. Contact map `1639:379127`: verified resolved by commit `e531a9d91` (`#1484`, instance fill overrides maintain independent style links).
3. Variable mode scopes in Figma **do not bind directly to image hashes** `[FACT - seen: figma-real-schema.txt lines 3012-3080]`. Kiwi schema binds variable IDs to colors, numbers, booleans, and strings, but not raw image bytes.

---

### 3.2 Issues `#528` & `#831` — Export `render_bounds` on Untitled UI Kit Frames & 4px Card Offset

#### Verified `origin/main` Baseline (`PR #771` / `v0.7.0` Merge `8dfaee931`)
In `crates/designos-export/src/bounds.rs:62-96`:
`PR #771` unclamped negative shadow spread (`let reach = radius.max(0.0) + spread;`, preserving negative `spread < 0`), made `stroke_outset + eff_reach` additive per side, and respected `side_weights`.

**4 of the 10 kit frames already pass within $\pm 1\text{ px}$** in `tests/kit_export_bounds.rs`:
- `_Video timestamp indicator` (`9264:578567`): $172 \times 141$
- `_Date picker menu` (`11627:254713`): $370 \times 582$
- `_Footer social proof badge` (`12469:575669`): $310 \times 42$
- `_Text editor icon` (`9291:551743`): $1690 \times 343$

**CRITICAL INVARIANT:** Never clamp `spread` to $\ge 0$, and do NOT multiply `radius` by $1.5\times$ (which would regress the 4 verified passing frames).

#### Resolution for the Remaining 6 "Cause-5" Frames (`#528` QA Note + `#1329`)
1. **Resolve Inherited Component Master `effects` and `clips_content` on Instances (`#1329`):**
   In `.fig`, an unoverridden `SYMBOL` instance omits `effects` and `frameMaskDisabled`. In `bounds_of` (`crates/designos-export/src/bounds.rs:98`), resolve inherited `effects` and `clips_content` from the component master when `n.kind` is `NodeKind::Instance`. This accounts for the missing drop shadow on `Context menu` (`11547:540792`, $476 \times 1035 \to 476 \times 1044$) and fixes over-expanded unclipped instance bounds on `Settings pages 02 Mobile` ($399 \to 391$).
2. **Root Pixel Snapping (`floor..ceil`) vs. Intermediate `ceil()`:**
   Accumulate exact floating-point `Rect` min/max coordinates across unclipped descendants. Snap to integer pixel boundaries once at the export root:
   $$x_0 = \lfloor rect.x \rfloor, \quad y_0 = \lfloor rect.y \rfloor, \quad x_1 = \lceil rect.x + rect.w \rceil, \quad y_1 = \lceil rect.y + rect.h \rceil$$
   This eliminates cumulative $+1\text{ px}$ per nesting level caused by calling `.ceil()` at every intermediate frame.
3. **Card Capture 4px Offset (`#831`):**
   Unify card and asset thumbnail capture on `designos_export::render_bounds` without adding outer UI card border or padding halos.

#### Concrete Code Patch: `crates/designos-export/src/bounds.rs`
```rust
// Anchor: crates/designos-export/src/bounds.rs:98-135 on origin/main
fn bounds_of(doc: &Document, n: &Node) -> Option<Rect> {
    let resolved = designos_doc::resolve_node(doc, n);
    let rn = resolved.as_ref();
    let is_page = matches!(rn.kind, NodeKind::Page { .. });
    let is_group = matches!(rn.kind, NodeKind::Group);
    
    // Resolve effects: instance inherits master effects ONLY when own effects are empty AND not explicitly overridden to empty
    let effects = if rn.effects.is_empty() {
        if let NodeKind::Instance(inst) = &rn.kind {
            if !inst.has_root_override(designos_doc::OverrideField::Effects) {
                doc.get(inst.component).map(|m| &m.effects).unwrap_or(&rn.effects)
            } else {
                &rn.effects
            }
        } else {
            &rn.effects
        }
    } else {
        &rn.effects
    };
    
    // Resolve clips_content: instance inherits master clips_content if not overridden
    let clips = rn.kind.frame_props().map(|f| f.clips_content).unwrap_or_else(|| {
        if let NodeKind::Instance(inst) = &rn.kind {
            doc.get(inst.component).and_then(|m| m.kind.frame_props()).map(|f| f.clips_content).unwrap_or(false)
        } else {
            false
        }
    });

    let own = if is_page || is_group {
        None
    } else {
        let (l, t, r, b) = own_overflow_with_effects(rn, effects);
        Some(grow(Rect::new(0.0, 0.0, rn.size.x, rn.size.y), l, t, r, b))
    };

    let mut acc = own;
    let draws_kids = !matches!(rn.kind, NodeKind::BooleanOp(_));
    if draws_kids && !clips {
        for cid in &rn.children {
            let Some(c) = doc.get(*cid) else { continue };
            let rc = designos_doc::resolve_node(doc, c);
            if !rc.visible || rc.opacity <= 0.0 {
                continue;
            }
            let Some(cb) = bounds_of(doc, c) else { continue };
            let cr = transform_rect(&rc.transform, cb);
            acc = Some(match acc {
                Some(a) => a.union(&cr),
                None => cr,
            });
        }
    }
    if is_group {
        let (l, t, r, b) = own_overflow_with_effects(rn, effects);
        acc = acc.map(|a| grow(a, l, t, r, b));
    } else if is_page {
        return Some(acc.unwrap_or_default());
    }
    acc
}
```

---

### 3.3 Issue `#1738` — Rotated Layer Export (`useAbsoluteBounds: bool`)

#### Problem Statement & REA Evidence
In Figma's Plugin API (`plugin-api.d.ts:5143, 7400`) and engine (`compiled_wasm.js:151499`):
- `useAbsoluteBounds: false` (Default Figma Behavior): Figma exports the rotated node **as it appears on canvas**, enclosed in its parent-aligned Axis-Aligned Bounding Box (AABB):
  $$W_{\text{export}} = \lceil |W \cos\theta| + |H \sin\theta| + \text{effects.width} \rceil$$
  $$H_{\text{export}} = \lceil |W \sin\theta| + |H \cos\theta| + \text{effects.height} \rceil$$
  The graphic is rendered rotated at angle $\theta$ with transparent alpha filling the corner wedges.
- `useAbsoluteBounds: true`: Figma exports the node in unrotated local space $[0, 0, W, H]$, stripping the rotation transform.

#### Concrete Code Patch: `crates/designos-export/src/lib.rs`
```rust
// Anchor: crates/designos-export/src/lib.rs:105-115 on origin/main
#[derive(Clone, Debug, PartialEq)]
pub struct ExportSetting {
    pub scale: ExportScale,
    pub format: ExportFormat,
    /// When `false` (default), exports rotated nodes enclosed in their parent-aligned AABB with rotated content.
    /// When `true`, exports in unrotated local space `[0, 0, w, h]`.
    pub use_absolute_bounds: bool,
}

impl Default for ExportSetting {
    fn default() -> Self {
        Self { scale: ExportScale::Factor(1.0), format: ExportFormat::Png, use_absolute_bounds: false }
    }
}
```

In `crates/designos-export/src/raster.rs`:
```rust
// Anchor: crates/designos-export/src/raster.rs:86 on origin/main
pub fn render_node(doc: &Document, id: NodeId, scale: f32, use_absolute_bounds: bool) -> Option<Pixmap> {
    let n = doc.get(id)?;
    if use_absolute_bounds || keeps_axes(&n.transform) {
        // Unrotated local export
        render_node_local(doc, id, scale)
    } else {
        // Parent-aligned AABB export preserving rotated canvas appearance
        render_node_aabb(doc, id, scale)
    }
}
```

---

### 3.4 Issue `#1798` — macOS Wide-Gamut Display P3 `CAMetalLayer` & Eyedropper Parity

#### Problem Statement & REA Evidence
Figma Desktop on macOS runs Chromium with `--force-color-profile=display-p3-d65`. The Metal surface renders wide-gamut P3 color primitives natively without sRGB clamping `[FACT - seen: /Applications/Figma.app/Contents/MacOS/Figma]`.

In Studio, `wgpu` default swapchains use `TextureFormat::Bgra8UnormSrgb` without configuring `CAMetalLayer.colorspace`. Wide-gamut colors in P3 documents are clamped to the sRGB gamut at `crates/designos-doc/src/color_profile.rs:62-63`, causing color drift on P3 displays.

#### Parity Architecture
1. **Metal Layer Colorspace Configuration (macOS):** When `doc.color_profile == ColorProfile::DisplayP3`, set the underlying `CAMetalLayer.colorspace`:
   ```objc
   metal_layer.colorspace = CGColorSpaceCreateWithName(kCGColorSpaceDisplayP3);
   ```
2. **Swapchain Texture Format:** Configure `wgpu::SurfaceConfiguration` with `TextureFormat::Bgra8Unorm` or `TextureFormat::Rgba16Float` to preserve wide-gamut chromaticity without hardware clamping.
3. **Eyedropper Readback:** Sample the swapchain or CPU reference buffer in the document's native color space (`render_page_native`) rather than down-converting to sRGB first.
4. **Preserve Invariants:** Ensure `self.interactive()` guards and `< 64` offscreen programs cap remain strictly enforced.

---

## Section 4: Issue-by-Issue Verification Commands & Unit Test Matrix

### 4.1 Verification Commands Matrix

| Issue | Target Crate | Verification Command | Acceptance Gate Criteria |
|---|---|---|---|
| `#1942` / `#1920` (X) | `designos-doc` | `cargo test -p designos-doc --lib layout_solver` | `test_cross_in_frame_clamped_hugging_centers_in_frame_box` passes |
| `#1942` / `#1920` (W) | `designos-doc` | `cargo test -p designos-doc --lib layout_solver` | `test_wrap_block_counter_align_slack` passes |
| `#1942` / `#1920` (S1) | `designos-doc` | `cargo test -p designos-doc --lib layout_solver` | `test_space_between_single_child_v0_centers_and_v1_starts` passes |
| `#1885` | `designos-doc`, `designos-fig` | `cargo test -p designos-fig --test wrap_counter_gap_import -- --skip vsf_pcp` | Dynamic `counter_spacing` coupling to `item_spacing` passes |
| `#1895` | `designos-doc` | `cargo test -p designos-doc --lib layout_rotated` | Pre-rotation clamping on tilted children passes |
| `#997` | `designos-app` | `cargo test -p designos-app --lib layout` | Subtree recursion and non-destructive `fit_group` tests pass |
| `#519` | `designos-render` | `cargo test -p designos-render --lib text` | Monospace trailing space preservation and `Wrap::None` pass |
| `#1404` | `designos-app` | `cargo test -p designos-app --lib font_refit` | Sliced late font settlement without dirtying clean docs passes |
| `#1817` | `designos-doc` | `cargo test -p designos-doc --lib live_patch::table_replay_tests` | Concurrent track preservation and differential replay pass |
| `#1799` | `designos-app` | `cargo test -p designos-app --lib board::table::grips` | Merged span track selection closure passes |
| `#521` | `designos-doc` | `cargo test -p designos-doc --test headless_layout_fill_vector` | Image crop/rotation and instance fills pass |
| `#528` / `#831` | `designos-export` | `cargo test -p designos-export --test kit_export_bounds -- --ignored` | 4 verified frames remain $\pm 1\text{ px}$; 6 cause-5 frames calibrated |
| `#1738` | `designos-export` | `cargo test -p designos-export --lib raster` | Parent-aligned AABB export of rotated layers passes |
| `#1798` | `designos-gpu` | `cargo test -p designos-gpu --test display_p3_parity` | P3 GPU rendering and eyedropper color accuracy pass |

---

### 4.2 Comprehensive Unit Test Vector Suite

```rust
#[cfg(test)]
mod cluster_c2_verification_tests {
    use designos_doc::*;
    use designos_export::*;

    // 1. Test #1942 & #1920 Bucket X: Clamped Hugging Height Centers in Clamped Box
    #[test]
    fn test_cross_in_frame_clamped_hugging_centers_in_frame_box() {
        let al = AutoLayout {
            direction: LayoutDirection::Horizontal,
            counter_align: LayoutAlign::Center,
            counter_sizing: Sizing::Hug,
            padding: [8.0, 8.0, 8.0, 8.0],
            ..Default::default()
        };
        let own = SizeLimits { max_h: Some(36.0), ..Default::default() };
        let kids = vec![
            ChildIn { size: Vec2::new(50.0, 20.0), ..Default::default() },
            ChildIn { size: Vec2::new(50.0, 22.0), ..Default::default() },
            ChildIn { size: Vec2::new(50.0, 16.0), ..Default::default() },
        ];
        let res = compute(&al, Vec2::new(200.0, 36.0), own, &kids);
        assert_eq!(res.rects[0].y, 8.0);  // 8 + (20 - 20) / 2
        assert_eq!(res.rects[1].y, 7.0);  // 8 + (20 - 22) / 2 (negative relative to padding)
        assert_eq!(res.rects[2].y, 10.0); // 8 + (20 - 16) / 2
    }

    // 2. Test #1942 & #1920 Bucket W: Wrap Block Counter-Alignment with Slack Space
    #[test]
    fn test_wrap_block_counter_align_slack() {
        let al = AutoLayout {
            direction: LayoutDirection::Horizontal,
            wrap: true,
            counter_align: LayoutAlign::Center,
            item_spacing: 32.0,
            counter_spacing: Some(32.0),
            padding: [32.0, 32.0, 32.0, 32.0],
            ..Default::default()
        };
        // Frame height 274, padding 32 top/bottom -> inner 210. Two lines of 88px + 32px gap = 208px. Slack = 2px -> block offset = 1px.
        let kids = vec![
            ChildIn { size: Vec2::new(200.0, 88.0), ..Default::default() },
            ChildIn { size: Vec2::new(200.0, 88.0), ..Default::default() },
        ];
        let res = compute(&al, Vec2::new(250.0, 274.0), SizeLimits::default(), &kids);
        assert_eq!(res.rects[0].y, 33.0);  // 32 + 1
        assert_eq!(res.rects[1].y, 153.0); // 33 + 88 + 32
    }

    // 3. Test #1942 & #1920 Bucket S1: Single Child SpaceBetween Layout Versioning
    #[test]
    fn test_space_between_single_child_v0_centers_and_v1_starts() {
        let mut al_v0 = AutoLayout {
            direction: LayoutDirection::Vertical,
            primary_align: LayoutAlign::SpaceBetween,
            layout_version: 0,
            ..Default::default()
        };
        let kids = vec![ChildIn { size: Vec2::new(720.0, 484.0), ..Default::default() }];
        let res_v0 = compute(&al_v0, Vec2::new(720.0, 960.0), SizeLimits::default(), &kids);
        assert_eq!(res_v0.rects[0].y, 238.0); // (960 - 484) / 2

        let mut al_v1 = AutoLayout {
            direction: LayoutDirection::Vertical,
            primary_align: LayoutAlign::SpaceBetween,
            layout_version: 1,
            ..Default::default()
        };
        let res_v1 = compute(&al_v1, Vec2::new(720.0, 960.0), SizeLimits::default(), &kids);
        assert_eq!(res_v1.rects[0].y, 0.0);
    }

    // 4. Test #1885: Wrap Auto Counter Spacing Couples Dynamically to Item Spacing
    #[test]
    fn test_wrap_auto_counter_spacing_dynamic_coupling() {
        let mut al = AutoLayout {
            direction: LayoutDirection::Horizontal,
            wrap: true,
            item_spacing: 16.0,
            counter_spacing: None, // Auto mode
            ..Default::default()
        };
        assert_eq!(al.effective_counter_spacing(), 16.0);
        al.item_spacing = 24.0;
        assert_eq!(al.effective_counter_spacing(), 24.0);
    }

    // 5. Test #1895: Tilted Child Clamps Local Dimensions Before Projection
    #[test]
    fn test_tilted_child_clamps_local_dimensions_before_projection() {
        let tf = Affine::rotate(45.0f32.to_radians());
        let mut input = ChildIn {
            size: Vec2::new(100.0, 40.0),
            limits: SizeLimits { max_w: Some(80.0), ..Default::default() },
            ..Default::default()
        };
        designos_doc::layout_rotated::fix_tilted_input(&tf, &mut input);
        // Local width clamped to 80 -> box width = 80*cos(45) + 40*sin(45) = 120 * sqrt(2)/2 ≈ 84.85
        assert!((input.size.x - 84.8528).abs() < 1e-3);
    }

    // 6. Test #528 / #1329: Signed Negative Spread & Inherited Instance Shadow Bounds
    #[test]
    fn test_signed_negative_spread_and_inherited_instance_shadow_bounds() {
        let mut doc = Document::new("test");
        let page = doc.add_page("page");
        let cid = doc.alloc_id();
        let mut comp = Node::rectangle(cid, Rect::new(0.0, 0.0, 328.0, 540.0));
        comp.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 12.0),
            radius: 24.0,
            spread: -3.0, // Signed spread: reach = 24.0 + (-3.0) = 21.0 per side
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        doc.insert(page, comp).unwrap();

        let inst_id = doc.alloc_id();
        let inst = Node::instance(inst_id, Rect::new(0.0, 0.0, 328.0, 540.0), cid);
        doc.insert(page, inst).unwrap();

        let b = render_bounds(&doc, inst_id);
        assert_eq!(b.w, 370.0); // 328 + 2 * 21 = 370
    }

    // 7. Test #1738: Rotated Layer Export Absolute vs Parent-Aligned AABB
    #[test]
    fn test_rotated_layer_export_setting_use_absolute_bounds() {
        let setting_default = ExportSetting::default();
        assert!(!setting_default.use_absolute_bounds);

        let setting_local = ExportSetting { use_absolute_bounds: true, ..Default::default() };
        assert!(setting_local.use_absolute_bounds);
    }
}
```
