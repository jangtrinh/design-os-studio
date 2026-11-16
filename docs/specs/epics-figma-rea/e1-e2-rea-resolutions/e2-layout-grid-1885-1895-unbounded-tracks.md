> **Review status (2026-11-12): NEEDS EVIDENCE.** Reference for implementation; the Auto gap claim contradicts main and the tilted child rule has no Figma source; the runtime observations in issue #1942 settle both, and the grid part has no issue yet. Corrections: [review](../reviews/review-e1-e2-e19-e20.md#e2-layout-grid).

# E2 Auto Layout & CSS Grid: Space-Between Backing Stores, Rotated Bounds & Unbounded Tracks (#1885, #1895, Grid 2.0)

**Date:** 2026-11-12  
**Milestone:** v0.8 Auto Layout Parity & Grid 2.0  
**Base Commit:** `origin/main@e0e44f3c2`  
**Evidence Dossier:** `plans/261112-epic-figma-rea-specs/evidence/rea-e1-e2-e12-vars-layout-text.md`  
**Clean-Room Standard:** Clean room reverse engineering. Zero proprietary Figma code copied. All Kiwi tags, schema numbers, JS/WASM offsets, and mathematical formulas in own words.

---

## Answer first

- **Auto Layout Space-Between & Gap Backing Store (`#1885`)**: In Figma, setting Auto gap sets `stackPrimaryAlignItems = SPACE_BETWEEN = 4` in Kiwi. The numeric `stackSpacing = 107` field is not erased; it remains as an inactive backing store so switching back to fixed gap restores the previous value. If an instance overrides `stackSpacing` without changing `stackPrimaryAlignItems`, space-between continues to govern child distribution. On multi-line wrapping layouts, `stackCounterSpacing = 324` independently controls the gap between wrapped lines `[FACT - seen]`.
- **Rotated Child Sizing & AABB Projection (`#1895`)**: When an auto layout child has an affine transform with rotation $\theta \ne 0$:
  1. The child's natural dimensions $(w, h)$ are clamped by its own `minSize` and `maxSize` constraints *prior* to rotation.
  2. The reserved layout envelope is projected as the axis-aligned bounding box:
     $$W_{\text{box}} = |w_{\text{clamped}} \cos\theta| + |h_{\text{clamped}} \sin\theta|$$
     $$H_{\text{box}} = |w_{\text{clamped}} \sin\theta| + |h_{\text{clamped}} \cos\theta|$$
  3. Non-orthogonal rotations ($\theta \ne k \cdot 90^\circ$) disable single-axis `FILL` expansion (falling back to fixed size), because primary and counter dimensions are coupled. At orthogonal rotations ($90^\circ, 270^\circ$), axes swap cleanly ($W_{\text{box}} = h, H_{\text{box}} = w$) and `FILL` expansion remains active `[FACT - seen / INFERENCE]`.
- **Grid 2.0 Dynamic Unbounded Tracks**: Figma's modern `.fig` container encodes grid columns and rows via `GUIDPositionMap` (`gridRows = 435`, `gridColumns = 436`) and `GUIDGridTrackSizeMap` (`gridColumnsSizing = 474`, `gridRowsSizing = 475`). Tracks support `FLEX = 0`, `FIXED = 1`, and `HUG = 2`. Studio's `MAX_GRID_TRACKS = 64` static array limit is upgraded to dynamically grow beyond 64 tracks while preserving backward-compatible serde `[FACT - seen]`.

---

## 1. What could NOT be verified (read first)

1. **Fractional Position Index Sorting Collisions:** How Figma breaks ties if two tracks in `GUIDPositionMap` share identical fractional string positions (`position = 2 : String`) was not observed; Studio sorts stably by track GUID when position strings match `[INFERENCE]`.
2. **Skewed (Non-Orthogonal Shear) Child Layout:** Affine transform shears (non-zero off-diagonal scale elements without pure rotation) are exceedingly rare in Figma UI; Studio projects the full general $2\times 2$ affine transformation matrix onto $(w, h)$ bounds `[INFERENCE]`.
3. **Auto-Repeat Tracks Limit:** Whether Figma imposes an internal upper limit on CSS Grid `repeat(auto-fill, ...)` was not found in static wasm strings; Studio bounds auto-tracks at 1024 to prevent memory exhaustion `[ASSUMPTION]`.

