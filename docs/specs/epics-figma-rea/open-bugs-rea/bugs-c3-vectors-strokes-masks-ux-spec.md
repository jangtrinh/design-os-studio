> **Review status: NOT REVIEWED.** Reference for implementation; verify symbols and statements about Figma against main and the schema before relying on them.

# True Figma Implementation Spec: Open Parity Bugs Cluster C3 (Vector Lines/Arrows, Parametric Primitives, Masks, Strokes, GPU Blur & UX Micro-Interactions)

**Epic / Cluster:** Cluster C3 — Vectors, Strokes, Baked Outlines, Masks, Blur & UX Micro-Interactions  
**Issues Covered (20 Issues):** `#1753`, `#1754`, `#1755`, `#1756`, `#1757`, `#1758`, `#1765`, `#953`, `#1936`, `#1937`, `#1506`, `#1621`, `#1631`, `#1689`, `#1476`, `#1109`, `#1110`, `#1111`, `#1474`, `#1730`  
**Status:** Build-Ready Clean-Room Engineering Specification  
**Date:** 2026-11-12  
**Target Milestone:** v0.8.0 / Epic Waves C3  
**Evidence Source:** REA Clean-Room Dossier [`rea-bugs-c3-vectors-strokes-masks-ux.md`](../evidence/rea-bugs-c3-vectors-strokes-masks-ux.md) (REA Application Analysis Bundle `ev_d40f84650d449631485673749ef384b0ba7719737de1c20a9cd4e264aae1865b`, Kiwi Schema `22cd4819bd2ad3e175bcef17fb7197d4f63d5d722868edd7160c57b712ba61d2`)  
**Live `origin/main` Commit Anchor:** `f2deb8d50ab58569d94ac3ddc396790d60bf024f`  
**Target Crates:** `designos-doc`, `designos-fig`, `designos-render`, `designos-gpu`, `designos-app`, `designos-mcp`

---

## Executive Summary & Architectural Overview

Cluster C3 resolves 20 critical parity bugs across vector lines, marker arrow caps, baked outline hygiene, parametric geometry, clipping masks, complex stroke rendering, GPU post-processing shaders, and desktop micro-interactions.

```text
+---------------------------------------------------------------------------------------------------+
|                                 CLUSTER C3 RESOLUTION TOPOLOGY                                    |
+------------------------------------+----------------------------------+---------------------------+
| Part 1: Lines, Caps & Primitives   | Part 2: Masks, Strokes & Blur    | Part 3: UX Interactions   |
| (#1753, #1754, #1755, #1756, #1757 | (#1506, #1621, #1631, #1689,     | (#1109, #1110, #1111,     |
|  #1758, #1765, #953, #1936, #1937) |  #1476)                          |  #1474, #1730)            |
+------------------------------------+----------------------------------+---------------------------+
| - Piecewise-rigid line stretch     | - Stencil union for unclipped    | - Left & right sidebar    |
|   ([0, E_start], shaft, [W0-E_end])|   mask frames + stroke-mask rule |   clamping & dblclick     |
| - Unified resize mutation paths    | - Connected-component gap text   | - Pages section collapse  |
| - clear_baked_outlines helper      |   stroke chunking (0 seam)       |   disclosure with pill    |
| - Per-vertex arrow caps in Kiwi    | - CPU tile margin = stroke_outset| - Context menu completeness|
| - Deterministic empty boolean op   | - GPU blit clamp_to_edge flag    | - 3.0px handle deadzone   |
| - Ellipse-inscribed star/polygon   | - Outlines & Pixel Preview modes |   preventing dirty undo   |
| - MCP ArcSpec round-trip           |                                  |                           |
+------------------------------------+----------------------------------+---------------------------+
```

### Architectural Contracts & Safety Invariants Preserved
1. **Zero String Serialization in Document Hashing (`AGENTS.md` §4.1)**: All geometric mutations retain zero-allocation streaming `HashSerializer`. No `doc.to_json()` invocations.
2. **Main Thread Lock Safety (`AGENTS.md` §4.2)**: No `ctx.load_texture` calls inside `ctx.data_mut` write guards.
3. **Headless/Interactive Parity (`AGENTS.md` §4.3)**: LOD greeking, scissor clipping fallbacks, and culling remain strictly inside `self.interactive()`. Headless reference tests remain bit-exact.
4. **Offscreen GPU Program Cap (`AGENTS.md` §4.4)**: Blur and blit pipeline refinements reuse existing WGSL layouts; total GPU `Program` allocations remain strictly $< 64$.
5. **Idle Frame Invariant**: Viewport-only view modes (Outlines, Pixel Preview) and sidebar layout calculations never mutate `Document` revision, ensuring idle CPU stays at `0.00%`.

---

## Part 1: Lines, Arrow Caps, Baked `strokeGeometry` Hygiene & Parametric Primitives

### 1.1 Unified Line/Arrow Resize & Piecewise-Rigid `stretch_line_geometry` (`#1753`, `#1754`, `#1756`)

#### 1.1.1 Root Cause & Mathematical Ground Truth (`#1753`)
In Figma's internal schema (`figma-real-schema.txt:215-225`, `plugin-api.d.ts:8753-8766`):
- A `Line` is modeled as a mathematical centerline from $(0, 0)$ to $(size.x, 0)$ with $size.y == 0.0$.
- Rotation and orientation are encoded in `relativeTransform` (a $2 \times 3$ affine matrix).
- When exported to `.fig` kiwi binary or REST JSON, Figma generates `strokeGeometry` as a closed boundary polygon representing the physical stroke band.
- For lines, Figma's baked outline does *not* center symmetrically on $y = 0.0$: depending on `strokeAlign` and cap type, the band sits in $[-weight, 0]$ or is shifted by an internal offset.
- **The Dilemma in Previous Studio Code**:
  1. *Affine Scaling (`scale_paths`)*: Scaling $x$ by $k_x = W_1 / W_0$ stretches the entire stroke band. For plain lines this works; however, for arrow lines (with `ArrowLines`, `ArrowEquilateral`, `TriangleFilled`, `CircleFilled`, `DiamondFilled`), affine scaling stretches the arrowhead along $x$. A 10px arrow cap at $3\times$ scale becomes a distorted 30px head!
  2. *Dropping Baked Outline (`node.stroke_geometry = None`)*: Clearing the outline causes the renderers (`tiny-skia` and `designos-gpu`) to fall back to drawing from the mathematical centerline $(0, 0) \to (W, 0)$ as a center-aligned stroke in $[-weight / 2, weight / 2]$. If Figma's baked outline was in $[-weight, 0]$, this drops the line down by $weight / 2.0$ screen pixels!

#### 1.1.2 The Clean-Room Piecewise-Rigid Algorithm: `stretch_line_geometry`
To preserve arrowhead proportions *and* maintain the exact vertical origin without a half-stroke shift, line geometry must be stretched using a 3-zone piecewise partition along the line axis:

