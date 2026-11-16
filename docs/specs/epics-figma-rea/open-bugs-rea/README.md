# True Figma Bug Resolution Spec Suite: Open Issues REA Index & Triage Guide

**Status:** Build-Ready Clean-Room Architecture  
**Target Engine:** Figma Desktop 126.11.7 (macOS ARM64, Wasm Core & Rspack UI Shell) vs. DESIGN:OS Studio (`design-os-studio`)  
**Epistemic Standard:** Clean-Room Reverse Engineering (`morluto/rea v6.8.0`). Zero proprietary Figma source code.  
**Live Source Anchoring:** Git revision `origin/main` (`f2deb8d50`)  
**Target Output Path:** `plans/261106-2245-epic-waves/specs/open-bugs-rea/README.md` (published to `docs/specs/epics-figma-rea/open-bugs-rea/README.md`)

---

## 1. Executive Summary & Repository-Wide Issue Audit

To achieve bit-for-bit rendering, serialization, and interaction parity between **Figma Desktop 126.11.7** and **DESIGN:OS Studio**, a complete static and dynamic audit was performed across all **570 Open Issues** in the repository.

### 1.1 The 4-Bucket Triage Architecture

Every open issue in `design-os-studio` falls unambiguously into one of four architectural buckets. This triage explains with mathematical clarity why Reverse Engineering Analysis (REA) was or was not required for each issue:

```
Total Open Issues: 570
├── Open Epics: 24 (E1–E21 + Architecture Roadmaps)
└── Non-Epic Open Issues: 546 (100.0%)
    ├── Bucket A: Covered by Existing Epic Specs ────────── 200 issues (36.6%)
    ├── Bucket B: Studio-Internal Architecture / No REA ── 282 issues (51.7%)
    ├── Bucket C: Open Figma Parity Bugs (Resolved by REA) ─ 51 issues  (9.3%)
    └── Bucket D: Figma UX Micro-Interactions ────────────── 13 issues  (2.4%)
```

```
Bucket C + D Distribution Across Clusters (56 Unique Issues / 57 Entries):
├── Cluster C1 (Components, Variants & Kiwi I/O) ────── 23 entries (22 Bucket C + #1329)
├── Cluster C2 (Auto-Layout, Text & Export Bounds) ─── 14 entries (14 Bucket C)
└── Cluster C3 (Vectors, Strokes, Masks & UX) ──────── 20 entries (15 Bucket C + 5 Bucket D)
```

| Triage Bucket | Issue Count | Share (%) | REA Required? | Description & Scope |
|---|---|---|---|---|
| **Bucket A: Epic Covered** | **200** | **36.6%** | Existing REA | Issues fully specified by Wave 1 and Wave 2 Epic specs in `docs/specs/epics-figma-rea/` (`E1`–`E21`). Includes prototyping transitions (`E11`), variable aliases (`E1`), multi-format export presets (`E6`), dev mode token extraction (`E7`), and real-time multiplayer protocol (`E21`). |
| **Bucket B: Studio Internal** | **282** | **51.7%** | **Zero REA** | Native Rust architecture, egui desktop chrome, BrainWave 2.0 theme styling, MCP stdio JSON-RPC protocol plumbing, macOS App Sandbox / notarization, cargo workspace boundaries, test infrastructure, and CI performance gates. These issues are internal engineering contracts that have no Figma counterpart. |
| **Bucket C: Parity Bugs** | **51** | **9.3%** | **100% REA** | Real-world parity bugs between Studio and Figma: `.fig` Kiwi container codec defects, nested component instance override dropping, variable-bound variant resolution, auto-layout hug clamping, text shaping settlement, export bounds shadow calculation, and vector outline tessellation. Every single issue is resolved in this suite. |
| **Bucket D: UX Parity** | **13** | **2.4%** | Selective REA | Figma desktop UX micro-interactions and tactile ergonomics. 8 issues were already specified in `e12-canvas-a11y-text/`; the remaining 5 issues (`#1109`, `#1110`, `#1111`, `#1474`, `#1730`) are resolved in `bugs-c3-vectors-strokes-masks-ux-spec.md`. |
| **Total Non-Epic Issues** | **546** | **100.0%** | — | **546 Non-Epic Issues + 24 Open Epics = 570 Total Open Issues.** |

---

## 2. Master Issue-to-Spec Lookup Table (All 56 Unique Issues)