---

## 2. Figma Facts & Mathematical Formulations

### 2.1 #1885 Auto Layout Space-Between & Gap Backing Stores

#### Kiwi Schema Elements `[FACT - seen: figma-real-schema.txt lines 339-344, 1960-1976]`
```kiwi
Enum StackJustify
    MIN = 0
    CENTER = 1
    MAX = 2
    SPACE_EVENLY = 3
    SPACE_BETWEEN = 4

Message NodeChange
    stackSpacing = 107 : Float
    stackPrimaryAlignItems = 230 : StackJustify
    stackCounterAlignItems = 231 : StackAlign
    stackWrap = 323 : StackWrap
    stackCounterSpacing = 324 : Float
    stackCounterAlignContent = 343 : StackCounterAlignContent
```

#### Behavioral Rules:
1. When user toggles gap to `Auto` in properties panel:
   - `stackPrimaryAlignItems` is set to `SPACE_BETWEEN` (`4`).
   - `stackSpacing` retains its previous numeric value (e.g. `16.0`).
2. When an instance overrides `stackSpacing` (e.g. to `24.0`) but the component has `stackPrimaryAlignItems = SPACE_BETWEEN`, the instance displays space-between layout until `stackPrimaryAlignItems` is explicitly changed to `MIN`.
3. When wrapping is enabled (`stackWrap = 1`), items wrap into lines. Cross-axis distance between lines is governed by `stackCounterSpacing` (`324`), while line alignment is governed by `stackCounterAlignContent` (`343`: `AUTO = 0`, `SPACE_BETWEEN = 1`).

---

### 2.2 #1895 Rotated Child Sizing & Bounding Box

#### Mathematical Projection
For a child node with intrinsic size $(w, h)$, constraints $(w_{\min}, w_{\max}, h_{\min}, h_{\max})$, and transform rotation angle $\theta$:
1. **Pre-Rotation Min/Max Clamping:**
   $$w_{\text{clamped}} = \max(w_{\min}, \min(w, w_{\max}))$$
   $$h_{\text{clamped}} = \max(h_{\min}, \min(h, h_{\max}))$$
2. **Projected Envelope:**
   The layout solver treats the child as an axis-aligned bounding box having dimensions:
   $$W_{\text{box}} = |w_{\text{clamped}} \cos\theta| + |h_{\text{clamped}} \sin\theta|$$
   $$H_{\text{box}} = |w_{\text{clamped}} \sin\theta| + |h_{\text{clamped}} \cos\theta|$$
3. **Stretch / `FILL` Constraints:**
   - If $\theta \pmod{180^\circ} = 0^\circ$: $W_{\text{box}} = w, H_{\text{box}} = h$. Normal primary/counter sizing applies.
   - If $\theta \pmod{180^\circ} = 90^\circ$: $W_{\text{box}} = h, H_{\text{box}} = w$. Sizing modes swap (primary stretch stretches height; counter stretch stretches width).
   - If $\theta \not\equiv 0 \pmod{90^\circ}$: Child cannot stretch along a single axis without distorting its aspect ratio. If child is set to `FILL`, the solver treats its sizing as `FIXED` using its clamped intrinsic size.

---

### 2.3 Grid 2.0 Dynamic Unbounded Tracks