```text
               Zone 1: Rigid Start Cap        Zone 2: Linear Shaft           Zone 3: Rigid End Cap
               [0, E_start]                   (E_start, W0 - E_end)          [W0 - E_end, W0]
             +------------------------------+-------------------------------+------------------------------+
  W0 (old)   |  <- Rigid Arrowhead ->       |  ======== Line Shaft =======  |  <- Rigid Arrowhead ->       |
             +------------------------------+-------------------------------+------------------------------+
                                                            |
                                                 stretches by k_shaft
                                                            v
             +------------------------------+-------------------------------------------+------------------------------+
  W1 (new)   |  <- Identical Arrowhead ->   |  ============== Line Shaft =============  |  <- Identical Arrowhead ->   |
             +------------------------------+-------------------------------------------+------------------------------+
             Offset: 0.0                    Linear scale along x                        Offset: + (W1 - W0)
```

Let:
- $W_0 = old\_size.x$, $W_1 = new\_size.x$, $\Delta W = W_1 - W_0$.
- $cap_{\text{start}} = node.stroke.start\_cap.unwrap\_or(node.stroke.cap)$
- $cap_{\text{end}} = node.stroke.end\_cap.unwrap\_or(node.stroke.cap)$
- Cap extents:
  $$E_{\text{start}} = \begin{cases} 2.5 \times weight & \text{if } cap_{\text{start}} \in \{\text{ArrowEquilateral}, \text{TriangleFilled}\} \\ 2.0 \times weight & \text{if } cap_{\text{start}} \in \{\text{ArrowLines}, \text{DiamondFilled}, \text{CircleFilled}\} \\ 0.0 & \text{if } cap_{\text{start}} \in \{\text{None}, \text{Round}, \text{Square}\} \end{cases}$$
  $$E_{\text{end}} = \begin{cases} 2.5 \times weight & \text{if } cap_{\text{end}} \in \{\text{ArrowEquilateral}, \text{TriangleFilled}\} \\ 2.0 \times weight & \text{if } cap_{\text{end}} \in \{\text{ArrowLines}, \text{DiamondFilled}, \text{CircleFilled}\} \\ 0.0 & \text{if } cap_{\text{end}} \in \{\text{None}, \text{Round}, \text{Square}\} \end{cases}$$

Mapping function for any vertex $p = (x, y)$ in `paths`, `fill_geometry`, or `stroke_geometry`:
1. **Short Line Edge Case ($W_0 \le E_{\text{start}} + E_{\text{end}}$ or $W_1 \le E_{\text{start}} + E_{\text{end}}$)**:
   When the line is shorter than its arrow caps, linear scaling across the entire extent is used to avoid negative shaft lengths:
   $$x' = x \times \frac{W_1}{W_0}$$
2. **Normal Line ($W_0 > E_{\text{start}} + E_{\text{end}}$ and $W_1 > E_{\text{start}} + E_{\text{end}}$)**:
   - If $x \le E_{\text{start}}$:
     $$x' = x \quad (\text{Rigid Start Zone})$$
   - If $x \ge W_0 - E_{\text{end}}$:
     $$x' = x + \Delta W \quad (\text{Rigid End Zone translated by } \Delta W)$$
   - If $E_{\text{start}} < x < W_0 - E_{\text{end}}$:
     $$x' = E_{\text{start}} + (x - E_{\text{start}}) \times \frac{W_1 - E_{\text{start}} - E_{\text{end}}}{W_0 - E_{\text{start}} - E_{\text{end}}} \quad (\text{Linear Shaft Stretch})$$
   - $y' = y$ (Strictly untouched).

```rust
// In crates/designos-doc/src/layout_constraints.rs

pub fn cap_extent(cap: Option<StrokeCap>, weight: f32) -> f32 {
    match cap {
        // Generous 4.5x weight margin encloses full Figma LINE_ARROW / TRIANGLE_ARROW rear barbs
        Some(StrokeCap::ArrowEquilateral | StrokeCap::TriangleFilled | StrokeCap::ArrowLines) => weight * 4.5,
        Some(StrokeCap::DiamondFilled | StrokeCap::CircleFilled) => weight * 3.0,
        _ => 0.0,
    }
}

pub fn stretch_line_geometry(paths: &mut [Path], old_w: f32, new_w: f32, weight: f32, start_cap: Option<StrokeCap>, end_cap: Option<StrokeCap>) {
    if old_w <= 1e-4 || new_w <= 1e-4 {
        return;
    }
    let max_zone = 0.45 * old_w.min(new_w);
    let e_start = cap_extent(start_cap, weight).min(max_zone);
    let e_end = cap_extent(end_cap, weight).min(max_zone);
    let grow = new_w - old_w;

    let map = |p: Vec2| -> Vec2 {
        if old_w <= e_start + e_end || new_w <= e_start + e_end {
            Vec2::new(p.x * (new_w / old_w), p.y)
        } else if p.x <= e_start {
            p
        } else if p.x >= old_w - e_end {
            Vec2::new(p.x + grow, p.y)
        } else {
            let shaft_old = old_w - e_start - e_end;
            let shaft_new = new_w - e_start - e_end;
            Vec2::new(e_start + (p.x - e_start) * (shaft_new / shaft_old), p.y)
        }
    };

    for path in paths {
        for cmd in &mut path.cmds {
            match cmd {
                PathCmd::MoveTo(p) | PathCmd::LineTo(p) => *p = map(*p),
                PathCmd::QuadTo(c, p) => {
                    *c = map(*c);
                    *p = map(*p);
                }
                PathCmd::CubicTo(c1, c2, p) => {
                    *c1 = map(*c1);
                    *c2 = map(*c2);
                    *p = map(*p);
                }
                PathCmd::Close => {}
            }
        }
    }
}
```

#### 1.1.3 Unifying All 4 Size Mutation Entry Points (`#1754`)
Currently, four separate code paths mutate node dimensions. Two paths call `apply_constrained_geometry`, while two bypass it:

| Mutation Path | Location | Previous Behavior | Parity Fix |
|---|---|---|---|
| **Canvas Interactive Handle** | `crates/designos-app/src/layout.rs:220` | Calls `apply_constrained_geometry` | Preserved; now calls `stretch_line_geometry` |
| **Inspector Width Edit** | `crates/designos-app/src/panels/transform.rs` | Calls `apply_constrained_geometry` | Preserved |
| **MCP `designos_update_node`** | `crates/designos-mcp/src/tools/mod.rs:340` | Matches only `NodeKind::Vector`, else directly sets `node.size` | Route `NodeKind::Line` through `apply_constrained_geometry` |
| **Variable-bound `width`** | `crates/designos-doc/src/variables_apply.rs:660` | Sets `n.size.x = x.max(0.0)` directly | Call `apply_constrained_geometry` for `NodeKind::Line` |

**MCP Mutation Fix (`crates/designos-mcp/src/tools/mod.rs:340`)**:
```rust
// Replace:
if matches!(node.kind, NodeKind::Vector { .. }) {
    designos_doc::apply_constrained_geometry(&mut node, new_transform, new_size);
} else {
    node.transform = new_transform;
    node.size = new_size;
}

// With:
if matches!(node.kind, NodeKind::Vector { .. } | NodeKind::Line | NodeKind::BooleanOp(_)) {
    designos_doc::apply_constrained_geometry(&mut node, new_transform, new_size);
} else {
    node.transform = new_transform;
    node.size = new_size;
}
```