The 51 Bucket C issues and 5 Bucket D issues form **56 unique open parity and interaction issues**. 

> **Arithmetic & Cross-Reference Note:**  
> Issue `#1329` (nested-instance shadow export bounds truncation) was originally cataloged under Export Bounds in Cluster C2 (under `#528`), but root-cause analysis proved it stems from `NodeKind::Instance` effects inheritance in `.fig` deserialization. It is fully resolved in **Cluster C1** (§3.5) and cross-referenced in **Cluster C2** (§3.2). Counting `#1329` in both clusters yields **57 total table entries** covering **56 unique issue IDs**.

| Issue ID | Cluster | Domain | Spec Document | Target Crates / Files | Core Parity Mechanism |
|---|---|---|---|---|---|
| **`#519`** | **C2** | Text Layout | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-doc::layout_solver` | Auto-width text font advance settlement and whitespace wrapping thresholds |
| **`#521`** | **C2** | Fills / Modes | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-doc::variables_apply` | Image fills in variable mode scopes & affine crop matrix preservation |
| **`#528`** | **C2** | Export Bounds | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-export::bounds` | Signed negative shadow spread and outline stroke containment in export rect |
| **`#831`** | **C2** | Export Bounds | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-export::bounds` | Clipped frame child overflow exclusion when `clips_content == true` |
| **`#953`** | **C3** | Vectors | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-render::tess` | Inscribed parametric ellipse geometry for multi-point stars and polygons |
| **`#997`** | **C2** | Auto-Layout | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-doc::layout_solver` | Discrepancy elimination between single-node and recursive subtree layout |
| **`#1109`** | **C3** | UX / Chrome | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-app::panels::right` | Right inspector sidebar resize drag, clamping (240–480px), double-click reset |
| **`#1110`** | **C3** | UX / Chrome | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-app::panels::left` | Left layers sidebar resize parity, collapse threshold & keyboard accessibility |
| **`#1111`** | **C3** | UX / Chrome | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-app::panels::layers` | Pages section header chevron collapse toggle state machine |
| **`#1329`** | **C1** | Export / Bounds | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-export`, `designos-fig` | Master component shadow inheritance on instance nodes lacking effects overrides |
| **`#1404`** | **C2** | Text Layout | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-fonts`, `designos-doc` | Text box layout settlement synchronization with asynchronous font loading |
| **`#1474`** | **C3** | UX / Chrome | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-app::canvas::context` | Layer right-click context menu action parity and keyboard shortcut hints |
| **`#1476`** | **C3** | Canvas View | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-render`, `designos-gpu` | Canvas Outlines Mode (`Cmd+Y`) and Pixel Preview (`Cmd+Opt+Y`) pipelines |
| **`#1506`** | **C3** | Masks / Render | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-render`, `designos-gpu` | Vector mask sibling scoping, nested alpha masks & stroke exclusion rules |
| **`#1621`** | **C3** | Text Stroke | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-render::text` | Elimination of outside text stroke double-painting on 128-glyph batch boundaries |
| **`#1631`** | **C3** | Image Stroke | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-render::paint` | CPU tile culling bounds correction for transformed image-pattern strokes |
| **`#1657`** | **C1** | Kiwi / Variables | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc`, `designos-fig` | Modeling `isPublishable = 174 : Bool` on `VARIABLE_SET` collections & variables |
| **`#1689`** | **C3** | Shaders / GPU | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-gpu::exec` | GPU background blur edge texel clamping to prevent dark canvas fringe artifacts |
| **`#1694`** | **C1** | Container I/O | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-fig::fig` | FigJam `.jam` (`b"fig-jam."`) & `editorType == 1` import as `DocumentKind::Board` |
| **`#1696`** | **C1** | Kiwi / Overrides | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-fig::fig_export` | Reset override of unopened instance filtered from exported `symbolOverrides` |
| **`#1697`** | **C1** | Kiwi / Layout | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-fig::fig_export` | Omit stale DSD self-transform when unopened instance was moved via MCP/API |
| **`#1698`** | **C1** | Kiwi / Variables | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-fig::fig_export` | Prune `variableConsumptionMap` entries when property is restated with static value |
| **`#1699`** | **C1** | Kiwi / Layout | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-fig::fig_export` | Digest `Layout` field changes on unopened instances to prevent export overwrite |
| **`#1700`** | **C1** | Serialization | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::node` | Isolate `.fig` own-entry flags as transient runtime state (prevent `.designos` leak) |
| **`#1709`** | **C1** | Kiwi / Board | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::board` | Retain unknown Kiwi keys on connectors/sections via scoped `extra` maps |
| **`#1730`** | **C3** | Canvas UX | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-app::canvas::handle` | Canvas resize handle drag deadzone threshold (3px) to prevent micro-jitter |
| **`#1738`** | **C2** | Export Bounds | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-export::bounds` | Rotated layer export local bounding box vs. parent-aligned AABB selection |
| **`#1748`** | **C1** | Kiwi / Text | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-fig::fig_export` | Hyperlink on text-style-linked text exported as `isOverrideOverTextStyle` entry |
| **`#1753`** | **C3** | Vectors | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-render::tess` | Invalidate baked `strokeGeometry` on non-uniform stretch of vector lines |
| **`#1754`** | **C3** | Vectors | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-doc::components` | Invalidate line stroke outlines on instance resize instead of stretching geometry |
| **`#1755`** | **C3** | Vectors | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-doc::components` | Invalidate stored stroke outlines when editing weight, caps, miter, or dashes |
| **`#1756`** | **C3** | Kiwi / Vectors | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-fig::fig_export` | Ensure `.fig` instance stroke weight overrides do not clear vector stroke bands |
| **`#1757`** | **C3** | Vectors | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-fig::vector` | Binary `VectorNetwork` decode and round-trip for dashed open strokes |
| **`#1758`** | **C3** | Booleans | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-render::boolean` | Boolean operations on 100% closed rings and open vector strokes |
| **`#1765`** | **C3** | Vectors | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-fig::vector` | Handle `[0, 0]` dash pattern arrays as solid continuous strokes |
| **`#1798`** | **C2** | Color / GPU | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-gpu::color` | Wide-Gamut Display P3 texture surface and Metal swapchain configuration |
| **`#1799`** | **C2** | Tables | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-doc::table` | Table merged cell selection grips and structural cell boundary editing |
| **`#1817`** | **C2** | Tables | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-doc::table` | Table column width persistence across MCP session serialization and replay |
| **`#1885`** | **C2** | Auto-Layout | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-doc::layout_solver` | Auto-layout wrap counter spacing coupling with `itemSpacing` when set to Auto |
| **`#1887`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::lib` | On-demand hydration (`ensure_component_hydrated`) for instances on deferred pages |
| **`#1888`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::document` | Stale DSD invalidation via master `content_hash` check on deferred page open |
| **`#1895`** | **C2** | Auto-Layout | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-doc::layout_solver` | FILL sizing and min/max clamping resolution on rotated auto-layout children |
| **`#1907`** | **C1** | Migration | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::lib` | Pre-0.7.13 migration synthesizing `OverrideField::ArcData` on ellipse ring copies |
| **`#1909`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::components` | Preserving authored relative `guidPath` identity across nested variant swaps |
| **`#1915`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::components` | Inactive override stashing (`InstanceOverrideStashV2`) for asymmetric layers |
| **`#1916`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::components` | Text override retention across variant switches when master defaults differ |
| **`#1917`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::components` | Child layout constraints re-evaluation when resized instance switches variant |
| **`#1918`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::components` | Dynamic text-box re-measurement on auto-resize text overrides before Hug layout |
| **`#1919`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::components` | Short-circuiting no-change `sync_main` layout pass via master `content_hash` |
| **`#1920`** | **C2** | Auto-Layout | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-doc::layout_solver` | Untitled UI PRO v8.0 census resolution: Hug clamping (473 layers) & Wrap slack (16) |
| **`#1925`** | **C1** | Kiwi / Overrides | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-fig::fig` | Multi-hop `guidPath` resolution per inheritance level (fixing 321k mislinks) |
| **`#1935`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::components` | Invalidate master baked `fill_geometry` when instance overrides `cornerRadius` |
| **`#1936`** | **C3** | Vectors | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-doc::components` | Invalidate stale vector outlines on Paste Properties and style matching |
| **`#1937`** | **C3** | DevMode / MCP | [Cluster C3 Spec](./bugs-c3-vectors-strokes-masks-ux-spec.md) | `designos-mcp::spec` | MCP `designos_export_spec` serialization of instance ring arc geometry |
| **`#1942`** | **C2** | Auto-Layout | [Cluster C2 Spec](./bugs-c2-layout-text-export-drift-spec.md) | `designos-doc::layout_solver` | Live Figma verification: hugging height clamping, wrap line pitch & single child |
| **`#1943`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-app`, `designos-doc` | Variable-bound variant runtime checks: overruling state, pill UI, Hug relayout |
| **`#1944`** | **C1** | Components | [Cluster C1 Spec](./bugs-c1-components-variants-fig-io-spec.md) | `designos-doc::components` | Live Figma verification: asymmetric layer stash, text retention, constraint sync |

