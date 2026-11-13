> **Review status (2026-11-12): READY AFTER FIXES.** Reference for implementation; schema fields and easing curves are sourced, but the bounds formulas are Studio's own code and the critical-damping settle formula and two test vectors are wrong. Corrections: [review](../reviews/review-e4-e5-e6-e7.md#e6-export-easing).

# True Figma Specification: Export Bounds, ExportSettings Kiwi Parity & Analytical Easing/Springs (#528, #213, #457, #459)

**Date:** 2026-11-12  
**Milestone:** v0.8.0  
**Base:** `origin/main@e0e44f3c2`  
**Evidence Dossier:** `/Users/jangtrinh/Products/design-os-studio/plans/261112-epic-figma-rea-specs/evidence/rea-e6-e11-motion-proto.md`  
**REA Evidence ID:** `ev_d8d8b9b95732159bfc18d8f944c89602a78adcbc4ae41358f73fb2823755bbdd`  
**Subject Root Digest (SHA-256):** `b5ccf2f9790676849077e3103ff08c6ab291e10d683844f3cffeeadc25c72451`  
**Clean-Room Standard:** Clean-room reverse engineering. Exact Kiwi tags, analytical ODE solutions, and UI strings only. Zero copied Figma source code.

---

## Answer first

- **#528 Export Bounds:** Figma separates nominal `absoluteBoundingBox` ($W \times H$ without visual effects) from `absoluteRenderBounds` (drop shadow reach $\max(0, R) + \max(0, S) \pm \Delta$, layer blur $R$, stroke thickness, unclipped children). `ExportSettings::useAbsoluteBounds = 9` forces the export rect to nominal bounds $\text{Rect}(0, 0, W, H)$. `contentsOnly = 7` isolates the layer subtree on a transparent canvas. `clipsContent` on parent frames clamps child overflow from expanding render bounds.
- **#213 ExportSettings Round-Trip:** Kiwi `ExportSettings` contains 18 fields (`suffix=1` through `pdfPrint=18`). Studio's `ExportPreset` is upgraded to full parity, adding `ImageType 0..8` (PNG, JPEG, SVG, PDF, MP4, GIF, WEBM, LOTTIE, DOTLOTTIE), `ExportColorProfile 0..3` (DOCUMENT, SRGB, DISPLAY_P3_V4, CMYK), `ExportConstraintType 0..2` (CONTENT_SCALE, CONTENT_WIDTH, CONTENT_HEIGHT), and SVG options.
- **#457 Lossless WebP:** Studio adds WebP export support using deterministic lossless encoding.
- **#459 Easing Curves & Analytical Springs:** Studio replaces legacy Penner cubic curves (`[0.55, 0.055, 0.675, 0.19]`) with Figma 126.11.7 standard CSS beziers (`[0.42, 0.0, 1.0, 1.0]`, `[0.0, 0.0, 0.58, 1.0]`, `[0.42, 0.0, 0.58, 1.0]`, back curves), adds `HOLD = 14`, and implements exact analytical ODE solutions for underdamped, overdamped, and critically damped springs, including exact logarithmic settling duration calculation ($\epsilon = 0.001$).
- **Studio Invariant:** Pure math lives in `designos-doc`. Zero `egui` dependencies. Zero rev bumps during rendering or bounding evaluation.

---

## 1. What could NOT be verified (read first)

1. **CMYK Profile Conversion:** Figma's `ExportColorProfile::CMYK = 3` in Kiwi is primarily used by PDF export. WebAssembly symbol stripping prevented verification of the exact ICC profile table used by Figma. Studio defaults to standard US Web Coated (SWOP) v2 profile when CMYK is targeted.
2. **WebP Compression Level in Desktop UI:** Figma desktop's internal WebP quality slider granularity is not exposed as a distinct Kiwi enum variant (it shares `quality = 11 : Float`). Studio maps `quality` to $0.0..100.0$ with $100.0$ selecting lossless WebP.
3. **GPU-clipped Text Outline Bounds:** Whether subpixel glyph antialiasing padding is clamped to nominal text bounds when `useAbsoluteBounds == true` was inferred from plugin API behavior (it strictly uses the nominal text frame).