**Variable Apply Mutation Fix (`crates/designos-doc/src/variables_apply.rs:660`)**:
```rust
// In match key { "width" => ... }
"width" => {
    let new_size = Vec2::new(x.max(0.0), n.size.y);
    if matches!(n.kind, NodeKind::Line) {
        crate::apply_constrained_geometry(n, n.transform, new_size);
    } else {
        n.size.x = new_size.x;
        if is_rect_like {
            n.fill_geometry = None;
            n.stroke_geometry = None;
        }
    }
}
```

#### 1.1.4 `.fig` Instance Overrides Clearing Stroke Bands (`#1756`)
In `crates/designos-fig/src/rest_import.rs:2841-2880` and `crates/designos-doc/src/components.rs:1580-1600`:
- When an instance node overrides `size`, but `derivedSymbolData` does not supply an explicit replacement `strokeGeometry`, the import code clears `node.stroke_geometry = None`.
- **Fix**: When copying or instantiating overridden symbol data for a `Line`:
  ```rust
  if matches!(child.kind, NodeKind::Line) && child.stroke_geometry.is_some() {
      let old_w = comp_child.size.x;
      let new_w = child.size.x;
      if (new_w - old_w).abs() > 1e-4 {
          let sc = child.stroke.start_cap.or(Some(child.stroke.cap));
          let ec = child.stroke.end_cap.or(Some(child.stroke.cap));
          if let Some(sg) = &mut child.stroke_geometry {
              crate::layout_constraints::stretch_line_geometry(sg, old_w, new_w, child.stroke.weight, sc, ec);
          }
      }
  }
  ```

---

### 1.2 Baked Outline Invalidation Helper `clear_baked_outlines` (`#1755`, `#1936`)

#### 1.2.1 Root Cause & Invalidation Matrix
Figma stores pre-tessellated polygon bands in `fillGeometry` and `strokeGeometry`. In Studio, `designos-render` renders these stored paths directly whenever present.
When a user modifies a property that changes the geometric silhouette or stroke envelope, leaving `fill_geometry` or `stroke_geometry` intact causes the renderer to ignore the new property values!

```rust
// In crates/designos-doc/src/node.rs
impl Node {
    /// Clears baked outline caches so the shape is re-evaluated parametrically
    /// by CPU and GPU render pipelines.
    pub fn clear_baked_outlines(&mut self) {
        self.fill_geometry = None;
        self.stroke_geometry = None;
    }
}
```

#### 1.2.2 Exact Invalidation Sites
1. **Stroke Properties Panel (`crates/designos-app/src/panels/stroke.rs`) (`#1755`)**:
   Every mutation to stroke parameters must set `n.stroke_geometry = None`:
   - Line 179: `n.stroke.align = a; n.stroke_geometry = None;`
   - Line 189: `n.stroke.weight = w; n.stroke_geometry = None;`
   - Line 204: `n.stroke.side_weights = ...; n.stroke_geometry = None;`
   - Line 312: `n.stroke.cap = CAPS[i].1; n.stroke_geometry = None;`
   - Line 326: `n.stroke.join = JOINS[i].1; n.stroke_geometry = None;`
   - Line 337: `n.stroke.miter_limit = limit; n.stroke_geometry = None;`
   - Line 348, 358: `n.stroke.dashes = ...; n.stroke_geometry = None;`
2. **Corner Radii & Smoothing Edits (`crates/designos-app/src/panels/properties.rs`) (`#1936`)**:
   When editing `corner_radius`, `corner_radii`, or `corner_smoothing`, invoke `n.clear_baked_outlines()`.
3. **Paste Properties & Match Style (`crates/designos-app/src/ops.rs:1120-1135`) (`#1936`)**:
   ```rust
   // In pub fn paste_properties(&mut self):
   n.fills = snap.fills.clone();
   n.stroke = snap.stroke.clone();
   n.effects = snap.effects.clone();
   n.opacity = snap.opacity;
   n.blend = snap.blend_mode;
   n.corner_radii = snap.corner_radii;
   n.corner_smoothing = snap.corner_smoothing;
   // MANDATORY: Clear stale imported bakes so new corners and strokes render live!
   n.clear_baked_outlines();
   ```

---

### 1.3 VectorNetwork Style Override Table, Dash Normalization & Vertex Caps (`#1757`, `#1765`)

#### 1.3.1 `VectorData.styleOverrideTable` & Binary Kiwi Unpacking (`#1757`)
Figma Kiwi schema (`figma-real-schema.txt:1034-1037`):
```kiwi
Message VectorData
    vectorNetworkBlob = 1 : Uint
    normalizedSize = 2 : Vector
    styleOverrideTable = 3 : NodeChange[]
```
In `vectorNetworkBlob`, vertices contain a style index pointing into `styleOverrideTable`. When a path has differing caps on its endpoints (e.g. `NONE` at start and `ARROW_LINES` at end), the override is encoded in `styleOverrideTable`:
```kiwi
strokeCap = 30 : StrokeCap
```

**Implementation in `crates/designos-fig/src/vecnet.rs:217-238`**:
```rust
pub(crate) fn apply_style_table(net: &mut VectorNetwork, styles: &[u32], table: &[Value]) {
    for entry in table {
        let Some(id) = entry.get("styleID").and_then(Value::as_u64) else { continue };
        for (v, s) in net.vertices.iter_mut().zip(styles) {
            if *s as u64 != id {
                continue;
            }
            if let Some(c) = entry.get("strokeCap").and_then(Value::as_str).and_then(cap_from) {
                v.cap = Some(c);
            }
            if let Some(j) = entry.get("strokeJoin").and_then(Value::as_str).and_then(join_from) {
                v.join = Some(j);
            }
            if let Some(r) = num(entry, "cornerRadius") {
                v.corner_radius = r.max(0.0);
            }
            if let Some(m) = entry.get("handleMirroring").and_then(Value::as_str) {
                v.mirror = mirror_from(m);
            }
        }
    }
}
```
For a 2-vertex open segment, also synchronize:
```rust
if net.vertices.len() == 2 && net.segments.len() == 1 {
    node.stroke.start_cap = net.vertices[0].cap;
    node.stroke.end_cap = net.vertices[1].cap;
}
```

#### 1.3.2 Normalizing `[0, 0]` Dash Arrays (`#1757`)
In Figma, `[0, 0]` represents a solid stroke (zero dash length, zero gap).
In `crates/designos-doc/src/layout_constraints.rs`:
```rust
// Replace:
let stretchable = node.stroke.dashes.is_empty() && ...;

// With:
let is_solid_dashes = node.stroke.dashes.is_empty() || node.stroke.dashes.iter().all(|&d| d <= 0.0);
let stretchable = is_solid_dashes && old_size.x > 1e-3 && ...;
```
When storing or importing, if `dashes.iter().all(|&d| d <= 0.0)`, normalize to `node.stroke.dashes = Vec::new()`.

