> **Review status (2026-11-12): READY AFTER FIXES.** Reference for implementation; the schema work is well grounded for slices 1 to 4; the preview server and the WOFF2 encoder need an owner decision. Corrections: [review](../reviews/review-e8-to-e13.md#e13-sites).

# E13.1–E13.5 Sites, Breakpoints, Scroll Effects, CMS & Zero-JS Compiler Spec

Date: 2026-11-12 | Milestone: v0.8.0 | Base: `origin/main@e0e44f3c2`
Scope: Epic #125 Slices 1–5 (Sites Mode, Responsive Breakpoints, Scroll Behaviors, CMS Collections, Semantic HTML/CSS Compiler).
Evidence: REA Dossier `evidence/rea-e13-e7-e10-e20-sites-dev-branch-io.md` (Figma 126.11.7 binary & Kiwi schema ground truth). Clean-room: own words, struct definitions, algorithms and offsets only.

---

## Answer first

- **Figma Sites architecture is fundamentally multi-breakpoint inheritance.** A website page is represented by a `WEBPAGE` node (`NodeType = 55`) residing directly under `CANVAS` (`NodeType = 2`) [FACT - seen: `wasm-strings.txt#L44661`]. It contains a `RESPONSIVE_SET` (`39`) holding 1 to 4 breakpoint frames (typically Desktop `1440px`, Tablet `768px`, Mobile `375px`).
- **Secondary breakpoints inherit via differential overrides, not separate frames.** The `Desktop` breakpoint frame has `isPrimaryBreakpoint = true` (`458`). Secondary breakpoints (`Tablet`, `Mobile`) have `isPrimaryBreakpoint = false`, set `primaryResponsiveNodeId = 457` to the Desktop frame GUID, and store layout/styling variations in `derivedBreakpointData = 500 : DerivedBreakpointData` [FACT - seen: schema L92-96]. Studio will implement this by reusing its existing `NodeChange` override resolution engine.
- **Rich Interactive Behaviors are declarative `NodeBehaviors`.** Scroll animations do not inject arbitrary JavaScript; they serialize as `appear` (`AppearBehavior`), `scrollParallax` (`ScrollParallaxBehavior`), `scrollTransform` (`ScrollTransformBehavior`), `marquee` (`MarqueeBehavior`), and `STICKY_SCROLLS = 2` (`ScrollBehavior`) [FACT - seen: schema L138-234].
- **Zero-JS-by-Default Compiler**: Studio's `designos-export` compiler converts breakpoints into modern CSS container and media queries (`@media (max-width: 1199px)`, `@media (max-width: 767px)`), CSS `position: sticky`, `@keyframes` animations, and pure CSS marquees, producing blazing fast, SEO-perfect static sites.
- **Local Live-Reload Preview**: Binds `127.0.0.1:0` with an ephemeral token and SSE (Server-Sent Events) live-reload trigger, terminating automatically upon tab close.

---

## 1. What could NOT be verified (read first)

1. **Complex CMS Rich Text Formatting Inlines**: While `CMSRichTextStyleMap` defines style classes for headings (`0..5`), paragraphs (`6`), links (`7`), and blockquotes (`8`) [FACT - seen: schema L282-292], Figma's custom AST for inline tokens (bold/italic spans mixed with dynamic CMS variables) within a single rich text block is serialized in an opaque binary blob. Studio handles this via standard Markdown/HTML tokenization.
2. **Dynamic Pagination & Real-time Server Search**: Figma Sites cloud deployment relies on Cloudflare Workers / Fastly Compute for dynamic query string filtering (`?sort=price&page=2`). Studio's compiler generates static, client-side pre-rendered pages with deterministic multi-page routing.
3. **Internal Dakota Fluid Typography Curve**: `ResponsiveSetSettings` defines `scalingMinFontSize` and `scalingMaxFontSize` alongside `scalingMinLayoutWidth` and `scalingMaxLayoutWidth` [FACT - seen: schema L107-110]. The exact interpolation curve (linear vs non-linear viewport clamping) is inferred to match CSS `clamp(min, preferred, max)`.

---

## 2. Figma Ground-Truth Schema & Enums

### 2.1 Editor & Node Types
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L79-L105`, `L4104-L4114`]
```text
Enum EditorType:
    DESIGN = 0
    WHITEBOARD = 1
    SLIDES = 2
    DEV_HANDOFF = 3
    SITES = 4

NodeType:
    RESPONSIVE_SET = 39
    WEBPAGE = 55
    RESPONSIVE_NODE_SET = 54
    CMS_RICH_TEXT = 49
    REPEATER = 50
```

### 2.2 Breakpoint Inheritance Fields in `NodeChange`
[FACT - seen: `figma-real-schema.txt#L1050-L1053`, `L2220-L2226`]
- Field `457`: `primaryResponsiveNodeId : GUID`
- Field `458`: `isPrimaryBreakpoint : Bool`
- Field `462`: `multiEditGlueId : GUID`
- Field `500`: `derivedBreakpointData : DerivedBreakpointData`
- Field `501`: `breakpointMinWidth : Float`
- Field `522`: `isBreakpointInFocus : Bool`
- Field `398`: `defaultResponsiveSetId : GUID`

```text
Message DerivedBreakpointData:
    overrides = 1 : NodeChange[]
```

### 2.3 Page & Site Settings (`ResponsiveSetSettings`)
[FACT - seen: `figma-real-schema.txt#L2455-L2479`]
```text
Enum ResponsiveScalingMode:
    REFLOW = 0
    SCALE = 1

Message ResponsiveSetSettings:
    title = 1 : String
    description = 2 : String
    scalingMode = 3 : ResponsiveScalingMode
    scalingMinFontSize = 4 : Float
    scalingMaxFontSize = 5 : Float
    scalingMinLayoutWidth = 6 : Float
    scalingMaxLayoutWidth = 7 : Float
    lang = 8 : String
    faviconHash = 9 : String
    socialImageHash = 10 : String
    googleAnalyticsID = 11 : String
    blockSearchIndexing = 12 : Bool
    customCodeHeadStart = 13 : String
    customCodeHeadEnd = 14 : String
    customCodeBodyStart = 15 : String
    customCodeBodyEnd = 16 : String
    faviconID = 17 : GUID
    socialImageID = 18 : GUID
    addBypassLinks = 19 : Bool
    ignoreReducedMotion = 20 : Bool
    cookieBanner = 21 : CookieBannerSettings
```

### 2.4 Behaviors Schema (`NodeBehaviors`)
[FACT - seen: `figma-real-schema.txt#L4428-L4510`]
```text
Message NodeBehaviors:
    link = 1 : LinkBehavior
    appear = 2 : AppearBehavior
    hover = 3 : HoverBehavior
    press = 4 : PressBehavior
    focus = 5 : FocusBehavior
    scrollParallax = 6 : ScrollParallaxBehavior
    scrollTransform = 7 : ScrollTransformBehavior
    cursor = 8 : CursorBehavior
    marquee = 9 : MarqueeBehavior
    code = 10 : CodeBehavior[]

Enum AppearBehaviorTrigger:
    PAGE_LOAD = 1
    THIS_LAYER_IN_VIEW = 2
    OTHER_LAYER_IN_VIEW = 3
    SCROLL_DIRECTION = 4

Enum RelativeDirection:
    UP = 1
    DOWN = 2
    LEFT = 3
    RIGHT = 4

Message ScrollParallaxBehavior:
    axis = 1 : ScrollDirection (NONE=0, HORIZONTAL=1, VERTICAL=2, BOTH=3)
    speed = 2 : Float
    relativeToPage = 3 : Bool
    speedVar = 4 : VariableData
    isDeleted = 5 : Bool

Message MarqueeBehavior:
    direction = 1 : RelativeDirection
    speed = 2 : Float
    shouldLoopInfinitely = 3 : Bool
    speedVar = 4 : VariableData
    shouldLoopInfinitelyVar = 5 : VariableData
    pauseOnHover = 6 : VariableData
    isDeleted = 7 : Bool
```

---

## 3. Studio Engine Architecture & Types

### 3.1 Data Model: `crates/designos-doc/src/site.rs`

```rust
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponsiveScalingMode {
    Reflow,
    Scale,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResponsiveSetSettings {
    pub title: String,
    pub description: String,
    pub scaling_mode: ResponsiveScalingMode,
    pub scaling_min_font_size: Option<f32>,
    pub scaling_max_font_size: Option<f32>,
    pub scaling_min_layout_width: Option<f32>,
    pub scaling_max_layout_width: Option<f32>,
    pub lang: Option<String>,
    pub favicon_hash: Option<String>,
    pub social_image_hash: Option<String>,
    pub google_analytics_id: Option<String>,
    pub block_search_indexing: bool,
    pub custom_code_head_start: Option<String>,
    pub custom_code_head_end: Option<String>,
    pub custom_code_body_start: Option<String>,
    pub custom_code_body_end: Option<String>,
    pub add_bypass_links: bool,
    pub ignore_reduced_motion: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BreakpointConfig {
    pub min_width: f32,
    pub is_primary: bool,
    pub primary_node_id: Option<NodeId>,
    pub overrides: Vec<NodeOverride>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeOverride {
    pub target_id: NodeId,
    pub property_path: String,
    pub value: serde_json::Value,
}
```

### 3.2 Behavior Model: `crates/designos-doc/src/behaviors.rs`

```rust
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppearTrigger {
    PageLoad,
    ThisLayerInView,
    OtherLayerInView,
    ScrollDirection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelativeDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppearBehavior {
    pub trigger: AppearTrigger,
    pub direction: RelativeDirection,
    pub other_layer: Option<NodeId>,
    pub distance: f32,
    pub duration_seconds: f32,
    pub delay_seconds: f32,
    pub plays_once: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScrollParallaxBehavior {
    pub horizontal: bool,
    pub vertical: bool,
    pub speed: f32, // e.g. -0.5 to 1.5 multiplier
    pub relative_to_page: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MarqueeBehavior {
    pub direction: RelativeDirection,
    pub speed_pixels_per_second: f32,
    pub infinite_loop: bool,
    pub pause_on_hover: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeBehaviors {
    pub appear: Option<AppearBehavior>,
    pub parallax: Option<ScrollParallaxBehavior>,
    pub marquee: Option<MarqueeBehavior>,
    pub sticky: bool, // derived from ScrollBehavior::STICKY_SCROLLS
}
```

---

## 4. Breakpoint Inheritance Resolution Algorithm

When rendering or exporting a secondary breakpoint (e.g. `Mobile` at 375px):
1. Locate the Primary breakpoint frame via `primary_node_id`.
2. Clone the Primary frame's scene subtree into an ephemeral resolved node tree.
3. Apply each override in `derived_breakpoint_data.overrides`:
   - Match node by stable `GUID` / `NodeId`.
   - Update modified fields: layout direction (`Horizontal` -> `Vertical`), frame sizing (`Fixed(1440)` -> `Hug` or `Fill`), typography font size (`48px` -> `32px`), or visibility (`is_visible: false` to hide desktop-only navigation).
4. Relayout the resolved subtree with Studio's layout engine.
5. Invariant check: Layout does NOT mutate the original Primary breakpoint on disk or canvas unless explicitly edited in "Multi-edit" mode (`multiEditGlueId`).

---

## 5. Zero-JS Semantic HTML/CSS Compiler (`designos-export`)

### 5.1 Compiler Architecture
The compiler generates a single clean directory structure:
```text
dist/
├── index.html
├── assets/
│   ├── fonts/ (WOFF2, font-display: swap)
│   ├── images/ (WebP compressed, lazy loaded with width/height attributes)
│   └── site.css
```

### 5.2 Responsive CSS Emission
The compiler extracts common layout and color tokens into `:root` and outputs targeted media queries:
```css
/* Primary Breakpoint: Desktop (>= 1200px) */
.page-container {
  max-width: 1440px;
  margin: 0 auto;
}

/* Secondary Breakpoint: Tablet (768px - 1199px) */
@media (max-width: 1199px) {
  .hero-section {
    flex-direction: column;
    padding: 32px 16px;
  }
  .hero-title {
    font-size: clamp(2rem, 4vw, 2.5rem);
  }
}

/* Secondary Breakpoint: Mobile (< 768px) */
@media (max-width: 767px) {
  .nav-desktop {
    display: none;
  }
  .nav-mobile-toggle {
    display: block;
  }
}
```

### 5.3 Pure CSS Scroll & Animation Emission
- **Sticky Elements**: Emits `position: sticky; top: 0; z-index: 100;`.
- **Appear on Scroll**: Emits CSS `@keyframes fadeInUp` combined with modern `animation-timeline: view()` or graceful intersection fallback.
- **Marquee / Ticker**: Emits infinite CSS translation:
```css
@keyframes marquee-left {
  0% { transform: translateX(0%); }
  100% { transform: translateX(-50%); }
}
.marquee-track {
  display: flex;
  overflow: hidden;
  user-select: none;
  animation: marquee-left 20s linear infinite;
}
.marquee-track:hover {
  animation-play-state: paused;
}
```

---

## 6. Live-Reload Preview Server Specification

- Server binds to `127.0.0.1:0`. The OS automatically assigns an unused port (e.g. `127.0.0.1:54321`).
- Generates a cryptographically random 16-byte hex bearer token on startup. All requests must provide `?token=<token>` or `Authorization: Bearer <token>`; unauthorized requests return `401 Unauthorized`.
- Connects an SSE endpoint at `GET /__designos_live_reload`.
- Whenever `session.rs` detects document edits, an SSE event `event: reload\ndata: ok\n\n` is published.
- The webview or browser window reconnects automatically without full page reload if only CSS changed.
- Shuts down cleanly immediately upon closing the document tab or quitting the application.

---

## 7. Machine-Checkable Verification Matrix

| Test ID | Vector / Scenario | Input Condition | Expected Output / Invariant |
|---|---|---|---|
| **V-SITES-01** | Kiwi Schema Round-Trip | `.fig` with `RESPONSIVE_SET (39)` and `WEBPAGE (55)` | Parsed without errors; node types match schema numbers exactly. |
| **V-SITES-02** | Breakpoint Override Cascade | Change primary title color on Desktop | Mobile title reflects new color unless mobile has an explicit color override. |
| **V-SITES-03** | Sticky Header Emission | Layer with `scrollBehavior = STICKY_SCROLLS (2)` | HTML/CSS compiler emits `position: sticky; top: 0px`. |
| **V-SITES-04** | Marquee CSS Generation | Marquee behavior: `direction = LEFT, speed = 100px/s` | CSS animation emitted with computed duration matching pixel speed and track width. |
| **V-SITES-05** | Reduced Motion Respect | `ignoreReducedMotion = false` | Emits `@media (prefers-reduced-motion: reduce) { animation: none !important; }`. |
| **V-SITES-06** | Preview Server Security | Fetch `http://127.0.0.1:<port>/` without token | Returns HTTP 401; access forbidden. |
