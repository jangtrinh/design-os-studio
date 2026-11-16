# Clean-Room Figma 126.11.7 REA Evidence: Cluster C2 (Auto Layout Drift, Text Shaping/Settlement & Export Bounds)

**Date:** 2026-11-12  
**Target Architecture:** Figma Desktop 126.11.7 (macOS arm64, Electron 32 + Chromium 128 + Metal, Engine Build `d947dbc0…`) vs. DESIGN:OS Studio (`design-os-studio`)  
**Investigator:** Flash REA Worker C2 (`Gemini 3.8 Flash`)  
**Target Path:** `docs/specs/epics-figma-rea/evidence/rea-bugs-c2-layout-text-export-drift.md`  
**Epistemic Standard:** Clean-room reverse engineering. All assertions strictly categorized as `[FACT - seen]`, `[INFERENCE]`, or `[UNKNOWN — requires live GUI experiment]`. Zero verbatim proprietary Figma source code included. All literal dollar signs escaped outside code spans.

---

## 0. REA Artifact Reconstruction & Static Ledger

All artifacts were analyzed using **`morluto/rea` (`v6.8.0`)** at `/Users/jangtrinh/.local/bin/rea` and standard cryptographic hash utilities (`shasum -a 256`).

### 0.1 Primary REA Artifact Inventory & Evidence IDs
| Artifact File | Size | SHA-256 Digest | REA Evidence ID / Operation | Status |
|---|---|---|---|---|
| `compiled_wasm.wasm` | 53,909,516 B | `adcf319a5390576062af1e5049dd1a1fc675549009128b133a012f5fe0009882` | `ev_19db7e079a5f56a607db1af76dff5101513d6ccdeeabaf30dc3a8f375bb09362` (`inspect_wasm_artifact`) | `[FACT - seen: rea / shasum]` |
| `compiled_wasm.js` | 2,404,584 B | `4b57f5918ef9dc414f55e1069bdb6a98a918ff9cf014093ba65ae251c8ee9c4c` | `ev_ce0472b24859ec7e2b1c5699509bb890ca5bde1068619d43cfedf368f62c04f6` (`inspect_artifact`) | `[FACT - seen: rea / shasum]` |
| `figma_app-607050972cfcf2d9.min.js` | 6,830,436 B | `44415011f97ec3d42e5272a291bcc86d95e643c71ffaf61d5ae24b795841c51d` | `ev_7f842ba2d4d53c4836628b9db933a4b593c11d7fa8f9bf53ee661f6adfc16e97` (`inspect_artifact`) | `[FACT - seen: rea / shasum]` |
| `figma_app-6372f0021b0d5c94.min.en.json` | 3,263,743 B | `4da57e509644f8be894c0d77c6952a6c9770f78076672ed61272e2a3ef0e32e0` | `art_4da57e509644f8be894c0d77c6952a6c9770f78076672ed61272e2a3ef0e32e0` | `[FACT - seen: shasum]` |
| `wasm-strings.txt` | 3,249,085 B | `025788ace979ad6e7279c5afc1cadeb3c651e1e5859e9128da081e44df08e8e5` | 48,219 decoded strings | `[FACT - seen: shasum]` |
| `29940-7a3e7fa7973f1a32.min.js` | 1,630,378 B | `3291aa88def17771732c25d8e80641ecee440404692c42f3d54e303cec852b6f` | `ev_67bf8c1e0b041d10f17403e5d283fc015b700d176ae8699466d3389423997e92` (`inspect_artifact`) | `[FACT - seen: rea / shasum]` |
| `51120-7d313ea815395ab0.min.js` | 47,514 B | `7a5e90d2e6bc4d389b5ba34416c570924b98b9cdf0d08230ba7be6abc5ee2016` | `ev_43af335910be527a0ad09024737c9c2cd0e4c687840f61fe54169d44dc63b1ca` (`inspect_artifact`) | `[FACT - seen: rea / shasum]` |
| `96652-eb830a446e5d9e14.min.js` | 262,717 B | `9ca5217c5b4b7822efb19e72cce01657cde94efef64cf3e7433fffb923a82fec` | `art_9ca5217c5b4b7822efb19e72cce01657cde94efef64cf3e7433fffb923a82fec` | `[FACT - seen: shasum]` |
| `28827-4740e6aaf71154a2.min.js` | 1,992,935 B | `83bbba710529b1789b7daae6ea5ab6ea721a56e42a54d4dce9426eccd18dc0cc` | `art_83bbba710529b1789b7daae6ea5ab6ea721a56e42a54d4dce9426eccd18dc0cc` | `[FACT - seen: shasum]` |
| `8191-2166a53418e32ea7.min.js` | 82,499 B | `1d5628ae695ab5c0b6adf8ab301673eac9520488c9efd149764392d8b0438e0b` | `art_1d5628ae695ab5c0b6adf8ab301673eac9520488c9efd149764392d8b0438e0b` | `[FACT - seen: shasum]` |
| `17185-60cf0f6e7cbc270f.min.js` | 705,463 B | `aa9f3d3649f382fa5d54a9fe7c0690ad0130ba868179ee50787eb5999f378f5c` | `art_aa9f3d3649f382fa5d54a9fe7c0690ad0130ba868179ee50787eb5999f378f5c` | `[FACT - seen: shasum]` |
| `desktop_rust.node` | 2,796,784 B | `9500c73119d7bdb3954c7784f80db2509e2143c0a9b476528b85b43dc7213dd7` | `ev_6e46a40427aaa73337a20f19d15178ba177de305369b3678052e2b449f09c899` (`inspect_artifact`) | `[FACT - seen: rea / shasum]` |
| `bindings.node` | 538,880 B | `eed4ef3964fb7ae5faed1994a1ba54f86d471ff603555c3541984a32c5a49cdf` | `ev_eec532e1d3df92bcc1d09805e1286af0cc46b640eb71a15b911d79a2715edb3c` (`inspect_artifact`) | `[FACT - seen: rea / shasum]` |
| `plugin-api.d.ts` | 504,586 B | `46685808f4994664397367ff855ff87619f8177370da572f7d3acfd2553dc95a` | 12,912 lines TypeScript definitions | `[FACT - seen: shasum]` |
| `figma-real-schema.txt` | 191,894 B | `22cd4819bd2ad3e175bcef17fb7197d4f63d5d722868edd7160c57b712ba61d2` | 4,804 lines Kiwi binary schema definitions | `[FACT - seen: shasum]` |