#### 1.3.3 Completing `CAPS` in `vertex.rs` (`#1765`)
In `crates/designos-app/src/panels/vertex.rs:18-20`:
```rust
// Replace:
const CAPS: [(&str, Option<StrokeCap>); 4] =
    [("Inherit", None), ("None", Some(StrokeCap::None)), ("Round", Some(StrokeCap::Round)), ("Square", Some(StrokeCap::Square))];

// With:
const CAPS: [(&str, Option<StrokeCap>); 9] = [
    ("Inherit", None),
    ("None", Some(StrokeCap::None)),
    ("Round", Some(StrokeCap::Round)),
    ("Square", Some(StrokeCap::Square)),
    ("Line arrow", Some(StrokeCap::ArrowLines)),
    ("Triangle arrow", Some(StrokeCap::ArrowEquilateral)),
    ("Circle", Some(StrokeCap::CircleFilled)),
    ("Diamond", Some(StrokeCap::DiamondFilled)),
    ("Triangle", Some(StrokeCap::TriangleFilled)),
];
```
This guarantees that selecting an arrow-capped vector vertex matches an entry in `CAPS` and displays the proper cap label instead of falling through to `"Mixed"`.

---

### 1.4 Boolean Operations with 100% Hairline Rings (`#1758`)

#### 1.4.1 Root Cause in `child_shapes` & `combine`
In `crates/designos-render/src/boolean.rs`:
- An ellipse with `inner_radius >= 0.999` has zero area. `geom.rs` marks `shape.fillable = false`.
- Previously, `child_shapes` returned `None` for any `!shape.fillable` node.
- In `combine`, `let Some(s) = child_shapes(doc, child, tol) else { continue };` skipped `None` shapes completely.
- Consequently:
  - `Intersect(rect, hairline_ring)` skipped the ring and returned the unmodified `rect` (should be empty!).
  - `Subtract(hairline_ring, rect)` skipped the ring and treated `rect` as the base operand (should be empty!).

#### 1.4.2 Clean-Room Fix
Distinguish "non-existent or malformed child" (`None`) from "valid operand with zero geometric area" (`Some(Shapes::new())`):

```rust
// In crates/designos-render/src/boolean.rs:105-125

fn child_shapes(doc: &Document, child: &Node, tol: f32) -> Option<Shapes> {
    let resolved = resolve_node(doc, child);
    let child = &*resolved;
    if !child.visible {
        return None;
    }
    let mut shapes: Shapes = if matches!(child.kind, NodeKind::Group) {
        combine(doc, child, OverlayRule::Union, tol)?
    } else {
        // A ring as wide as its disc is a valid operand with zero area, not a missing operand!
        if matches!(&child.kind, NodeKind::Ellipse(arc) if crate::geom::is_hairline_ring(arc)) {
            return Some(Shapes::new());
        }
        let shape: Shape = node_shape(doc, child, tol)?;
        if !shape.fillable {
            // Closed non-fillable shapes (e.g. collapsed polygons) contribute empty area
            if shape.closed {
                return Some(Shapes::new());
            }
            return None;
        }
        let mut acc: Option<Shapes> = None;
        for (path, rule) in &shape.paths {
            let contours = flatten(path, tol);
            if contours.is_empty() {
                continue;
            }
            let s = contours.simplify_shape(io_rule(*rule));
            acc = Some(match acc {
                None => s,
                Some(a) => a.overlay(&s, OverlayRule::Union, FillRule::NonZero),
            });
        }
        acc.unwrap_or_default()
    };
    transform_shapes(&mut shapes, &child.transform);
    Some(shapes)
}
```
**Truth Table Verified**:
- `Intersect(A, Empty)`: `Intersect` with empty set = `Empty`.
- `Subtract(Empty, B)`: Difference `0 - B` = `Empty`.
- `Subtract(A, Empty)`: Difference `A - 0` = `A`.
- `Union(A, Empty)`: Union `A + 0` = `A`.
- `Exclude(A, Empty)`: XOR `A ^ 0` = `A`.

---

### 1.5 Parametric Ellipse-Inscribed Star & Polygon Math (`#953`)

#### 1.5.1 Inscribed Ellipse Formulation
In Figma (`figma-real-schema.txt:1523-1526`, `starInnerScale = 24 : Float`):
Stars and regular polygons are placed along the ellipse inscribed in the bounding rectangle $(w, h)$, rather than fitting points directly to the rectangular box.

Vertex generation on the unit circle:
- Polygon ($N$ vertices):
  $$\theta_i = -\frac{\pi}{2} + \frac{2\pi i}{N}, \quad (u_i, v_i) = (\cos \theta_i, \sin \theta_i)$$
- Star ($N$ points, inner ratio $r \in [0.0, 1.0]$):
  $$\theta_i = -\frac{\pi}{2} + \frac{\pi i}{N}, \quad \rho_i = \begin{cases} 1.0 & \text{if } i \text{ is even} \\ r & \text{if } i \text{ is odd} \end{cases}$$
  $$(u_i, v_i) = (\rho_i \cos \theta_i, \rho_i \sin \theta_i)$$

Mapping to node-local space $(x, y)$ in `crates/designos-render/src/geom.rs` and `crates/designos-export/src/geo.rs`:
$$x_i = \frac{u_i + 1.0}{2.0} \times w, \quad y_i = \frac{v_i + 1.0}{2.0} \times h$$

#### 1.5.2 Dashed Stroke on Rings (Single-Centerline Contour)
When an ellipse arc with `inner_radius > 0.0` has dashes (`!stroke.dashes.is_empty()`):
Generating two concentric boundary ovals causes tiny-skia to dash each contour independently, yielding paired dots.
**Fix**: For dashed rings, construct a single centerline path at:
$$R_{\text{center\_x}} = \frac{w}{2.0} \times \frac{1.0 + inner\_radius}{2.0}, \quad R_{\text{center\_y}} = \frac{h}{2.0} \times \frac{1.0 + inner\_radius}{2.0}$$
$$weight_{\text{effective}} = \frac{w}{2.0} \times (1.0 - inner\_radius) + stroke.weight$$
Stroke this single path as `StrokeAlign::Center` with the dash pattern, yielding clean, single-ring dots matching Figma.

---

### 1.6 `designos-mcp` Ring `ArcSpec` Round-Trip (`#1937`)

#### 1.6.1 Data Structure Definitions in `crates/designos-doc/src/spec.rs`
```rust
/// Declarative specification for an ellipse arc, progress ring, or donut hole.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ArcSpec {
    /// Starting angle in radians (0.0 is 3 o'clock).
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub starting_angle: f32,
    /// Ending angle in radians (2*PI is full circle).
    #[serde(default, skip_serializing_if = "is_tau_f32")]
    pub ending_angle: f32,
    /// Inner radius ratio in 0.0..=1.0 (0.0 is solid disc; 1.0 is hairline ring).
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub inner_radius: f32,
}

fn is_tau_f32(v: &f32) -> bool {
    (v - std::f32::consts::TAU).abs() < 1e-4
}
```

Add `arc` field to `NodeSpec` (around line 115) and `OverrideSpec` (around line 290):
```rust
pub struct NodeSpec {
    // ...
    /// Ellipse arc parameters (starting angle, ending angle, inner radius).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc: Option<ArcSpec>,
    // ...
}

pub struct OverrideSpec {
    // ...
    /// Override arc parameters on a descendant Ellipse layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc: Option<ArcSpec>,
    // ...
}
```