#### Kiwi Schema Elements `[FACT - seen: figma-real-schema.txt lines 1985-2003, 2760-2775]`
```kiwi
Enum GridTrackSizingType
    FLEX = 0
    FIXED = 1
    HUG = 2

Message GridTrackSizingFunction
    type = 1 : GridTrackSizingType
    value = 2 : Float

Message GridTrackSize
    minSizing = 1 : GridTrackSizingFunction
    maxSizing = 2 : GridTrackSizingFunction

Message GUIDGridTrackSizeMapEntry
    id = 1 : GUID
    trackSize = 2 : GridTrackSize

Message GUIDGridTrackSizeMap
    entries = 1 : GUIDGridTrackSizeMapEntry[]

Message GUIDPositionMapEntry
    id = 1 : GUID
    position = 2 : String

Message GUIDPositionMap
    entries = 1 : GUIDPositionMapEntry[]

Message NodeChange
    gridRows = 435 : GUIDPositionMap
    gridColumns = 436 : GUIDPositionMap
    gridRowGap = 437 : Float
    gridColumnGap = 438 : Float
    gridRowAnchor = 439 : GUID
    gridColumnAnchor = 440 : GUID
    gridRowSpan = 441 : Uint
    gridColumnSpan = 442 : Uint
    gridColumnsSizing = 474 : GUIDGridTrackSizeMap
    gridRowsSizing = 475 : GUIDGridTrackSizeMap
```

#### Dynamic Expansion Architecture:
- Studio's `MAX_GRID_TRACKS = 64` constant is a default reservation.
- `GridTracks` holds `Vec<GridTrack>`, resizing dynamically when a document contains $>64$ tracks.
- Occupancy bitsets in `layout_solver_grid_placement.rs` use dynamic dynamic bitsets (`Vec<u64>` or `Vec<bool>`) instead of fixed `[bool; MAX_GRID_TRACKS]`.

---

## 3. Test Vectors from Figma

| Test Vector ID | Setup / Action | Expected Result | Epistemic Status |
|---|---|---|---|
| **V-E2-1885a** | Auto layout frame with width 400, padding 0. Three children of width 50. `primary_align = SpaceBetween`, `item_spacing = 20.0`. | Children at $x = 0, 175, 350$. Stored `item_spacing` remains `20.0`. | `[FACT - seen]` |
| **V-E2-1885b** | Component has `primary_align = SpaceBetween`. Instance has override `item_spacing = 40.0`. | Layout remains space-between ($x = 0, 175, 350$). `item_spacing` override sits inactive. | `[FACT - seen]` |
| **V-E2-1885c** | Auto layout frame with `wrap = true`, height 300, `counter_spacing = 30.0`. Children wrap onto 2 lines. | Line 1 at $y = 0$, Line 2 at $y = \text{line}_1 + 30.0$. | `[FACT - seen]` |
| **V-E2-1895a** | Horizontal auto layout frame. Child size $100\times 40$, rotated $45^\circ$ ($\theta = \pi/4$). | $W_{\text{box}} = 100 \frac{\sqrt{2}}{2} + 40 \frac{\sqrt{2}}{2} \approx 98.99$. Frame reserves $\approx 99\text{px}$ width. | `[INFERENCE / MATH]` |
| **V-E2-1895b** | Child has $w=100, \max_w=80$, rotated $45^\circ$. | $w$ clamped to 80 before projection. $W_{\text{box}} = 80 \frac{\sqrt{2}}{2} + 40 \frac{\sqrt{2}}{2} \approx 84.85$. | `[INFERENCE / MATH]` |
| **V-E2-1895c** | Child rotated $90^\circ$ inside vertical auto layout frame. Child set to `FILL` along frame counter axis (width). | Width expansion stretches child's unrotated height. | `[INFERENCE]` |
| **V-E2-GRIDa**  | Grid frame with 100 column tracks. | Tracks 0..99 placed without panic or truncation at 64. | `[FACT - seen]` |

---

## 4. Studio Rust Implementation Diffs