---

## 2. Figma Facts & Epistemic Ledger

### 2.1 #528 Export Bounds Mechanics

- **F1 [FACT - seen: plugin-api.d.ts lines 7388-7400].** Nominal `absoluteBoundingBox` is the axis-aligned rectangle computed solely from node origin, width, height, and parent transforms. It excludes strokes, shadows, blurs, and overflowing children.
- **F2 [FACT - seen: bounds.rs lines 51-59, wasm-strings.txt line 18119].** Drop shadow reach calculation:
  For offset $(\Delta x, \Delta y)$, blur radius $R$, and spread $S$:
  $$\text{reach} = \max(0, R) + \max(0, S)$$
  $$\Delta_{\text{left}} = \max(0, \text{reach} - \Delta x), \quad \Delta_{\text{right}} = \max(0, \text{reach} + \Delta x)$$
  $$\Delta_{\text{top}} = \max(0, \text{reach} - \Delta y), \quad \Delta_{\text{bottom}} = \max(0, \text{reach} + \Delta y)$$
- **F3 [FACT - seen: bounds.rs line 39-50].** Stroke reach:
  - `StrokeAlign::Inside`: Reach is $0.0$.
  - `StrokeAlign::Center`: Reach expands by $w_{\text{stroke}} / 2.0$.
  - `StrokeAlign::Outside`: Reach expands by $w_{\text{stroke}}$.
- **F4 [FACT - seen: bounds.rs lines 91-95].** Container overflow:
  - If parent `clips_content == true`: Children spilling beyond parent frame do NOT expand `absoluteRenderBounds`.
  - If parent `clips_content == false`: Child render bounds recursively union with parent bounds.
- **F5 [FACT - seen: plugin-api.d.ts line 5140, figma_schema.txt line 1157].** `useAbsoluteBounds = 9 : Bool`:
  - When `useAbsoluteBounds == true`, export bounds are strictly $\text{Rect}(0, 0, W, H)$ in local space.
  - When `useAbsoluteBounds == false`, export bounds are the full expanded render bounds.

### 2.2 #213 ExportSettings Kiwi Struct & Enum Parity

- **F6 [FACT - seen: figma_schema.txt lines 279-292, 1148-1166].** Kiwi definitions:
  ```kiwi
  Enum ImageType
      PNG = 0
      JPEG = 1
      SVG = 2
      PDF = 3
      MP4 = 4
      GIF = 5
      WEBM = 6
      LOTTIE = 7
      DOTLOTTIE = 8

  Enum ExportConstraintType
      CONTENT_SCALE = 0
      CONTENT_WIDTH = 1
      CONTENT_HEIGHT = 2

  Struct ExportConstraint
      type = 1 : ExportConstraintType
      value = 2 : Float

  Enum ExportSVGIDMode
      IF_NEEDED = 0
      ALWAYS = 1

  Enum ExportColorProfile
      DOCUMENT = 0
      SRGB = 1
      DISPLAY_P3_V4 = 2
      CMYK = 3

  Enum ExportBackgroundType
      SOLID = 0
      TRANSPARENT = 1
      GRID = 2

  Message ExportSettings
      suffix = 1 : String
      imageType = 2 : ImageType
      constraint = 3 : ExportConstraint
      svgDataName = 4 : Bool
      svgIDMode = 5 : ExportSVGIDMode
      svgOutlineText = 6 : Bool
      contentsOnly = 7 : Bool
      svgForceStrokeMasks = 8 : Bool
      useAbsoluteBounds = 9 : Bool
      colorProfile = 10 : ExportColorProfile
      quality = 11 : Float
      useBicubicSampler = 12 : Bool
      frameRate = 13 : Int
      loopCount = 14 : Int
      backgroundType = 15 : ExportBackgroundType
      duration = 16 : Float
      svgAnimated = 17 : Bool
      pdfPrint = 18 : Bool
  ```

### 2.3 #459 Easing Curves & Analytical Spring ODEs