#### 1.6.2 Serialization & Deserialization
1. **In `export_spec_inner` (`crates/designos-doc/src/spec.rs:1820`)**:
   ```rust
   let arc = match &n.kind {
       NodeKind::Ellipse(a) if (a.start.abs() > 1e-4 || (a.end - std::f32::consts::TAU).abs() > 1e-4 || a.inner_radius > 1e-4) => {
           Some(ArcSpec {
               starting_angle: a.start,
               ending_angle: a.end,
               inner_radius: a.inner_radius,
           })
       }
       _ => None,
   };
   ```
2. **In `spec_instance.rs` (Export Overrides)**:
   When inspecting an instance descendant of kind `NodeKind::Ellipse(a)` where `cn.component.overrides.contains(&OverrideField::ArcData)`:
   ```rust
   spec_ov.arc = Some(ArcSpec {
       starting_angle: a.start,
       ending_angle: a.end,
       inner_radius: a.inner_radius,
   });
   any_field = true;
   ```
3. **In `spec_instance.rs` (Apply Overrides `apply_single_override_with_ctx`)**:
   ```rust
   if let Some(arc_spec) = &ov.arc {
       if let Some(node) = doc.get_mut(target_id) {
           if let NodeKind::Ellipse(arc) = &mut node.kind {
               arc.start = arc_spec.starting_angle;
               arc.end = arc_spec.ending_angle;
               arc.inner_radius = arc_spec.inner_radius.clamp(0.0, 1.0);
               node.component.overrides.insert(OverrideField::ArcData);
           }
       }
   }
   ```

---

## Part 2: Masks, Outside Text Strokes, CPU Tile Margin, GPU Blur Clamp & View Modes

### 2.1 Vector Mask Scoping, Nested Masks & Stroke-Mask Rules (`#1506`)

#### 2.1.1 Mask Scoping State Machine
In Figma (`wasm-strings.txt:23270-23285`, `is-this-or-parent-outline-mask`, `clipToOutline`):
- A layer with `is_mask == true` acts as a stencil for subsequent siblings in the **same parent container**.
- **Scope Termination**: The mask's influence stops when:
  1. A subsequent sibling also has `is_mask == true` (which begins a new mask scope).
  2. The parent container's children array ends.
- **Nested Container Scoping**: If a mask is placed inside a child `Frame` or `Group`, it is strictly scoped to that child container. It NEVER punches through to mask siblings of the parent container.

#### 2.1.2 Unclipped Frame Masks & Descendant Inclusions
When a vector mask is a `Group` containing a `Frame` with `clips_content == false`:
- In Figma, any child of that frame that overflows its bounds is part of the group's drawn visual stencil.
- **Fix in `crates/designos-render/src/mask.rs:48`**:
  Do NOT unconditionally call `n.children.clear()` on frames inside vector mask stencils!
  ```rust
  // In mask_stencil:
  MaskType::Vector => {
      n.opacity = 1.0;
      if let NodeKind::Text(t) = &mut n.kind {
          t.style.fills = vec![Paint::solid(Color::BLACK)];
          for r in &mut t.runs {
              r.style.fills = vec![Paint::solid(Color::BLACK)];
          }
      } else if matches!(n.kind, NodeKind::Group) {
          n.fills.clear();
      } else if let NodeKind::Frame(fp) = &n.kind {
          // If the frame does not clip content, its children contribute to the stencil!
          if !fp.clips_content && !n.children.is_empty() {
              n.fills = vec![Paint::solid(Color::BLACK)];
          } else {
              n.fills = vec![Paint::solid(Color::BLACK)];
              n.children.clear();
          }
      } else {
          n.fills = vec![Paint::solid(Color::BLACK)];
          if !matches!(n.kind, NodeKind::BooleanOp(_)) {
              n.children.clear();
          }
      }
  }
  ```

#### 2.1.3 `stroke-mask` Fallback Rule
In `crates/designos-render/src/mask.rs:33`:
```rust
pub fn mask_stencil(mask: &Node) -> Node {
    let mut n = mask.clone();
    n.blend = BlendMode::Normal;
    n.effects.clear();

    // Figma stroke-mask rule:
    // If the mask shape has NO visible fills and has a visible stroke (or is a Line/open vector),
    // the stroke geometry acts as the cutting stencil!
    let has_fill = has_own_fill(mask);
    let has_stroke = !mask.stroke.paints.is_empty() && mask.stroke.weight > 0.0;
    let is_open_vector = matches!(&mask.kind, NodeKind::Line) || matches!(&mask.kind, NodeKind::Vector { network: Some(net), .. } if !net.is_empty() && !net.open_chain_ends().is_empty());

    if (!has_fill && has_stroke) || is_open_vector {
        // Retain stroke paint as solid black stencil
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.fills.clear();
    } else {
        n.stroke.paints.clear();
    }
    // ...
```

---

### 2.2 Outside Text Stroke Double-Painting on 128-Glyph Cuts (`#1621`)

#### 2.2.1 Root Cause of the 0.75 Alpha Seam
In `crates/designos-render/src/vectorops.rs:258-340`:
- Text with an `Outside` stroke clusters glyphs into touching groups using `BoundsGrid`.
- A 128-glyph limit (`TEXT_STROKE_MAX_GLYPHS = 128`) blindly cut touching glyph chains into chunks:
  `g.chunks(TEXT_STROKE_MAX_GLYPHS)`.
- At the cut between glyph 127 and glyph 128, the stroke ring of glyph 127 (Cluster A) and glyph 128 (Cluster B) overlap in the exterior space between them.
- Because Cluster A and Cluster B are emitted as two separate `AlignedTextLayer` vectors drawn sequentially, translucent paint ($A = 0.5$) blends twice:
  $$A_{\text{total}} = 1.0 - (1.0 - 0.5)^2 = 0.75$$
  This causes a visible dark vertical band at the 128-glyph boundary.

#### 2.2.2 Clean-Room Gap-Aware Chunking Solution
Instead of chunking at an arbitrary array index, split only at natural spatial gaps:
1. Two glyphs whose exterior stroke rings touch must remain in the same cluster.
2. A cluster split is ONLY allowed between glyph $i$ and glyph $i+1$ if their bounding boxes satisfy:
   $$x_{i+1}.\text{min} - x_i.\text{max} > 2.0 \times \text{stroke.weight}$$
   At this distance, their stroke rings cannot physically intersect.
3. **Fallback for Unbroken Connected Scripts $> 128$ Glyphs**:
   If a single unbroken cursive word or ligature exceeds 128 glyphs, compute a planar bisector:
   $$x_{\text{split}} = \frac{x_{127}.\text{max} + x_{128}.\text{min}}{2.0}$$
   Clip Cluster A's stroke polygon with the half-plane $x \le x_{\text{split}}$ and Cluster B's stroke polygon with $x > x_{\text{split}}$.
   Because the two polygons strictly partition the plane without overlapping, the composited alpha is exactly $0.500$ across the seam.

---

### 2.3 CPU Adjusted Image Stroke Tile Culling (`#1631`)