---

## 3. Structure of the Cluster Specs

The specifications are organized into three self-contained documents:

1. **[Cluster C1 Specification](./bugs-c1-components-variants-fig-io-spec.md)**:
   - **Issues Covered (23 entries)**: `#1329`, `#1657`, `#1694`, `#1696`, `#1697`, `#1698`, `#1699`, `#1700`, `#1709`, `#1748`, `#1887`, `#1888`, `#1907`, `#1909`, `#1915`, `#1916`, `#1917`, `#1918`, `#1919`, `#1925`, `#1935`, `#1943`, `#1944`.
   - **Core Focus**: Component systems, variant switches, `overrideStashV2`, multi-hop `guidPath` resolution, lazy hydration, and `.fig` container / Kiwi round-trip fidelity.
   - **Evidence Dossier**: `../evidence/rea-bugs-c1-components-variants-fig-io.md` (published to `../evidence/rea-bugs-c1-components-variants-fig-io.md`).

2. **[Cluster C2 Specification](./bugs-c2-layout-text-export-drift-spec.md)**:
   - **Issues Covered (14 entries)**: `#519`, `#521`, `#528`, `#831`, `#997`, `#1404`, `#1738`, `#1798`, `#1799`, `#1817`, `#1885`, `#1895`, `#1920`, `#1942`.
   - **Core Focus**: Auto-layout solver fidelity (hugging clamp, wrap slack, legacy space-between), text shaping and settlement, table cell structures, wide-gamut Display P3, and export bounding boxes.
   - **Evidence Dossier**: `../evidence/rea-bugs-c2-layout-text-export-drift.md` (published to `../evidence/rea-bugs-c2-layout-text-export-drift.md`).