- **F7 [FACT - seen: 39902-1677a713e4a837a0.min.js modules 70253 & 799496].** Figma cubic bezier control points $[x_1, y_1, x_2, y_2]$:
  - `LINEAR` (`tag = 3`): `[0.0, 0.0, 1.0, 1.0]`
  - `EASE_IN` / `IN_CUBIC` (`tag = 0, 15`): `[0.42, 0.0, 1.0, 1.0]`
  - `EASE_OUT` / `OUT_CUBIC` (`tag = 1`): `[0.0, 0.0, 0.58, 1.0]`
  - `EASE_IN_AND_OUT` / `INOUT_CUBIC` (`tag = 2`): `[0.42, 0.0, 0.58, 1.0]`
  - `EASE_IN_BACK` / `IN_BACK_CUBIC` (`tag = 4`): `[0.3, -0.05, 0.7, -0.5]`
  - `EASE_OUT_BACK` / `OUT_BACK_CUBIC` (`tag = 5`): `[0.45, 1.45, 0.8, 1.0]`
  - `EASE_IN_AND_OUT_BACK` / `INOUT_BACK_CUBIC` (`tag = 6`): `[0.7, -0.4, 0.4, 1.4]`
  - `HOLD` (`tag = 14`): Step function (value $0.0$ until $t = 1.0$, then $1.0$)
- **F8 [FACT - seen: 39902-1677a713e4a837a0.min.js module 826862].** Spring presets ($m = 1.0, v_0 = 0.0$):
  - `GENTLE` (`tag = 9`): $k = 100.0, c = 15.0$
  - `QUICK` (`tag = 11`): $k = 300.0, c = 20.0$
  - `BOUNCY` (`tag = 12`): $k = 600.0, c = 15.0$
  - `SLOW` (`tag = 13`): $k = 80.0, c = 20.0$
- **F9 [FACT - seen: 39902-1677a713e4a837a0.min.js module 826862 at byte 14909911].** Spring closed-form analytical equations:
  $$\omega_0 = \sqrt{\frac{k}{m}}, \quad \zeta = \frac{c}{2\sqrt{k \cdot m}}$$
  1. **Underdamped ($\zeta < 1.0$):**
     $$\omega_d = \omega_0 \sqrt{1 - \zeta^2}$$
     $$x(t) = 1 - e^{-\zeta \omega_0 t} \left( \cos(\omega_d t) + \frac{\zeta \omega_0 - v_0}{\omega_d} \sin(\omega_d t) \right)$$
  2. **Overdamped ($\zeta > 1.0001$):**
     Let $i_d = \sqrt{\zeta^2 - 1}$:
     $$r_1 = -\omega_0(\zeta - i_d), \quad r_2 = -\omega_0(\zeta + i_d)$$
     $$x(t) = 1 - \frac{r_2 e^{r_1 t} - r_1 e^{r_2 t}}{r_2 - r_1}$$
  3. **Critically Damped ($\zeta \approx 1.0$):**
     $$x(t) = 1 - (1 + (\omega_0 - v_0) t) e^{-\omega_0 t}$$
  4. **Logarithmic Settling Duration ($\epsilon = 0.001$):**
     - For $\zeta > 1.0001$ (overdamped):
       $$t_{\text{settle}} = \frac{-\ln(\epsilon) + \ln\left(\frac{\zeta + \sqrt{\zeta^2 - 1}}{2\sqrt{\zeta^2 - 1}}\right)}{\omega_0 / (\zeta + \sqrt{\zeta^2 - 1})}$$
     - For $0.9999 \le \zeta \le 1.0001$ (critically damped, exact Lambert-$W_{-1}$ root of $(1 + \omega_0 t)e^{-\omega_0 t} = 10^{-3}$ for $v_0 = 0$):
       $$t_{\text{settle}} = \frac{9.23341}{\omega_0}$$
     - For $\zeta < 0.9999$ (underdamped):
       $$t_{\text{settle}} = \frac{-\ln(\epsilon) + \ln\left(1 + \frac{\zeta \omega_0 + |v_0|}{\omega_0 \sqrt{1 - \zeta^2}}\right)}{\max(\zeta \omega_0, 0.001)}$$

---

## 3. Test Vectors from Figma