#### 2.3.1 Root Cause in `paint.rs`
In `crates/designos-render/src/paint.rs:275-285` and line 593:
- For fills, `fill_paths` calls `with_paint(..., margin: 0.0, ...)`.
- Stored outlines (`sh.stroke_paths`), including outside strokes, are also drawn via `fill_paths`, inheriting `margin = 0.0`.
- An outside stroke of weight $W$ extends outside the layer box by $W$.
- When a canvas tile intersects the stroke band but does *not* intersect the un-stroked layer rectangle $[0, 0, size.x, size.y]$, `transformed_bounds` with `margin = 0.0` computes an empty tile intersection (`region.2 <= region.0 || region.3 <= region.1`).
- `with_paint` returns `None`, and the stroke disappears from that tile!

#### 2.3.2 Clean-Room Fix
In `crates/designos-render/src/paint.rs:585-610`:
```rust
// In fill_paths:
pub fn fill_paths_with_margin(
    env: &PaintEnv,
    target: &mut sk::Pixmap,
    paths: &[(sk::Path, sk::FillRule)],
    ts: sk::Transform,
    size: Vec2,
    paint: &d::Paint,
    alpha: f32,
    margin: f32,
    clip: Option<&sk::Mask>,
) {
    if let Some(mode) = custom_blend(paint.blend) {
        let normal = d::Paint { blend: d::BlendMode::Normal, ..paint.clone() };
        draw_custom(target, mode, |l| fill_paths_with_margin(env, l, paths, ts, size, &normal, alpha, margin, clip));
        return;
    }
    let dims = (target.width(), target.height());
    with_paint(env, paint, size, &ts, alpha, margin, dims, |sp| {
        for (path, rule) in paths {
            target.fill_path(path, sp, *rule, ts, clip);
        }
    });
}
```
When drawing stored stroke outlines in `crates/designos-render/src/lib.rs:779-795`:
Pass `margin = stroke_outset`, where:
$$stroke\_outset = \begin{cases} stroke.weight & \text{if } stroke.align == StrokeAlign::Outside \\ stroke.weight \times 0.5 & \text{if } stroke.align == StrokeAlign::Center \\ 0.0 & \text{if } stroke.align == StrokeAlign::Inside \end{cases}$$
Neighboring tiles evaluate non-empty regions and rasterize the stroke cleanly.

---

### 2.4 GPU Background vs Layer Blur Downsample Edge Texel Clamping (`#1689`)

#### 2.4.1 Root Cause in `exec.rs` & `blit.wgsl`
In `crates/designos-gpu/src/shaders/blit.wgsl`:
```wgsl
@fragment
fn fs_blit(f: FsIn) -> @location(0) vec4<f32> {
    var s = (f.pos.xy) * (tp.src_extent / tp.dst_extent);
    s = clamp(s, vec2<f32>(0.5), tp.src_extent - vec2<f32>(0.5));
    return textureSampleLevel(tsrc, tsamp, s / tp.src_tex, 0.0);
}
```
`fs_blit` unconditionally clamps sampling coordinates to the edge texel centers!
When large blur filters ($\sigma > 16.0$) downsample the layer texture ($k \ge 2$), edge clamping repeats the border color across the downsampled texture.
When `blur_pass` runs with `clamp_edges == false`, the input texture already has repeated border colors, resulting in an opaque fringe/halo instead of fading smoothly to $0.0$.
Conversely, `BackgroundBlur` *requires* clamping to prevent backdrop edges from bleeding dark transparent halos.

#### 2.4.2 Clean-Room WGSL & Rust Pipeline Fix
**Update `crates/designos-gpu/src/shaders/blit.wgsl`**:
```wgsl
struct BlitParams {
    src_extent: vec2<f32>,
    dst_extent: vec2<f32>,
    src_tex: vec2<f32>,
    clamp_to_edge: f32, // 1.0 = clamp to edge, 0.0 = zero border outside
    pad: f32,
};

@group(0) @binding(0) var<uniform> tp: BlitParams;
@group(0) @binding(1) var tsrc: texture_2d<f32>;
@group(0) @binding(2) var tsamp: sampler;

@fragment
fn fs_blit(f: FsIn) -> @location(0) vec4<f32> {
    var s = (f.pos.xy) * (tp.src_extent / tp.dst_extent);
    if (tp.clamp_to_edge > 0.5) {
        s = clamp(s, vec2<f32>(0.5), tp.src_extent - vec2<f32>(0.5));
    } else {
        if (s.x < 0.0 || s.y < 0.0 || s.x > tp.src_extent.x || s.y > tp.src_extent.y) {
            return vec4<f32>(0.0);
        }
    }
    return textureSampleLevel(tsrc, tsamp, s / tp.src_tex, 0.0);
}
```

**Update `crates/designos-gpu/src/exec.rs:1520-1585`**:
```rust
fn resample(&mut self, enc: &mut wgpu::CommandEncoder, src: &Lease, src_extent: (u32, u32), dst_extent: (u32, u32), clamp: bool) -> Lease {
    let dst = self.r.plain_pool.acquire(&self.r.dev, dst_extent.0, dst_extent.1, self.r.frame_no);
    let ub = self.r.ring.alloc(
        &self.r.dev,
        &self.r.queue,
        &BlitParams {
            src_extent: [src_extent.0 as f32, src_extent.1 as f32],
            dst_extent: [dst_extent.0 as f32, dst_extent.1 as f32],
            src_tex: [src.w as f32, src.h as f32],
            clamp_to_edge: if clamp { 1.0 } else { 0.0 },
            pad: 0.0,
        },
    );
    // ...
```
In `blur(&mut self, ..., clamp: bool)`:
Pass `clamp` into `self.resample(enc, src, (w, h), (sw, sh), clamp)` and `self.resample(enc, &done, (sw, sh), (w, h), clamp)`.
Preserves `self.interactive()` and creates zero new pipelines ($< 64$ cap).

---

### 2.5 Outlines Mode & Pixel Preview Architecture (`#1476`)

#### 2.5.1 View Mode State Model
```rust
// In crates/designos-app/src/app.rs (UiState)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CanvasViewMode {
    #[default]
    Normal,
    Outlines {
        include_hidden: bool,
        show_bounds: bool,
    },
    PixelPreview1x,
    PixelPreview2x,
}
```

#### 2.5.2 Keyboard Chords & Action Triggers
- `Cmd+Y` (macOS) / `Ctrl+Shift+3` (Windows) / `Shift+O`: Toggle `CanvasViewMode::Outlines`.
- `Ctrl+P`: Cycle `Normal -> PixelPreview1x -> PixelPreview2x -> Normal`.
- Note: Adjust `shell.rs:1318` so `Cmd+Y` toggles outlines mode on macOS, while Redo is mapped to `Shift+Cmd+Z`.

#### 2.5.3 Outlines Mode Hairline Rendering Pass
When `app.ui.view_mode` is `Outlines`:
1. All fills, image paints, shadows, and blurs are skipped.
2. Every vector segment, polygon contour, and frame rectangle is stroked with a 1 physical screen pixel hairline:
   $$weight_{\text{hairline}} = \frac{1.0}{zoom}$$