3. **[Cluster C3 Specification](./bugs-c3-vectors-strokes-masks-ux-spec.md)**:
   - **Issues Covered (20 entries)**: `#953`, `#1109`, `#1110`, `#1111`, `#1474`, `#1476`, `#1506`, `#1621`, `#1631`, `#1689`, `#1730`, `#1753`, `#1754`, `#1755`, `#1756`, `#1757`, `#1758`, `#1765`, `#1936`, `#1937`.
   - **Core Focus**: Vectors, baked stroke geometry invalidation, outside text stroke double-painting, sibling vector masks, GPU blur fringes, and the 5 remaining desktop UX micro-interactions.
   - **Evidence Dossier**: `../evidence/rea-bugs-c3-vectors-strokes-masks-ux.md` (published to `../evidence/rea-bugs-c3-vectors-strokes-masks-ux.md`).

---

## 4. Architectural Rules & Safety Guardrails

All coding agents implementing fixes from these specifications must follow the invariants defined in `AGENTS.md` §4:

1. **No `doc.to_json()` in `doc_hash` (`crates/designos-app/src/home/session.rs`):**
   Always hash documents via the streaming, zero-allocation `HashSerializer`. Never serialize `Document` to a string for change detection or autosave probing.
2. **No `ctx.load_texture` inside `ctx.data_mut` (`crates/designos-app/src/icons.rs`):**
   `egui::Context::data_mut` holds a non-reentrant write guard. Decode and load textures outside `ctx.data_mut` to avoid deadlocking Frame 1.
3. **Preserve `interactive()` Guard in `designos-gpu` (`crates/designos-gpu/src/compile.rs`):**
   Keep all LOD approximations strictly guarded by `self.interactive()` so headless reference tests (`cargo test -p designos-gpu`) remain bit-exact against CPU reference rendering.
4. **Testing Invariant:**
   Always append `-- --skip vsf_pcp` to all `cargo test -p designos-fig` invocations to skip the 1.05M-node enterprise fixture unless specifically benchmarking memory.
5. **Clean-Room Verification:**
   Zero proprietary Figma source code is included in these specs or implementations. All structures and algorithms are idiomatic clean-room Rust implementations based on observable binary behavior, schema fields, and runtime AST semantics.