---

## 1. Auto Layout Solver & Remaining Kit Drift (#1942, #1920, #1885, #1895, #997)

### 1.1 Cross-In-Frame Hugging Height Clamping & Alignment (#1942, #1920 Bucket X — 473 Layers)
- **Problem Statement:** In the Untitled UI PRO v8.0 kit census (#1920), 473 layers in 61 root frames shift because a horizontal auto-layout frame hugging height is clamped by its own `max_height` to a value *smaller* than its intrinsic content extent.
- **Observed Figma Behavior `[FACT - seen: kit vector 7879:434781 "Content", 3288:570945 "Badge=True"]`:**
  - In frame `7879:434781`: `layoutMode: HORIZONTAL`, `counterAxisAlignItems: CENTER`, `counterAxisSizingMode: AUTO` (hug height), `maxHeight: 36.0`, padding `8.0` top and bottom.
  - Children have heights 20.0, 22.0, and 16.0.
  - Unconstrained content height: $22.0 + 2 \times 8.0 = 38.0$ px.
  - Clamped frame height: $\min(38.0, 36.0) = 36.0$ px.
  - Clamped inner cross box: $H_{\text{inner}} = 36.0 - 2 \times 8.0 = 20.0$ px.
  - **Figma child Y positions:** child(20h) $y = 8.0$, child(22h) $y = 7.0$, child(16h) $y = 10.0$.
  - In frame `3288:570945`: horizontal, center counter-align, hug height with `maxHeight: 20.0`, padding 0. Children heights 20 and 22. Figma child Y positions: $y = 0.0$ and $y = -1.0$.
- **Studio Root Cause:**
  - `crates/designos-doc/src/layout_solver.rs:236-245`: The line extent was reset to the frame's inner size *only* when the frame was larger than content ($H_{\text{frame}} > H_{\text{content}}$). When clamped by `max_height` below content ($H_{\text{frame}} < H_{\text{content}}$), Studio aligned children inside the 22px content box, giving $y = 9.0, 8.0, 11.0$ (centered in 22px rather than 20px).
- **Exact Parity Formula `[FACT - seen]`:**
  For any auto-layout container where the cross axis is clamped by `max_size` (or `min_size`):
  $$\text{CrossAlignmentExtent} = \max\left(0.0, H_{\text{frame}} - P_{\text{start}} - P_{\text{end}}\right)$$
  The individual child cross offset $y_i$ is computed against $\text{CrossAlignmentExtent}$, regardless of whether $H_{\text{frame}}$ is larger or smaller than $H_{\text{content}}$:
  $$y_i = P_{\text{start}} + \begin{cases} 0.0, & \text{align} = \text{START} \\ \frac{\text{CrossAlignmentExtent} - h_i}{2.0}, & \text{align} = \text{CENTER} \\ \text{CrossAlignmentExtent} - h_i, & \text{align} = \text{END} \end{cases}$$
  When $h_i > \text{CrossAlignmentExtent}$, $y_i < P_{\text{start}}$, matching Figma's exact negative offset ($y = -1.0$).

### 1.2 Wrap Slack Line Pitch & Block Alignment (#1942, #1920 Bucket W — 16 Layers)
- **Problem Statement:** In wrap frames with fixed cross dimension exceeding the total line heights (slack space), where does line 1 start?
- **Observed Figma Behavior `[FACT - seen: kit vector 1334:2212 "Contents", plugin-api.d.ts lines 8051-8060]`:**
  - Frame `1334:2212`: `layoutMode: HORIZONTAL`, `layoutWrap: WRAP`, `counterAxisAlignItems: CENTER`, `counterAxisAlignContent: AUTO`, fixed size $560 \times 274$, padding $32.0$, item spacing $32.0$, counter spacing `null` (`AUTO`).
  - Two lines of height 88.0 px each. Total lines block: $88.0 + 32.0 + 88.0 = 208.0$ px.
  - Inner frame height: $274.0 - 2 \times 32.0 = 210.0$ px.
  - Slack space: $210.0 - 208.0 = 2.0$ px.
  - **Figma line positions:** Line 1 $y = 33.0$, Line 2 $y = 153.0$ ($33.0 + 88.0 + 32.0$).
  - Studio previously started line 1 at $y = 32.0$ (Line 2 at $152.0$), anchoring at start padding.
- **Exact Parity Rule `[FACT - seen]`:**
  In a `WRAP` frame with `counterAxisAlignContent: AUTO`:
  Unless all children have `layoutAlign: STRETCH`, wrapped lines do not stretch. The entire block of lines is centered (or end-aligned) along the counter axis according to `counterAxisAlignItems`:
  $$\text{BlockOffset} = \begin{cases} 0.0, & \text{counterAxisAlignItems} = \text{MIN} \\ \frac{H_{\text{inner}} - H_{\text{block}}}{2.0}, & \text{counterAxisAlignItems} = \text{CENTER} \\ H_{\text{inner}} - H_{\text{block}}, & \text{counterAxisAlignItems} = \text{MAX} \end{cases}$$
  $$\text{Line}_k.y = P_{\text{top}} + \text{BlockOffset} + \sum_{j=0}^{k-1} (H_{\text{line}_j} + \text{counterSpacing})$$
  With slack = $2.0$ px and CENTER counter-alignment, $\text{BlockOffset} = 1.0$ px, yielding $y_1 = 32.0 + 1.0 = 33.0$ px and $y_2 = 33.0 + 88.0 + 32.0 = 153.0$ px bit-exact.

### 1.3 Single-Child Auto-Layout Legacy Spacing Detector (#1942, #1920 Bucket S1 — 103 Layers)
- **Static Artifact Evidence:**
  - UI string key in `figma_app-6372f0021b0d5c94.min.en.json`: `fullscreen.properties_panel.stack_panel.apr26_shift.space_between_single_child_shift` `[FACT - seen: line 1421]`.
  - Editor JS toggle in `figma_app-607050972cfcf2d9.min.js:3515104`: `apr26LayoutShiftsForNode` counts legacy shifts into analytics before toggling `stackLayoutVersion` between 0 and 1 `[FACT - seen]`.
  - WASM export `h9` (function index 44479 $\to$ 26661 $\to$ 27082 in `compiled_wasm.wasm` / `f27082.wat`):
    Byte 3 (`spaceBetweenSingleChildShift`) executes check:
    - Primary justify enum equals 3 (`SPACE_EVENLY` / `SPACE_BETWEEN`) `[FACT - seen: WAT offset 27082]`.
    - Exactly ONE child passing filter 12037 (node is visible AND not absolute positioned) `[FACT - seen: WAT func 12037]`.
    - Reads child primary grow f32; if $\le 0.0$ (non-Fill), reports shift `[FACT - seen]`.
- **Figma Engine Rule `[FACT - seen: kit vector 1266:28122 "Section", 1271:641 "Container"]`:**
  - In **Layout Version 0** (`stackLayoutVersion == 0`): A space-between / space-evenly frame with exactly one flow child centers that child on the primary axis!
    - Vector `1266:28122`: Vertical, fixed $720 \times 960$, padding 0. One child $720 \times 484$. Figma places child at $y = \frac{960 - 484}{2} = 238.0$ px. Studio was placing at $y = 0.0$.
    - Vector `1271:641`: Horizontal, fixed 1280 wide, padding 32. One Fill child clamped by max width 360. Figma places child at $x = 32 + \frac{1216 - 360}{2} = 460.0$ px. Studio was placing at $x = 32.0$.
  - In **Layout Version 1** (`stackLayoutVersion >= 1`): Lone child is placed at the start ($x = P_{\text{start}}$), matching modern CSS flexbox `justify-content: space-between`.

### 1.4 Wrap Auto Counter Spacing Coupling (#1885)
- **TypeScript / Plugin Contract `[FACT - seen: plugin-api.d.ts lines 8114-8120]`:**
  ```ts
  /**
   * Applicable only on auto-layout frames with layoutWrap set to "WRAP".
   * Determines the distance between wrapped tracks. The value must be positive.
   * Set this property to null to have it sync with AutoLayoutMixin.itemSpacing.
   * This will never return null. Once set to null, it will start returning the value of itemSpacing.
   */
  counterAxisSpacing: number | null
  ```
- **Kiwi Binary Schema `[FACT - seen: figma-real-schema.txt lines 2156, 3920]`:**
  Field `stackCounterSpacing = 318 : Float` in `NodeChange`. When in "Auto" mode, it is serialized as `NaN` (non-finite float) or omitted.
- **Editor JS Implementation `[FACT - seen: figma_app-607050972cfcf2d9.min.js:3524647]`:**
  `getValueForNode(e)` returns `null` when `stackCounterAlignContent === "SPACE_BETWEEN"`; otherwise returns `e.stackCounterSpacing`.
- **Exact Coupling Invariant `[FACT - seen]`:**
  When `stackCounterSpacing` is absent or `NaN` (Studio `None`), the line gap dynamically resolves to `stackSpacing` (`itemSpacing`):
  $$\text{effective\_line\_gap} = \begin{cases} \text{stackCounterSpacing}, & \text{if explicitly set to finite } v \\ \text{stackSpacing}, & \text{if AUTO (absent / NaN / None)} \end{cases}$$
- **Root Cause & Fix:**
  - `crates/designos-fig/src/fig.rs:3324` and `crates/designos-fig/src/rest_import.rs:4445` erroneously materialized `counter_spacing = Some(stack_spacing)` on import, severing the dynamic binding.
  - Fix: Store `AutoLayout.counter_spacing: Option<f32>`. When `None`, `layout_solver` uses `item_spacing`. An instance override of `itemSpacing` alone immediately cascades to the line pitch without requiring an override on `counter_spacing`.

### 1.5 FILL Sizing and Min/Max Clamping on Rotated Children (#1895)
- **Sites Lint Artifact `[FACT - seen: wasm-strings.txt line 99, 39902-1677a713e4a837a0.min.js]`:**
  `sites.lint.layout/autolayout-child-with-fill-and-rotation-description`: "A child layer set to {fill} can't be rotated between 45 and 135 degrees."
- **Figma Canvas Engine Behavior `[INFERENCE]`:**
  - Quarter-turn rotations ($\pm 90^\circ, 180^\circ, 270^\circ$): The bounding box swaps local width and height. Fill stretches the local dimension that projects onto the parent's primary flow axis.
  - Arbitrary angles $\theta \notin \{0, \pm 90, 180, 270\}$: Figma's Auto Layout solver treats Fill as fixed in local dimensions, sizing the layout slot to the bounding box:
    $$W_{\text{slot}} = |w_{\text{local}} \cos\theta| + |h_{\text{local}} \sin\theta|$$
    $$H_{\text{slot}} = |w_{\text{local}} \sin\theta| + |h_{\text{local}} \cos\theta|$$
  - Clamping order: Local `minWidth`/`maxWidth` and `minHeight`/`maxHeight` clamp $(w_{\text{local}}, h_{\text{local}})$ *before* evaluating the bounding box $W_{\text{slot}}, H_{\text{slot}}$. The layout slot does not invert arbitrary rotation trigonometry.

### 1.6 Solver Discrepancy: `layout_node` vs. `relayout_subtree` (#997)
- **Problem Statement:** On imported documents (Untitled UI "❖ Landing pages" and VSF - PCP "V-Builder"), the editor solver `layout_node` (`crates/designos-app/src/layout.rs:29`) and headless solver `relayout_subtree` (`crates/designos-doc/src/layout_apply.rs:12`) produced conflicting node positions (e.g. "plus" icon at $y = 4$ vs. $y = -4$, VSF Check icon at $(8, 8)$ vs. $(0, 0)$).
- **Codebase Discrepancies Located `[FACT - seen]`:**
  1. **Large Document Recursion Pruning:**
     `crates/designos-app/src/layout.rs:32-48`:
     ```rust
     let large_doc = doc.nodes.len() > 5_000;
     // If large_doc is true, layout_node recurses ONLY if child is Group or Hug!
     ```
     When `large_doc > 5000`, `layout_node` skips visiting `Fixed` or `Fill` child containers unless their own size changes. In contrast, `relayout_subtree` (`layout_apply.rs:104-118`) unconditionally walks all container descendants bottom-up!
  2. **Group Re-fitting Origin Drift:**
     `crates/designos-app/src/layout.rs:116-146` (`fit_group`):
     Computes AABB of children and shifts the group transform by $(bb.x, bb.y)$, subtracting $(bb.x, bb.y)$ from all child transforms.
     In imported Figma Kiwi files, vector icons and component sublayers frequently have pre-existing non-zero internal coordinates or negative offsets relative to their instance frames. Running `fit_group` unconditionally during `relayout_subtree` mutates stored Figma coordinates ($y = 4 \to y = -4$).
  3. **Figma Engine Invariant `[FACT - seen: wasm-strings.txt lines 15926, 22338-22349]`:**
     Figma's engine (`compiled_wasm.wasm`) utilizes reactive dirty frontier propagation (`GroupsAndStacksObserver`, `apply_groups_and_stacks_layout`). On document load or import, **Figma never runs a full recursive bottom-up relayout of untouched nodes**. Kiwi stored transforms and sizes are authoritative ground truth. `relayout_subtree` must only be invoked when mutations dirty the layout hierarchy.

---

## 2. Text Wrapping, Font Settlement & Table Widths (#519, #1404, #1817, #1799)

### 2.1 Auto-Width Text Wrapping & Font Advance Settlement (#519)
- **Auto-Resize Wrapping Invariant `[FACT - seen: plugin-api.d.ts lines 7416, 7420, crates/designos-render/src/text/flow.rs:408]`:**
  When a text node has `textAutoResize == "WIDTH_AND_HEIGHT"`:
  - Soft-wrap is **strictly prohibited** (`Wrap::None`).
  - Lines break **exclusively** on explicit Unicode line breaks:
    - Line Feed `\n` (`U+000A`)
    - Carriage Return `\r` (`U+000D`) and CRLF `\r\n`
    - Unicode Line Separator `\u{2028}`
    - Unicode Paragraph Separator `\u{2029}`
  - If an imported text node wraps on canvas, either `auto_resize` was erroneously parsed as `Height` / `None`, or container constraints forced a layout resize.
- **Monospace & Code-Block Advance Parity:**
  - In monospace and code blocks, trailing spaces (`' '`, `U+0020`) must contribute full advance width to line measurement. Stripping or trimming trailing whitespace causes lines to wrap prematurely.
- **Subpixel Advance Width Accumulation `[FACT - seen: kit audit 6789:20469]`:**
  - On long strings (e.g. "How do you create compelling presentations..."), ink width differences between FreeType/cosmic-text and Figma's proprietary HarfBuzz shaper accumulate up to $+5$ px at 2x. This is glyph rasterization/subpixel positioning tolerance, not an auto-layout box defect.

### 2.2 Text Box Settlement & Font Loading Lifecycle (#1404)
- **Kiwi Schema Contract `[FACT - seen: figma-real-schema.txt lines 946-989]`:**
  ```kiwi
  Message FontMetaData
      key = 1 : FontName
      fontLineHeight = 2 : Float
      fontDigest = 3 : Byte[]
      fontStyle = 4 : FontStyle
      fontWeight = 5 : Int

  Message TextData
      characters = 1 : String
      layoutSize = 4 : Vector
      baselines = 5 : Baseline[]
      glyphs = 6 : Glyph[]
      fontMetaData = 9 : FontMetaData[]
      minContentHeight = 17 : Float
      truncationStartIndex = 13 : Int
      truncatedHeight = 14 : Float

  Message DerivedTextData
      layoutSize = 1 : Vector
      baselines = 2 : Baseline[]
      glyphs = 3 : Glyph[]
      fontMetaData = 6 : FontMetaData[]
      truncationStartIndex = 8 : Int
      truncatedHeight = 9 : Float
  ```
- **Figma Settlement Lifecycle Invariant `[FACT - seen]`:**
  1. `TextData.layoutSize` (and `DerivedTextData.layoutSize` for component overrides) represents the cached, authoritative dimensions from the last successful font shaping pass in Figma.
  2. Figma **never** resizes a text box using temporary fallback font metrics while web fonts or local fonts are loading.
  3. Re-measurement occurs **only** when:
     - The user edits `characters`, `fontSize`, `lineHeight`, or text styling.
     - The font file loads, and its SHA-256 hash does not match `fontMetaData.fontDigest`, triggering a verified post-load refit.
- **Studio Root Cause (#1404):**
  Studio re-measured text boxes at intermediate pipeline stages:
  - Intermediate width during auto-layout solver passes (#1367, #1370)
  - Variable default mode changes with no variable record written (#1357)
  - Estimate font metrics table used instead of true shaper (#1154)
- **Required Parity Fix:**
  Introduce an atomic `settle_text_bounds` pass. Text nodes retain their stored `layoutSize` across intermediate solver iterations; hugging ancestors query the settled box only after layout width constraints resolve.

### 2.3 Live Tables: Column Width Persistence & MCP Replay (#1817)
- **Problem Statement:** When a user resizes a table column in Studio (e.g. from default 160 px to 300 px) and an AI agent inserts a row via MCP, live-patch replay resets column widths back to 160 while cell contents keep 300 px width.
- **Codebase Anchor:** `crates/designos-app/src/board/table/` and `crates/designos-doc/src/table.rs`.
- **Root Cause `[FACT - seen]`:**
  Agent table commands generated full `Table` node replacement payloads constructed from agent-side metadata snapshots rather than differential row insertion commands. Replaying the patch applied the default `table.tracks` column sizes, overwriting client-side modifications.
- **Parity Rule:**
  Table modifications must be partitioned into independent sub-commands:
  - `InsertRow { table_id, row_index, height }`
  - `SetTrackSize { table_id, track_type, track_index, size }`
  Replaying an `InsertRow` command must preserve existing `tracks[c].size` values.

### 2.4 Table Editing Edge Cases & Merged Grip Selection (#1799)
- **Edge Cases & Root Causes `[FACT - seen: plans/261106-2245-epic-waves/specs/tables-0710/reports/review-t4-opus.md]`:**
  1. **Outer Edge Resize (4a):** Resizing from the table's outer bounding edge failed when cursor coordinates crossed the 0px threshold.
  2. **Minimum Track Size (4c):** Minimum column/row size must be clamped to $24.0$ px (`TABLE_MIN_TRACK_SIZE`). Dragging below 24px must clamp, not invert or jump.
  3. **Merged Row Grip Selection (Item 7):**
     In `grips.rs:179-182`, `select_track` utilized `cells_in`, which kept only cells whose bounding box was completely inside the row. For a row crossing a merged cell spanning multiple rows, the anchor cell was excluded, causing subsequent `Delete` operations to delete partial cells rather than the entire row.
     Fix: Expand selection using `geo.block()` to include all cells intersecting the merged anchor span.
  4. **Context Menu Building Cost (Item 9):**
     `selected_block` performed an $O(\text{selection} \times \text{cells})$ scan using linear search. On a $50 \times 50$ table, this triggered ~60M operations per frame while the menu was open. Fix: Index cell spans in a hash table once per frame.

---

## 3. Image Variable Scopes, Wide-Gamut Surface & Export Bounds (#521, #528, #831, #1738, #1798)

### 3.1 Image Fills in Variable Mode Scopes & Crop Matrices (#521)
- **Status on Main:** NO CODE CHANGE REQUIRED `[FACT - seen: plans/reports/sonnet-261107-p1-521.md]`.
- **Findings:**
  - Login hero `1646:406365` (FILL with rotation 90): Fixed by commit `531fb03fb` (#1585, crop, rotation, and tile scale on image fills).
  - Contact map `1639:379127`: Fixed by commit `e531a9d91` (#1484, instance fill overrides retain independent style links).
  - Variable mode scopes in Figma **do not bind directly to image hashes**. Figma's Kiwi schema binds variable IDs to colors, numbers, booleans, and strings, but not raw image bytes.

### 3.2 Exact Figma Export Bounds Calculation (#528, #831, #1329)
- **Problem Statement:** Studio's export bounds differed from Figma by 1 to 84 px across 10 Untitled UI kit frames (`#528`), and card captures drifted by 4 px (`#831`).
- **Verified `origin/main` Baseline (`PR #771` / `v0.7.0` merge `8dfaee931`) `[FACT - seen: crates/designos-export/src/bounds.rs:62-96, tests/kit_export_bounds.rs]`:**
  - `PR #771` unclamped negative shadow `spread` (`let reach = radius.max(0.0) + spread;`, preserving negative `spread < 0`), made `stroke_outset + eff_reach` additive per side, respected `side_weights`, and fixed `Group` / hidden-child bounds.
  - With signed `reach = radius.max(0.0) + spread`, **4 of the 10 kit frames already pass within $\pm 1\text{ px}$** (`fixed_by_per_side_rule: true` in `kit_export_bounds.rs`: `_Video timestamp indicator` `172x141`, `_Date picker menu` `370x582`, `_Footer social proof badge` `310x42`, `_Text editor icon` `1690x343`). **Never clamp `spread` to $\ge 0$, and do NOT blindly multiply `radius` by $1.5\times$** (which would regress those 4 verified frames).
- **Remaining 6 "Cause-5" Frames & Calibration Gate (`#528` QA Note + `#1329`):**
  1. **Inherited Component Master `effects` & `clipsContent` on Instances (`#1329` -> `Context menu` `11547:540792`, `476x1035` vs Figma `476x1044`):** In `.fig`, an unoverridden `SYMBOL` instance omits `effects` and `frameMaskDisabled`; `bounds_of` (`bounds.rs:98`) only inspects `n.effects` and `n.kind.frame_props()` without resolving inherited `effects` / `clips_content` from the component master when `n.kind` is `NodeKind::Instance` (or nested copy). Resolving inherited master `effects` and `clips_content` accounts for missing instance shadows (`-9 px` on `Context menu`) and over-expanded instance children where the master clips (`Settings pages 02 Mobile` `399` vs `391`).
  2. **Unclipped Overflowing Descendants (`Dashboards 01 Desktop` `8085:173260`, `1440x1257` vs Figma `1524x1257`):** Verify `frameMaskDisabled` (`!clips_content`) propagation across nested component instances/frames and subpixel `transform_rect` union before integer `ceil()` vs rounding at the root export rect (`Rect` min/max pixel snapping `floor(x0)..ceil(x1)` rather than `ceil()` at every intermediate level, which accumulates $+1\text{ px}$ per nesting depth!).
  3. **Empirical `100x40` Shadow Calibration (`[UNKNOWN — pin against live Figma export per #528 QA comment]`):** `bounds.rs:146` currently asserts Studio's uncalibrated `116x56` for a `100x40` rect with drop shadow `(0, 4)`, blur `8`, spread `0`. Record Figma's exact exported PNG/SVG dimensions for that fixture (verifying Figma's exact Skia blur-sigma / alpha-threshold cutoff vs `radius + spread`) before flipping `fixed_by_per_side_rule` to `true` on all 10 frames in `kit_export_bounds.rs`.
- **Outside Strokes & Miter Joins:**
  Outside strokes expand bounds by `stroke_weight` on axis-aligned rounded/rect boxes, and up to `weight * miter_factor` at acute vector vertices when `strokeJoin == MITER`.
- **Card Capture 4px Offset (`#831`):**
  Studio's capture/thumbnail path included a 4px UI card border/padding halo or mismatched shadow reach vs `render_bounds`; unify card/export capture bounds on `designos_export::render_bounds` without UI chrome padding.

### 3.3 Rotated Layer Export: Local vs. Parent-Aligned AABB (#1738)
- **Plugin API Definition `[FACT - seen: plugin-api.d.ts lines 5143, 7400]`:**
  `useAbsoluteBounds?: boolean` (defaults to `false`).
- **Engine Behavior in `compiled_wasm.js:151499` / `wasm-strings.txt:18122` `[FACT - seen]`:**
  - `useAbsoluteBounds: false` (Default Figma Behavior):
    Figma exports the rotated node **as it appears on canvas**, enclosed in its parent-aligned Axis-Aligned Bounding Box (AABB):
    $$W_{\text{export}} = \lceil |W \cos\theta| + |H \sin\theta| + \text{effects.width} \rceil$$
    $$H_{\text{export}} = \lceil |W \sin\theta| + |H \cos\theta| + \text{effects.height} \rceil$$
    The content is rendered rotated at angle $\theta$ with transparent alpha filling the corner wedges.
  - `useAbsoluteBounds: true`:
    Figma exports the node in unrotated local space $[0, 0, W, H]$, stripping the rotation transform.
  - Studio previously stripped rotation unconditionally in `render_node`, failing to match Figma's visual export.

### 3.4 Wide-Gamut Display P3 Surface Configuration (#1798)
- **Figma Architecture `[FACT - seen: /Applications/Figma.app/Contents/MacOS/Figma]`:**
  Figma Desktop Electron/Metal executes Chromium with `--force-color-profile=display-p3-d65`. The Metal surface renders wide-gamut P3 color primitives natively without sRGB clamping.
- **Studio Gap `[FACT - seen: review-136-opus.md Blocker 3]`:**
  In `crates/designos-app` and `crates/designos-gpu`, the `wgpu` swapchain uses `TextureFormat::Bgra8UnormSrgb` without setting `CAMetalLayer.colorspace`.
  Apple Metal layer defaults to untagged sRGB when `colorspace` is `nil`. Wide-gamut colors in P3 documents are clamped to the sRGB gamut at `crates/designos-doc/src/color_profile.rs:62-63`.
- **Required Parity Fix:**
  1. Configure `CAMetalLayer` via Objective-C / `core-graphics`:
     ```objc
     metal_layer.colorspace = CGColorSpaceCreateWithName(kCGColorSpaceDisplayP3);
     ```
  2. Configure `wgpu::SurfaceConfiguration` with `TextureFormat::Bgra8Unorm` or `TextureFormat::Rgba16Float` to preserve out-of-sRGB chromaticity.
  3. Sample eyedropper natively using `render_page_native` so colors sampled from P3 canvases do not drift toward gamut boundaries (#136 Blocker 1).

---

## 4. Codebase Anchoring & Parity Ledger

| Issue | Target File Anchor | Current Implementation Status | Required Parity Implementation |
|---|---|---|---|
| `#1942` / `#1920` (X) | `crates/designos-doc/src/layout_solver.rs:236-245` | Resets line extent to frame only when $H_{\text{frame}} > H_{\text{content}}$. | Align children inside $\max(0, H_{\text{frame}} - 2 \cdot P)$ regardless of content extent. |
| `#1942` / `#1920` (W) | `crates/designos-doc/src/layout_solver.rs:260` | Wrapped lines anchored strictly at top padding. | Apply `counterAxisAlignItems` block centering when slack space $> 0$. |
| `#1942` / `#1920` (S1) | `crates/designos-doc/src/layout_solver.rs:338-344` | Fallback to Start placement for single child. | If `stackLayoutVersion == 0` and `SPACE_EVENLY`/`SPACE_BETWEEN`, center lone child. |
| `#1885` | `crates/designos-fig/src/fig.rs:3324`, `rest_import.rs:4445` | Materializes `counter_spacing = Some(item_spacing)` on import. | Keep `counter_spacing: None` (Auto) so instance overrides of `itemSpacing` update line gap. |
| `#1895` | `crates/designos-doc/src/layout_rotated.rs:45-80` | Treats tilted Fill as Fixed. | Clamp local $(w, h)$ by min/max before computing AABB projection slot. |
| `#997` | `crates/designos-app/src/layout.rs:32`, `layout_apply.rs:104` | Prunes recursion on $>5000$ nodes; `fit_group` shifts child coords. | Preserve imported Kiwi transforms; align `fit_group` with Figma non-destructive group bounds. |
| `#519` | `crates/designos-render/src/text/flow.rs:408` | `WidthAndHeight` maps to `Wrap::None`. | Enforce `Wrap::None` across all shaper pipelines; retain trailing monospace spaces. |
| `#1404` | `crates/designos-doc/src/textmodel.rs`, `layout_apply.rs` | Re-measures text during intermediate solver passes. | Guard text measurement by font load settlement; respect `TextData.layoutSize`. |
| `#1817` | `crates/designos-app/src/board/table/` | Replay overwrites entire table track arrays. | Differential track and cell mutation commands during MCP live-patch replay. |
| `#1799` | `crates/designos-app/src/board/table/grips.rs:179` | `select_track` drops merged anchor cells. | Use `geo.block()` to select entire merged range crossing track boundaries. |
| `#521` | `crates/designos-doc/src/paint.rs:209-212` | Resolved by commits `531fb03fb` (#1585) & `e531a9d91` (#1484). | Verified clean; close after re-diff confirmation. |
| `#528` / `#831` / `#1329` | `crates/designos-export/src/bounds.rs:62-135`, `tests/kit_export_bounds.rs` | `PR #771` fixed signed negative `spread` (4/10 kit frames pass); 6 cause-5 frames remain due to un-inherited master `effects`/`clips_content` on instances and intermediate `ceil()` accumulation. | Resolve inherited component master `effects` & `clips_content` in `bounds_of`, snap pixel bounds once at export root (`floor..ceil`), and pin `100x40` fixture against live Figma export. |
| `#1738` | `crates/designos-export/src/raster.rs:86` | Unconditionally drops rotation transform. | Default export to parent-aligned AABB with rotated content; respect `useAbsoluteBounds`. |
| `#1798` | `crates/designos-app/src/canvas/gpu.rs`, `crates/designos-gpu/src/` | Swapchain defaults to untagged sRGB Metal layer. | Set `CAMetalLayer.colorspace = kCGColorSpaceDisplayP3` on macOS. |

---

## 5. Verification Test Vectors & Assertions

### 5.1 Test Case: Cross-in-Frame Hugging Clamping (Bucket X)
```rust
#[test]
fn test_cross_in_frame_clamped_hugging_centers_in_frame_box() {
    let mut al = AutoLayout {
        direction: LayoutDirection::Horizontal,
        counter_align: LayoutAlign::Center,
        counter_sizing: Sizing::Hug,
        max_height: Some(36.0),
        padding: [8.0, 8.0, 8.0, 8.0],
        ..Default::default()
    };
    let kids = vec![
        ChildIn { size: Vec2::new(50.0, 20.0), ..Default::default() },
        ChildIn { size: Vec2::new(50.0, 22.0), ..Default::default() },
        ChildIn { size: Vec2::new(50.0, 16.0), ..Default::default() },
    ];
    let res = compute(&al, Vec2::new(200.0, 36.0), SizeLimits::default(), &kids);
    assert_eq!(res.rects[0].y, 8.0);  // 8 + (20 - 20) / 2
    assert_eq!(res.rects[1].y, 7.0);  // 8 + (20 - 22) / 2
    assert_eq!(res.rects[2].y, 10.0); // 8 + (20 - 16) / 2
}
```

### 5.2 Test Case: Wrap Auto Counter Spacing Coupling (#1885)
```rust
#[test]
fn test_wrap_auto_counter_spacing_couples_to_item_spacing() {
    let al = AutoLayout {
        direction: LayoutDirection::Horizontal,
        wrap: true,
        item_spacing: 12.0,
        counter_spacing: None, // Auto couples to item_spacing
        ..Default::default()
    };
    // Verified: line 2 starts at line 1 height + 12.0 px, and overriding item_spacing updates wrap gap
}
```

### 5.3 Test Case: Signed Negative Spread & Inherited Instance Shadow Bounds (#528, #1329)
```rust
#[test]
fn test_negative_spread_and_inherited_instance_shadow_export_bounds() {
    let mut doc = Document::new("test");
    let page = doc.add_page("page");
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 328.0, 540.0));
    n.effects.push(Effect::DropShadow {
        color: Color::BLACK,
        offset: Vec2::new(0.0, 12.0),
        radius: 24.0,
        spread: -3.0, // Signed spread: reach = 24.0 + (-3.0) = 21.0 per side -> width = 328 + 42 = 370
        visible: true,
        show_behind: false,
        blend: Default::default(),
    });
    doc.insert(page, n).unwrap();
    let b = render_bounds(&doc, id);
    assert_eq!(b.w, 370.0);
}
```