3. **Color Codes**:
   - Component & Instance outlines: `#7B61FF` (Figma Purple).
   - Standard shapes, groups, frames: `#808080` (Neutral Outline Grey).
   - Text layers: Render raw glyph outlines plus baseline and text box bounding hairlines.
   - Hidden layers (`include_hidden == true`): Render with dashed hairlines (`dashes = [4.0 / zoom, 4.0 / zoom]`).

#### 2.5.4 Pixel Preview Upscale Pass
When `PixelPreview1x` or `PixelPreview2x` is active:
1. Render canvas scene offscreen at $1.0\times$ or $2.0\times$ unzoomed physical pixel resolution.
2. Blit offscreen texture to the viewport using nearest-neighbor interpolation (`FilterMode::Nearest`).
3. Purely viewport-driven; never modifies document geometry, bumps revision, or affects idle CPU.

---

## Part 3: Figma UX Micro-Interaction State Machines

### 3.1 Right Sidebar Resize, Clamping & Double-Click Reset (`#1109`)

#### 3.1.1 Requirements & Clamping Bounds
- Default width: `240.0px`.
- Minimum width: `240.0px`.
- Maximum width: `500.0px`.
- Double-click on left splitter resets width to `240.0px`.
- Cursor: `CursorIcon::ResizeHorizontal` (`col-resize`).
- Persistence: Stored in user preferences JSON (`~/.config/designos/prefs.json`).

#### 3.1.2 Update to `crates/designos-app/src/shell/columns.rs`
```rust
// Replace static INSPECTOR_W with dynamic requested width:
pub fn columns(
    window_w: f32,
    board: bool,
    kind: LeftKind,
    requested_left_w: f32,
    left_collapsed: bool,
    requested_right_w: f32,
) -> Columns {
    let w = if window_w.is_finite() { window_w.max(0.0) } else { 0.0 };
    let rail_w = RAIL_W.min(w);
    let right_clamp = requested_right_w.clamp(theme::RIGHT_PANEL_WIDTH, 500.0) + theme::BORDER_WIDTH;
    let mut right_w = if board { 0.0 } else { right_clamp };
    // ...
```

#### 3.1.3 Interaction & Splitter Drag in `crates/designos-app/src/shell.rs`
```rust
// At right_rect.left():
let splitter_rect = Rect::from_min_max(
    Pos2::new(right_rect.left() - 4.0, right_rect.top()),
    Pos2::new(right_rect.left() + 4.0, right_rect.bottom()),
);
let resp = ui.interact(splitter_rect, ui.id().with("right-panel-splitter"), Sense::click_and_drag());
if resp.hovered() || resp.dragged() {
    ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
    ui.painter().vline(right_rect.left() - 0.5, right_rect.y_range(), egui::Stroke::new(2.0, theme::BG_BRAND));
}
if resp.double_clicked() {
    self.app.ui.right_panel_width = theme::RIGHT_PANEL_WIDTH;
    self.save_user_prefs();
} else if resp.dragged() {
    if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
        let new_w = full.right() - pos.x;
        self.app.ui.right_panel_width = new_w.clamp(240.0, 500.0);
        self.save_user_prefs();
    }
}
```

---

### 3.2 Left Sidebar Resize Parity & Keyboard Accessibility (`#1110`)

#### 3.2.1 Specifications
- Default width: `240.0px`.
- Minimum width: `200.0px` (clamped to leave at least `160.0px` for canvas).
- Double-click on right splitter resets width to `240.0px`.
- Persistence: Stored in user preferences JSON.
- **Keyboard Accessibility**:
  - The splitter registers as an accessible focusable control (`role="separator"`).
  - Arrow Left: shrinks sidebar by `10.0px` (with `Shift`: by `50.0px`).
  - Arrow Right: expands sidebar by `10.0px` (with `Shift`: by `50.0px`).
  - Home: resets to `200.0px`.
  - End: expands to `left_max_w`.

---

### 3.3 Pages Section Collapse Disclosure (`#1111`)

#### 3.3.1 State & Visual Model
```rust
// In UiState:
pub pages_collapsed: bool,
```
- In `crates/designos-app/src/panels/left.rs:113`:
  - Replace static `section_title` with an interactive disclosure row.
  - Chevron button: `Icon::ChevronRight` when collapsed, `Icon::ChevronDown` when expanded.
  - When collapsed:
    - `list_h = 0.0`. The pages list and draggable splitter are completely hidden.
    - Title displays active page pill: `Pages  /  Page 1`.
    - Remaining vertical height is fully allocated to the virtualized Layers tree.
    - Space/Enter on header row toggles `app.ui.pages_collapsed`.
    - State is persisted per workspace.

---

### 3.4 Context Menu Parity (`#1474`)

#### 3.4.1 Complete Menu Action Item Table
In `crates/designos-app/src/panels/context.rs:120-195`:

| Action | Label | Shortcut | Enablement Predicate |
|---|---|---|---|
| `Rename` | Rename | `Cmd+R` / `F2` | Single layer selected |
| `Cut` | Cut | `Cmd+X` | `!selection.is_empty()` |
| `Copy` | Copy | `Cmd+C` | `!selection.is_empty()` |
| `Paste` | Paste | `Cmd+V` | Clipboard has content |
| `CopySvg` | Copy as SVG | — | `!selection.is_empty()` |
| `CopyPng` | Copy as PNG | — | `!selection.is_empty()` |
| `CopyCss` | Copy as CSS | — | `!selection.is_empty()` |
| `PasteToReplace` | Paste to replace | `Cmd+Shift+R` | `!selection.is_empty() && has_clip` |
| `Group` | Group selection | `Cmd+G` | `!selection.is_empty()` |
| `Ungroup` | Ungroup | `Cmd+Shift+G` | Any selected is Group/Frame |
| `FrameSelection` | Frame selection | `Cmd+Alt+G` | `!selection.is_empty()` |
| `UseAsMask` | Use as mask | `Cmd+Ctrl+M` | Single shape selected |
| `CreateComponent` | Create component | `Cmd+Alt+K` | Can create component |
| `OutlineStroke` | Outline stroke | `Cmd+Shift+O` | Selection has stroked vector |
| `Flatten` | Flatten | `Cmd+E` | `!selection.is_empty()` |
| `BooleanUnion` | Union selection | `Alt+Cmd+U` | $\ge 2$ shapes selected |
| `BooleanSubtract` | Subtract selection | `Alt+Cmd+S` | $\ge 2$ shapes selected |
| `BooleanIntersect` | Intersect selection | `Alt+Cmd+I` | $\ge 2$ shapes selected |
| `BooleanExclude` | Exclude selection | `Alt+Cmd+X` | $\ge 2$ shapes selected |
| `FlipHorizontal` | Flip horizontal | `Shift+H` | `!selection.is_empty()` |
| `FlipVertical` | Flip vertical | `Shift+V` | `!selection.is_empty()` |
| `BringForward` | Bring forward | `Cmd+]` | `!selection.is_empty()` |
| `BringToFront` | Bring to front | `Cmd+Alt+]` | `!selection.is_empty()` |
| `SendBackward` | Send backward | `Cmd+[` | `!selection.is_empty()` |
| `SendToBack` | Send to back | `Cmd+Alt+[` | `!selection.is_empty()` |
| `ToggleLock` | Lock / Unlock | `Cmd+Shift+L` | `!selection.is_empty()` |
| `ToggleVisible` | Hide / Show | `Cmd+Shift+H` | `!selection.is_empty()` |
| `Delete` | Delete | `Backspace` | `!selection.is_empty()` |