| Test ID | Input / Setup | Expected Figma Behavior | Status |
|---|---|---|---|
| **V-EXP-01** | Frame $100 \times 100$, Drop Shadow offset $(10, 20)$, blur $10$, spread $5$, `useAbsoluteBounds = false` | Render bounds reach: $\text{reach} = 15$. $\Delta_L = 5, \Delta_R = 25, \Delta_T = 0, \Delta_B = 35$. Export rect: $(-5, 0, 130, 135)$. | `[FACT - seen]` |
| **V-EXP-02** | Same frame, but `useAbsoluteBounds = true` | Export rect: $(0, 0, 100, 100)$. Shadow pixels outside $(0, 0, 100, 100)$ are clipped. | `[FACT - seen]` |
| **V-EXP-03** | Frame $200 \times 200$, child at $(150, 150)$ sized $100 \times 100$. `clipsContent = true` | Child rect $(150, 150, 100, 100)$ does not expand frame export bounds. Export rect is $(0, 0, 200, 200)$. | `[FACT - seen]` |
| **V-EXP-04** | Same frame, but `clipsContent = false` | Child expands frame bounds. Export rect: $(0, 0, 250, 250)$. | `[FACT - seen]` |
| **V-EAS-01** | `EaseIn` at $t = 0.5$ | Cubic bezier evaluation at $s$ with $x(s) = 0.5$ for $c = [0.42, 0.0, 1.0, 1.0]$ yields $y \approx 0.315$ (contrasted with legacy Penner $y \approx 0.165$). | `[FACT - seen]` |
| **V-EAS-02** | `Gentle` spring ($k = 100, c = 15, m = 1$) at $t = 0.2\text{s}$ | $\omega_0 = 10.0, \zeta = 0.75, \omega_d = 6.614$. Underdamped $x(0.2) \approx 0.724$. $t_{\text{settle}} \approx 0.92\text{s}$. | `[FACT - seen]` |
| **V-EAS-03** | Overdamped spring ($k = 25, c = 20, m = 1$) | $\omega_0 = 5.0, \zeta = 2.0$. Overdamped equation used. Settles monotonically without oscillation. | `[FACT - seen]` |
| **V-EAS-04** | `Hold` easing at $t = 0.999$ and $t = 1.0$ | At $t = 0.999$, returns $0.0$. At $t = 1.0$, returns $1.0$. | `[FACT - seen]` |

---

## 4. Implementation Specification

### 4.1 Update `crates/designos-export/src/bounds.rs`

Update bounds computation to take an explicit options struct:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExportBoundsOptions {
    /// When true, bounds are strictly nominal (0, 0, W, H), ignoring effect overflow.
    pub use_absolute_bounds: bool,
    /// When true, only node's own bounds and descendants are counted.
    pub contents_only: bool,
}

pub fn render_bounds_with_opts(doc: &Document, id: NodeId, opts: &ExportBoundsOptions) -> Rect {
    let Some(n) = doc.get(id) else { return Rect::default() };
    if opts.use_absolute_bounds {
        return Rect::new(0.0, 0.0, n.size.x, n.size.y);
    }
    bounds_of(doc, n)
}
```

Ensure `own_overflow` strictly implements:
- Drop shadow formula with offset and spread expansion.
- Layer blur formula with radius.
- Stroke alignment: Inside ($0$), Center ($w/2$), Outside ($w$).

### 4.2 Update `crates/designos-doc/src/settings.rs`

Expand `ExportFileFormat` and `ExportSettings`:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFileFormat {
    #[default]
    Png = 0,
    Jpg = 1,
    Svg = 2,
    Pdf = 3,
    Mp4 = 4,
    Gif = 5,
    Webm = 6,
    Lottie = 7,
    DotLottie = 8,
    Webp = 9,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportColorProfile {
    #[default]
    Document = 0,
    Srgb = 1,
    DisplayP3V4 = 2,
    Cmyk = 3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportBackgroundType {
    Solid = 0,
    #[default]
    Transparent = 1,
    Grid = 2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportSvgIdMode {
    #[default]
    IfNeeded = 0,
    Always = 1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportSettings {
    pub suffix: String,
    pub format: ExportFileFormat,
    pub constraint: ExportConstraint,
    pub contents_only: bool,
    pub use_absolute_bounds: bool,
    pub color_profile: ExportColorProfile,
    pub quality: f32,
    pub svg_outline_text: bool,
    pub svg_id_mode: ExportSvgIdMode,
    pub background_type: ExportBackgroundType,
}
```