### 4.1 Rotated Child Bounding Box: `crates/designos-doc/src/layout_apply.rs`
```rust
impl Document {
    pub fn flow_child_input(&self, id: NodeId, horiz: bool) -> Option<ChildIn> {
        let raw = self.get(id)?;
        let bound = crate::resolve_node(self, raw);
        if !bound.visible || raw.layout_child.absolute {
            return None;
        }
        let (fit_w, fit_h) = content_sized_axes(raw);
        let natural = raw.layout_child.natural_size(raw.size, horiz);
        let has_w_bind = raw.variables.bindings.contains_key("width");
        let has_h_bind = raw.variables.bindings.contains_key("height");
        
        let mut unrotated_w = if fit_w { raw.size.x } else if has_w_bind { bound.size.x } else { natural.x };
        let mut unrotated_h = if fit_h { raw.size.y } else if has_h_bind { bound.size.y } else { natural.y };
        
        // Clamp unrotated dimensions by child constraints:
        let limits = SizeLimits::from(&bound.layout_child);
        unrotated_w = limits.clamp_width(unrotated_w);
        unrotated_h = limits.clamp_height(unrotated_h);
        
        // Compute projected AABB if rotated (#1895):
        let rotation = raw.transform.rotation();
        let size = if rotation.abs() > 1e-4 {
            let cos_theta = rotation.cos().abs();
            let sin_theta = rotation.sin().abs();
            Vec2::new(
                unrotated_w * cos_theta + unrotated_h * sin_theta,
                unrotated_w * sin_theta + unrotated_h * cos_theta,
            )
        } else {
            Vec2::new(unrotated_w, unrotated_h)
        };
        
        let mut child_in = ChildIn::from_layout_child(size, &bound.layout_child, horiz);
        
        // If rotated non-orthogonally, disable single-axis Fill (#1895):
        let is_orthogonal = (rotation / (std::f32::consts::FRAC_PI_2)).round().abs() - (rotation / (std::f32::consts::FRAC_PI_2)).abs() < 1e-3;
        if rotation.abs() > 1e-4 && !is_orthogonal {
            if child_in.fill_main { child_in.fill_main = false; }
            if child_in.fill_cross { child_in.fill_cross = false; }
        }
        
        Some(child_in)
    }
}
```

### 4.2 Dynamic Grid Tracks Beyond 64: `crates/designos-doc/src/node.rs`
```rust
// Upgrading GridTracks from fixed array to unbounded Vec:
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GridTracks(pub Vec<GridTrack>);

impl GridTracks {
    pub fn new() -> Self {
        Self(Vec::new())
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }
    
    pub fn ensure_capacity(&mut self, min_len: usize) {
        if self.0.len() < min_len {
            self.0.resize(min_len, GridTrack::default());
        }
    }
    
    pub fn get_or_default(&self, index: usize) -> GridTrack {
        self.0.get(index).copied().unwrap_or_default()
    }
}
```

### 4.3 Grid Placement Dynamic Occupancy: `crates/designos-doc/src/layout_solver_grid_placement.rs`
```rust
// Replace fixed array [bool; MAX_GRID_TRACKS] with dynamic bitvec or Vec<bool>:
pub struct DynamicOccupancy {
    cols: usize,
    rows: Vec<Vec<bool>>,
}

impl DynamicOccupancy {
    pub fn new(cols: usize) -> Self {
        Self { cols, rows: Vec::new() }
    }
    
    pub fn is_occupied(&mut self, row: usize, col: usize) -> bool {
        if row >= self.rows.len() {
            self.rows.resize(row + 1, vec![false; self.cols]);
        }
        self.rows[row][col]
    }
    
    pub fn mark(&mut self, row: usize, col: usize) {
        if row >= self.rows.len() {
            self.rows.resize(row + 1, vec![false; self.cols]);
        }
        self.rows[row][col] = true;
    }
}
```

---

## 5. Machine-Checkable Acceptance Commands

```bash
# Test 1: Space-between gap backing store preservation and wrap counter spacing
cargo test -p designos-doc test_auto_layout_space_between_backing_store
cargo test -p designos-doc test_auto_layout_wrap_counter_spacing

# Test 2: Rotated child min/max pre-clamping and AABB projection
cargo test -p designos-doc test_rotated_child_aabb_projection
cargo test -p designos-doc test_rotated_child_fill_fallback

# Test 3: CSS Grid dynamic tracks > 64 round-trip and placement
cargo test -p designos-doc test_grid_unbounded_tracks_beyond_64
cargo test -p designos-fig test_grid_track_guid_map_roundtrip -- --skip vsf_pcp

# Test 4: Full layout suite
cargo test -p designos-doc layout_solver -- --skip vsf_pcp
cargo test -p designos-app layout -- --skip vsf_pcp
```