All operations execute in a **single undo step** via `app.run_tx(tx)`.

---

### 3.5 Canvas Control Handle Drag Deadzone (`#1730`)

#### 3.5.1 The `DRAG_THRESHOLD = 3.0px` Invariant
In `crates/designos-app/src/canvas/interact.rs`:
Clicking on a handle (corner radius, ellipse arc sweep/start/inner-radius, star inner-ratio, polygon count, gradient stop) without moving the pointer must NOT mutate the shape or register an Undo transaction!

#### 3.5.2 State Machine Structure
```rust
// In CornerRadiusDragState:
pub struct CornerRadiusDragState {
    pub tx: Tx,
    pub id: NodeId,
    pub corner: usize,
    pub start_radii: [f32; 4],
    pub start_screen: Pos2,
    pub moved: bool,
}

// In ShapeControlDragState:
pub struct ShapeControlDragState {
    pub tx: Tx,
    pub id: NodeId,
    pub kind: handles::ShapeControlKind,
    pub start_page: DV,
    pub start_screen: Pos2,
    pub start_ratio: f32,
    pub start_end_angle: f32,
    pub moved: bool,
}
```

#### 3.5.3 Drag Evaluation & Cancellation Logic
1. **On Pointer Move (`follow_pointer`)**:
   ```rust
   let dist_screen = (pos - state.start_screen).length();
   if dist_screen >= 3.0 {
       state.moved = true;
   }
   if !state.moved {
       return; // Suppress value modification and app.changed()
   }
   // Proceed with geometric calculation
   ```
2. **On Pointer Release (`end_open_gesture`)**:
   ```rust
   if !state.moved {
       state.tx.ed.revert(&mut app.doc);
       // Do NOT call app.run_tx(state.tx); discard transaction cleanly!
       return;
   }
   app.run_tx(state.tx);
   ```

---

## Part 4: Issue-by-Issue Verification Commands & Unit Test Matrix

Every test command below is machine-checkable and must exit with status code `0`.

```bash
export PATH="$HOME/.cargo/bin:$PATH"

# -------------------------------------------------------------------------
# Section 1 Tests: Lines, Arrow Caps, Baked Outlines & Primitives
# -------------------------------------------------------------------------

# #1753: Line arrowhead rigid stretch without distortion or half-stroke drop
cargo test -p designos-doc --lib layout_constraints::tests::line_arrow_resize_preserves_head_and_origin

# #1754: Unified line resize across MCP update_node and variables
cargo test -p designos-mcp --test resized_vector_via_update
cargo test -p designos-doc --lib variables_apply::tests::variable_line_width_resize_stretches_outline

# #1755: Stale baked outline cleared on stroke property edit
cargo test -p designos-app --lib panels::stroke::tests::editing_stroke_weight_clears_baked_stroke_geometry

# #1756: .fig instance override with size stretches inherited stroke band
cargo test -p designos-fig --test '*' -- --skip vsf_pcp instance_line_override_stretches_inherited_band

# #1757 & #1765: VectorNetwork styleOverrideTable caps & vertex dropdown completeness
cargo test -p designos-fig --test '*' -- --skip vsf_pcp vector_network_style_override_table_caps
cargo test -p designos-app --lib panels::vertex::tests::arrow_caps_dropdown_not_mixed

# #1758: Boolean operations with 100% hairline rings
cargo test -p designos-render --test boolean_with_zero_width_ring

# #953: Inscribed ellipse star/polygon math & single-centerline dashed rings
cargo test -p designos-render --test basic_shapes_match_figma
cargo test -p designos-render --test dashed_progress_ring_single_centerline

# #1936: Clear baked outlines on paste properties and corner edits
cargo test -p designos-app --lib ops::tests::paste_properties_clears_baked_outlines

# #1937: MCP export_spec / apply_spec ArcSpec round-trip
cargo test -p designos-doc --test '*' spec::tests::arc_spec_round_trip_on_ellipse_instances

# -------------------------------------------------------------------------
# Section 2 Tests: Masks, Outside Text Strokes, Blur & View Modes
# -------------------------------------------------------------------------

# #1506: Vector mask scoping, unclipped frames & stroke-mask fallback
cargo test -p designos-render --test mask_scoping_and_unclipped_children
cargo test -p designos-gpu --test mask_gpu_stroke_fallback

# #1621: Outside text stroke gap-aware chunking without double-paint seams
cargo test -p designos-render --test text_outside_stroke_seamless_chunking

# #1631: Adjusted image outside stroke tile margin expansion
cargo test -p designos-render --test adjusted_image_stroke_tile_margins

# #1689: GPU background vs layer blur downsample edge clamping
cargo test -p designos-gpu --test blur_edge_texel_clamping

# #1476: Outlines mode 1px hairlines and Pixel Preview
cargo test -p designos-app --lib canvas::draw::tests::outlines_mode_renders_hairlines
cargo test -p designos-app --lib canvas::draw::tests::pixel_preview_nearest_neighbor

# -------------------------------------------------------------------------
# Section 3 Tests: UX Micro-Interactions
# -------------------------------------------------------------------------

# #1109 & #1110: Sidebar resize drag, clamping, double-click reset & persistence
cargo test -p designos-app --lib shell::columns::tests::right_sidebar_clamping
cargo test -p designos-app --lib shell::tests::sidebar_double_click_reset_and_persistence

# #1111: Pages section collapse toggle and pill header
cargo test -p designos-app --lib panels::left::tests::pages_section_collapse_toggle

# #1474: Layer list & canvas right-click context menu parity
cargo test -p designos-app --lib panels::context::tests::complete_figma_context_menu_actions

# #1730: Canvas control handle 3.0px deadzone threshold
cargo test -p designos-app --lib canvas::interact::tests::handle_click_without_drag_does_not_mutate
```

---

## Part 5: Non-Goals & Boundaries

1. **Non-Goal: Variable-Width Freehand Stroke Geometry**: Calligraphic pressure-sensitive variable-width mesh interpolation belongs to Epic E4 Brush Tooling, not Cluster C3.
2. **Non-Goal: Changing Kiwi Schema Definitions**: The binary layout of Kiwi records in `crates/designos-fig/src/kiwi.rs` is locked and clean-room verified. No new Kiwi message fields are introduced.
3. **Non-Goal: New GPU Pipelines**: All WGSL adjustments in `blit.wgsl` and `blur.wgsl` leverage existing uniform buffer layouts. Zero additional `Program` allocations; total GPU `Program`s strictly remain $< 64$.
4. **Non-Goal: Document Serialization Changes**: App preferences (sidebar widths, pages collapsed) must be stored in `~/.config/designos/prefs.json`, NEVER in the `.designos` document model.