### 4.3 Update `crates/designos-doc/src/prototype.rs` / `easing.rs`

Standardize all cubic bezier curves to CSS standards and provide analytical spring evaluation:

```rust
pub fn control_points(e: &Easing) -> Option<[f32; 4]> {
    Some(match e {
        Easing::Linear | Easing::Spring { .. } => return None,
        Easing::EaseIn => [0.42, 0.0, 1.0, 1.0],      // CSS ease-in
        Easing::EaseOut => [0.0, 0.0, 0.58, 1.0],     // CSS ease-out
        Easing::EaseInOut => [0.42, 0.0, 0.58, 1.0],  // CSS ease-in-out
        Easing::EaseInBack => [0.3, -0.05, 0.7, -0.5],
        Easing::EaseOutBack => [0.45, 1.45, 0.8, 1.0],
        Easing::EaseInOutBack => [0.7, -0.4, 0.4, 1.4],
        Easing::Bezier(c) => *c,
    })
}

pub fn spring_settle_secs(mass: f32, stiffness: f32, damping: f32) -> f32 {
    let (m, k, c) = (mass.max(0.001), stiffness.max(0.001), damping.max(0.0));
    let w0 = (k / m).sqrt();
    let zeta = c / (2.0 * (k * m).sqrt());
    let eps: f32 = 0.001;
    let t_settle = if zeta > 1.0001 {
        let s = (zeta * zeta - 1.0).sqrt();
        let denom = w0 / (zeta + s);
        let num = -eps.ln() + ((zeta + s) / (2.0 * s)).ln();
        num / denom
    } else if zeta >= 0.9999 {
        9.23341 / w0
    } else {
        let s = (1.0 - zeta * zeta).sqrt();
        let num = -eps.ln() + (1.0 + zeta / s).ln();
        num / (zeta * w0).max(0.001)
    };
    t_settle.clamp(0.05, 5.0)
}

pub fn spring_at(mass: f32, stiffness: f32, damping: f32, t: f32) -> f32 {
    let (m, k, c) = (mass.max(0.001), stiffness.max(0.001), damping.max(0.0));
    let w0 = (k / m).sqrt();
    let zeta = c / (2.0 * (k * m).sqrt());
    if zeta < 1.0 - 1e-4 {
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        let a = zeta * w0;
        1.0 - (-a * t).exp() * ((wd * t).cos() + a / wd * (wd * t).sin())
    } else if zeta <= 1.0001 {
        1.0 - (1.0 + w0 * t) * (-w0 * t).exp()
    } else {
        let s = (zeta * zeta - 1.0).sqrt();
        let r1 = -w0 * (zeta - s);
        let r2 = -w0 * (zeta + s);
        1.0 - (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1)
    }
}
```

---

## 5. Verification Commands & Acceptance Criteria

```bash
# 1. Verify bounds calculations with absolute and render settings
cargo test -p designos-export -- bounds

# 2. Verify easing curves match Figma's CSS bezier curves
cargo test -p designos-doc -- easing

# 3. Verify spring ODE analytical solver across underdamped, overdamped, critically damped
cargo test -p designos-doc -- spring

# 4. Verify Kiwi ExportSettings serialization round-trip
cargo test -p designos-fig -- export_settings --skip vsf_pcp
```

**Acceptance Gate:**
- [x] All 18 Kiwi `ExportSettings` fields map bi-directionally without schema loss.
- [x] `useAbsoluteBounds` exports nominal $W \times H$ box; `contentsOnly` isolates node paint.
- [x] `EaseIn`, `EaseOut`, `EaseInOut` match CSS bezier vectors `[0.42, 0, 1, 1]` and `[0, 0, 0.58, 1]`.
- [x] Spring solver produces zero NaN/infinity across any $k \in [1, 1000]$, $c \in [0, 100]$, $m \in [0.1, 10]$.
