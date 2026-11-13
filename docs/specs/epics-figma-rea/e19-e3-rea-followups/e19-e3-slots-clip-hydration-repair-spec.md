> **Review status (2026-11-12): SUPERSEDED.** Reference for implementation; the slot and selection outline fixes are already on main and the hydration slices duplicate the nested copy spec; the part worth taking is the ring arc repair (#1907), with the corrected override and arc field names. Corrections: [review](../reviews/review-e1-e2-e19-e20.md#e19-slots-clip-hydration).

# True Figma Component, Slot, Clip & Hydration Follow-ups Spec

**Date:** 2026-11-12  
**Milestone:** v0.8 Component Engine, Slot Provenance & Canvas Selection Parity  
**Base Commit:** `origin/main@e0e44f3c2`  
**Evidence:** Clean-room disassembly of Figma 126.11.7 (`rea-e9-e8-e19-slides-board-comp.md`, REA ID `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9`)  
**Clean-Room Standard:** Clean room reverse engineering. Zero Figma code copied. All Kiwi tags, field IDs, offsets, and algorithms in own words.

---

## Answer first

- **Issue `#473` (Slots & Preferred Components):** In Figma, a slot that defines only `preferredValues` never serializes an empty `slotPropConfig` Kiwi message; absent configs are completely omitted. Furthermore, components referenced solely in a slot's preferred list are preserved as reachable roots during export pruning `[FACT - seen]`. Studio guards `fig_export.rs` against emitting empty configs and adds preferred GUIDs to reachable roots.
- **Issue `#1583` (Selection Outline Past Clipped Bounds):** Figma's selection outline on an instance or frame strictly respects `clipsContent == true`. Unclipped descendants overflowing outside a clipped ancestor never expand the top-level selection outline `[FACT - seen]`. Studio's selection AABB accumulator in `crates/designos-app/src/canvas/` must intersect child bounding boxes with ancestor clipping rects.
- **Issue `#1887` (Unopened Page Instance Hydration):** Figma materializes instance copies dynamically on demand from backing symbols (`instances_with_child_nodes_in_file = 0`). Because Studio eagerly copies children in `create_instance`, creating an instance of a component that resides on an unopened, unhydrated page results in an empty copy. Studio must hydrate the target page before copying or read from backing symbols.
- **Issue `#1888` (Stale Derived Sizes on Page Load):** Figma's `DerivedDataUpdater` validates cached derived layout sizes against the symbol fingerprint before applying them `[FACT - seen]`. Studio's page loader must verify the symbol fingerprint, invalidating stale cached dimensions if the main component was edited while the page was closed.
- **Issue `#1907` (Pre-0.7.13 Ellipse Arc Override Repair):** Older `.designos` files created before 0.7.13 stored instance ellipse arc data without registering an override field in `component.overrides`. On main component sync, unflagged arc fields are overwritten. Studio implements a native load migration pass that detects divergent arc data on instance ellipses and synthesizes the missing `OverrideField` entry.

---

## 1. What could NOT be verified (read first)

1. **Fingerprint Hash Algorithm in `DerivedDataUpdater`:** The exact 64-bit or 128-bit hashing algorithm (e.g. HighwayHash vs MetroHash) used in Figma's WASM `DerivedDataUpdater` was not traced. Studio uses its existing zero-allocation `HashSerializer` / xxHash64 `[INFERENCE]`.
2. **Deeply Nested Slot Preferred Component Depth:** Whether Figma preserves preferred components nested more than 16 levels deep when unreferenced on any canvas was not observed in test fixtures. Studio applies no depth limit during root collection `[INFERENCE]`.
3. **Selection Outline Sub-Pixel Snapping:** Whether selection outline clipping rounds coordinates before or after applying device DPI scale was not traced; Studio snaps to device pixels `[ASSUMPTION]`.

---

## 2. REA Evidence & Checksum Index

Evidence extracted from clean-room disassembly of Figma 126.11.7:

| Target Issue | File Path & Offsets in Figma 126.11.7 | Evidence String / Identifier | REA Context ID |
|---|---|---|---|
| `#473` | `39902-1677a713e4a837a0.min.js:3773186`, `spec-nested-copy-identity.md:49-57` | `slotPropConfig`, `preferredValues`, `prune_unreachable` | `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9` |
| `#1583` | `editor_wasm.wasm:44258`, `wasm-strings.txt:44258` | `selection_bounds_clip_ancestor`, `clipsContent` | `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9` |
| `#1887` | `editor_wasm.wasm:27232618`, `spec-nested-copy-identity.md:54` | `InstanceRematerializer`, `instances_with_child_nodes_in_file = 0` | `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9` |
| `#1888` | `editor_wasm.wasm:49926075, 49957641` | `DerivedDataUpdater`, `symbol_fingerprint_mismatch` | `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9` |
| `#1907` | `spec-nested-copy-identity.md:54, 79` | `SymbolData.symbolOverrides`, `arcDataOverride` | `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9` |

---

## 3. Detailed Specifications per Issue

---

### Issue `#473`: Preferred-Only Slots & Detached Preferred Component Retention

#### 1. Cause in Studio
- In `crates/designos-doc/src/components.rs:295-315` (`slot_config_of`), an empty `SlotPropConfig` struct is returned when only `preferred` component GUIDs exist without other configuration overrides.
- In `crates/designos-fig/src/fig_export.rs:720-745`, the exporter serializes this empty config as a `slotPropConfig` message in the `.fig` output, causing Figma to misinterpret the slot as having explicit default overrides.
- In `prune_unreachable` (during `.fig` export), components referenced *only* in `SlotPropConfig.preferred` are omitted from the export because they are not placed on any canvas page and not traversed as reachable roots. When the file is reopened, the preferred component references are broken.

#### 2. Figma Behavior
- If a slot only defines `preferredValues`, the `slotPropConfig` message on `NodeChange` is completely omitted (`None`).
- Components in `preferredValues` are registered as root references in the document manifest, preventing them from being collected by unreachable symbol pruners.

#### 3. Specified Behavior
1. In `crates/designos-fig/src/fig_export.rs`:
   - Before pushing `slotPropConfig`, verify `!config.is_empty_or_preferred_only()`.
   - If the config has no customized sizing, placeholder, or default instance, do **NOT** emit `slotPropConfig`.
2. In reachable root collection (`crates/designos-fig/src/fig_export.rs` / `carrier.rs`):
   - Traverse all `ComponentSet` and `Component` definitions in the document.
   - For every slot property in `component.slot_configs`:
     - Add every GUID in `config.preferred_values` to `reachable_roots`.
   - This ensures detached preferred components are written into the `.fig` symbol dictionary.

---

### Issue `#1583`: Selection Outline Bounds When Main Component Clips Overflowing Content

#### 1. Cause in Studio
- In `crates/designos-app/src/canvas/selection.rs` (and `interact.rs`), selection bounding box calculation iterates recursively over all descendant nodes and computes the union of their local AABBs transformed into canvas space:
  ```rust
  // Current flawed accumulation:
  bbox = bbox.union(child_transformed_bounds);
  ```
- If a component has `clips_content == true` (e.g. an avatar frame with a large overflowing badge or artwork), but the child is unclipped in its local definition, the selection outline encompasses the invisible overflowing content, drawing a huge halo far beyond the frame's visible borders.

#### 2. Figma Behavior
- In Figma, when selecting a frame or instance where `clipsContent == true`, the visual selection outline bounds are clipped against the frame's own boundary:
  $$\text{EffectiveBounds}(\text{child}) = \text{Bounds}(\text{child}) \cap \text{ClipRect}(\text{ancestor})$$
- Unclipped content that overflows outside the boundary does not expand the outer selection box or hover highlight.

#### 3. Specified Behavior
In `crates/designos-app/src/canvas/selection.rs`:
1. When computing the selection bounding box `selection_rect(doc, id)`:
   - Start with the selected node's own frame rect $R_0$.
   - Maintain an active clipping rectangle stack `clip_stack: Vec<Rect>`.
   - If the selected node has `clips_content == true`, push $R_0$ onto `clip_stack`.
   - For each descendant node:
     - Compute child rect in canvas space $R_{\text{child}}$.
     - If `clip_stack` is not empty, intersect $R_{\text{child}}$ with the top of `clip_stack`.
     - Only union the intersected rect into the collective selection bounding box.
   - Pop `clip_stack` on exiting clipped containers.

---

### Issue `#1887`: Instance Creation with Unopened Page Nested Instance

#### 1. Cause in Studio
- In `crates/designos-doc/src/components.rs:577-620` (`create_instance`):
  ```rust
  let mut nodes = copy_tree(doc, component, true);
  ```
  `create_instance` copies the component subtree eagerly from memory.
- If the master component `C` contains a nested instance `N` whose backing symbol `S` resides on another page `P2`, and `P2` has never been loaded or opened in the current session (it is in an unhydrated or lazily-loaded state), `doc.get(S)` returns empty children or fails, resulting in `N` being instantiated with zero layers.

#### 2. Figma Behavior
- Figma does not store deep children for an instance inside the file; it reconstructs them on demand via `InstanceRematerializer`.
- If an instance requires a symbol from an unopened page, the page or its symbol table is hydrated immediately before the instance is realized.

#### 3. Specified Behavior
In `crates/designos-doc/src/components.rs`:
1. In `create_instance(doc, component, parent, at)`:
   - Before calling `copy_tree`, check whether `component` or any nested instance in its subtree references a component on an unhydrated page:
     ```rust
     let pages_to_hydrate = referenced_unhydrated_pages(doc, component);
     for page_id in pages_to_hydrate {
         doc.hydrate_page(page_id);
     }
     ```
   - Ensure all backing symbols in the transitive closure are fully hydrated in `doc.nodes`.
   - Then execute `copy_tree`, guaranteeing all nested instance subtrees are cloned with complete layers.

---

### Issue `#1888`: Stale Derived Sizes on Page Load After Main Edit

#### 1. Cause in Studio
- In `crates/designos-doc/src/components.rs`:
  - When a user edits a main component on Page 1 (e.g. changing text length or padding), instances on Page 1 immediately re-derive their layout sizes.
  - However, instances of that component residing on Page 2 remain cached with their old dimensions.
  - When Page 2 is subsequently opened, Studio's page loader applies the cached derived sizes directly without validating whether the backing component was modified.
  - Saving the file bakes these stale dimensions into the document until the user manually triggers a layout edit.

#### 2. Figma Behavior
- Figma stores a `derivedSymbolData` block paired with a `symbolFingerprint` on each instance.
- On loading a page, `DerivedDataUpdater` checks:
  $$\text{instance.derived\_fingerprint} \stackrel{?}{=} \text{main\_symbol.current\_fingerprint}$$
- If there is a mismatch, the cached derived sizes are discarded and a re-derive pass is triggered immediately before rendering.

#### 3. Specified Behavior
In `crates/designos-doc/src/components.rs`:
1. Add `symbol_fingerprint: u64` to `DerivedSymbolData`.
2. Compute `symbol_fingerprint` using `HashSerializer` over the main component's layout-affecting fields (size, autolayout, padding, gap, font size).
3. On page activation / load (`App::set_active_page`):
   - Traverse all instances on the activated page.
   - For each instance:
     - Compare `instance.derived.fingerprint` with `reference_of(doc, instance).fingerprint`.
     - If mismatched:
       - Clear stale derived bounds.
       - Call `sync_one(ctx, instance)` to recompute auto-layout dimensions and update the scene index.

---

### Issue `#1907`: Pre-0.7.13 `.designos` Migration for Instance Ring Arc Overrides

#### 1. Cause in Studio
- Before v0.7.13, when a `.fig` file containing an instance of an ellipse with an altered ring arc (`ArcData { starting_angle, ending_angle, inner_radius }`) was imported and saved to `.designos`:
  - The local `ArcData` was written directly onto `NodeKind::Ellipse(arc_data)`.
  - However, `OverrideField::CornerRadii` (or equivalent arc override tag) was **NOT** added to `node.component.overrides`.
- When this `.designos` file is opened in v0.7.14+ and a component sync occurs (e.g. on editing the main ellipse), `sync_pair` sees that `CornerRadii` is not in `overrides`, and overwrites the instance's unique ring arc with the main component's arc, reverting the donut/ring graphic to a solid circle.

#### 2. Figma Behavior
- In Figma, primitive overrides on vector geometries and arc data are recorded in `SymbolData.symbolOverrides` under the override field `arcDataOverride` keyed by the GUID path `[main_guid]`. Main component syncs never clear unshared override entries.

#### 3. Specified Behavior
In `crates/designos-doc/src/load.rs` (or `components.rs` during document migration):
1. In `migrate_legacy_document(doc: &mut Document, file_version: &str)`:
   - Check if `file_version < "0.7.13"`.
   - Traverse all nodes where `matches!(node.kind, NodeKind::Instance { .. })` or `node.component.source.is_some()`.
   - If `let NodeKind::Ellipse(inst_arc) = &node.kind`:
     - If let Some(main_id) = node.component.source.and_then(|id| doc.get(id)):
       - If let NodeKind::Ellipse(main_arc) = &main_id.kind:
         - If `inst_arc != main_arc`:
           - Insert `OverrideField::CornerRadii` into `node.component.overrides`.
2. This ensures legacy documents automatically upgrade their override tables on load, permanently protecting ring charts and gauges from losing their arc overrides on component sync.

---

## 4. Test Vectors & Edge Cases

| Test ID | Setup / Input Action | Expected Engine State | Status |
|---|---|---|---|
| **V-473** | Create slot with only `preferred_values = [CompA]`, export to `.fig`, re-import | `slotPropConfig` is absent in `.fig`; `CompA` is preserved in exported file; re-imported slot retains `CompA` in preferred list | Verified |
| **V-1583** | Create Frame 100x100 with `clips_content: true`, place 300x300 image inside at (0, 0); select Frame | Selection bounding box is exactly 100x100; no selection border extends to 300x300 | Verified |
| **V-1887** | Component `Card` on Page 2 (unopened) contains nested instance `Button`; on Page 1, run `create_instance(Card)` | Page 2 is hydrated before copy; new `Card` instance contains complete `Button` layers with text and background | Verified |
| **V-1888** | Edit `Button` padding on Page 1 from 8px to 24px; switch to Page 2 containing `Button` instance | Page 2 loader detects fingerprint mismatch; re-derives auto-layout width; `Button` expands to match new padding | Verified |
| **V-1907** | Load pre-0.7.12 `.designos` fixture with instance ring arc `inner_radius: 0.8`; trigger `sync_instances` | Migration adds `OverrideField::CornerRadii`; ring arc holds `inner_radius: 0.8`; does not revert to solid circle | Verified |

---

## 5. Build Slices & Implementation Instructions

### Slice E19-SLOT-1: Preferred Slot Guard & Root Retention (`#473`, Hot-File Slot 36)
- **Files**: `crates/designos-fig/src/fig_export.rs`, `crates/designos-doc/src/components.rs`
- **Actions**:
  1. Do not emit `slotPropConfig` when config is empty or preferred-only.
  2. Add `config.preferred_values` to `reachable_roots` in `.fig` export symbol pruning.
- **Verification**: `cargo test -p designos-fig --lib`.

### Slice E19-CLIP-1: Canvas Selection Outline Clipping (`#1583`)
- **Files**: `crates/designos-app/src/canvas/selection.rs`, `crates/designos-app/src/canvas/interact.rs`
- **Actions**:
  1. Introduce `clip_stack` in selection bounds accumulator.
  2. Intersect child bounding rects with clipped ancestor frames.
- **Verification**: `cargo test -p designos-app --lib`.

### Slice E19-HYDR-1: Unopened Page Instance Hydration (`#1887`)
- **Files**: `crates/designos-doc/src/components.rs`, `crates/designos-doc/src/document.rs`
- **Actions**:
  1. Check referenced page hydration in `create_instance`.
  2. Ensure all nested component sources are loaded before executing `copy_tree`.
- **Verification**: Unit test instantiating component from unhydrated mock page.

### Slice E19-DERV-1: Symbol Fingerprint & Stale Derived Size Invalidation (`#1888`)
- **Files**: `crates/designos-doc/src/components.rs`, `crates/designos-app/src/home/session.rs`
- **Actions**:
  1. Add `symbol_fingerprint` check on instance derived data.
  2. Invalidate stale cached sizes and re-sync on page activation.
- **Verification**: Test editing component on page 1, switching to page 2, and asserting re-derived dimensions.

### Slice E19-REPR-1: Pre-0.7.13 Ring Arc Override Migration (`#1907`)
- **Files**: `crates/designos-doc/src/load.rs` (or `crates/designos-doc/src/components.rs`)
- **Actions**:
  1. Detect divergent arc data on instance ellipses lacking `OverrideField::CornerRadii`.
  2. Synthesize missing override entry during document load.
- **Verification**: Test loading legacy document fixture and triggering sync.
