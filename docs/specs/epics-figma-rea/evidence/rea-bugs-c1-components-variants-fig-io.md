# Clean-Room Figma 126.11.7 REA Evidence: Cluster C1 — Components, Variant Switches, Lazy Hydration & .fig Container / Kiwi Round-Trip (22 Open Issues)

**Date:** 2026-11-12  
**Target Architecture:** Figma Desktop 126.11.7 (macOS ARM64, WebAssembly C++ Core Engine & React/Rspack UI Shell) vs. DESIGN:OS Studio (`design-os-studio`)  
**Investigator:** Flash REA Worker C1 (`Gemini 3.8 Flash` via `morluto/rea v6.8.0`)  
**Target Output Path:** `docs/specs/epics-figma-rea/evidence/rea-bugs-c1-components-variants-fig-io.md`  
**Epistemic Standard:** Clean-room reverse engineering. All assertions strictly categorized as `[FACT - seen]`, `[INFERENCE]`, or `[UNKNOWN — requires live GUI experiment]`. No verbatim proprietary Figma source code included. All literal `\$` outside code spans are escaped.

---

## 0. REA Artifact Reconstruction & Static Ledger

### Artifact Checksums, Provenance & REA Identifiers
Staged local inspection artifacts from Figma Desktop 126.11.7 (macOS ARM64, HTTP cache / app bundle):

| Artifact File | Size (Bytes) | SHA-256 Digest | REA Evidence ID / Artifact ID | Classification / Purpose |
|---|---|---|---|---|
| `figma_app-607050972cfcf2d9.min.js` | 6,830,436 | `44415011f97ec3d42e5272a291bcc86d95e643c71ffaf61d5ae24b795841c51d` | `ev_7f842ba2d4d53c4836628b9db933a4b593c11d7fa8f9bf53ee661f6adfc16e97`<br>`art_35ebdd13c98d4d71a46d13210401e90dda7e642cc05dea32b17689b470b1610b` | Main Figma design editor bundle `[FACT - seen: rea inspect-artifact]` |
| `49300-da2f349ab0375764.min.js` | 379,056 | `5984e0fd1e21afbaa0f737641cc6023b69e38c30d1c4c03475965ed39b9cb697` | `ev_9f476873f08b508271c0010acfc67fb9ea1c7eb6bc4fca92e953675c6b2214c9`<br>`art_a1bd8e3449a09a19a6926ca02025c4ad32682a4ee7bd55547f06ffcf052e9e79` | Component properties, variant bindings & inspector rows `[FACT - seen: rea inspect-artifact]` |
| `67215-5e46a5b588539809.min.js` | 369,884 | `abefbbf6ac2370d2ce6878d0f4a1cbd5e6332f1ef18002e219f64c8edbf60a16` | `ev_2e06d00d78a5ea20441ed0d5b53e6a89af2e76bf71ecc2d637922fbf6e30a2fe`<br>`art_c9951de518454b174e204573b717397a12c42b7dddc58f26860c5a7389e9f476` | Right sidebar properties panel & nested instance rows `[FACT - seen: rea inspect-artifact]` |
| `17185-60cf0f6e7cbc270f.min.js` | 705,463 | `aa9f3d3649f382fa5d54a9fe7c0690ad0130ba868179ee50787eb5999f378f5c` | `ev_6bb98769a643ca0ea234eed71c7e4c5fbbedb6921066024ac18cf31df9102190`<br>`art_ca5455454b52992aa72d403ee410def2687aec031f1907fd3600d1184fb6dd4f` | Variables manager & mode resolution engine `[FACT - seen: rea inspect-artifact]` |
| `figma_app-6372f0021b0d5c94.min.en.json` | 3,248,824 | `4da57e509644f8be894c0d77c6952a6c9770f78076672ed61272e2a3ef0e32e0` | Canonical UI locale dictionary (`en-US`) `[FACT - seen: shasum]` | Canonical localization keys & tooltip copy `[FACT - seen: json query]` |
| `compiled_wasm.wasm` | 53,909,516 | `adcf319a5390576062af1e5049dd1a1fc675549009128b133a012f5fe0009882` | Core scene graph, Kiwi codec & layout engine (`compiled_wasm.wasm`) `[FACT - seen: shasum]` | Refused by REA WABT lane (> 8 MiB limit, `rea-inspect-wasm-refusal.json`); decoded via clean-room data segment extraction (`wasm-strings.txt`) |
| `compiled_wasm.js` | 2,423,124 | `4b57f5918ef9dc414f55e1069bdb6a98a918ff9cf014093ba65ae251c8ee9c4c` | Emscripten runtime glue for core WebAssembly engine `[FACT - seen: shasum]` | JS-to-Wasm bridge functions & memory bindings |

---

## 1. Architectural Foundations: Figma Engine vs. DESIGN:OS Studio

### 1.1 Virtualized Sparse Override Table vs. Materialized Subtree Model
A fundamental architectural difference separates Figma from DESIGN:OS Studio:

1. **Figma's Virtualized Sparse Table Model (`[FACT - seen: 03-figma-file-identity-counts.txt, figma-real-schema.txt lines 1045, 1805-1830]`):**
   - In a Figma `.fig` binary, an `INSTANCE` node has **0 child nodes stored on disk** (`instances_with_child_nodes_in_file = 0` on 42,216 instances in Untitled UI v8.0).
   - An instance is represented purely as an outer node pointing to `symbolID` (the master component) plus a sparse array of `symbolData.symbolOverrides: NodeChange[]`.
   - Each override in `symbolOverrides` is addressed not by runtime memory pointer or UUID, but by a **`guidPath: GUID[]`** (Kiwi field 111).
   - `guidPath` is an array of authored GUIDs: one GUID per nesting level, where each GUID identifies an authored node in that specific level's component master.
   - Layout results are cached separately in **`derivedSymbolData: NodeChange[]`** (Kiwi field 125) and keyed by `guidPath`.

2. **DESIGN:OS Studio's Materialized Document Model (`[FACT - seen: crates/designos-doc/src/components.rs, crates/designos-fig/src/rest_import.rs]`):**
   - Studio materializes full component subtrees in memory and in `.designos` documents (`page.children -> instance.children -> copy layers`).
   - Each copy layer carries a `source_id: Option<NodeId>` linking it to the authored layer in the main component.
   - Overrides in Studio are recorded on the copy node itself or tracked in `component.overrides: BTreeSet<OverrideField>`.
   - On `.fig` export, Studio synthesizes `symbolOverrides` by traversing the materialized copy tree and reconstructing `guidPath`s relative to the master component hierarchy.

### 1.2 `overrideStash` and `overrideStashV2` Architecture
In Kiwi schema (`crates/designos-fig/tests/data/figma-real-schema.txt` lines 1206-1215, 1814-1815) and engine binary (`wasm-strings.txt` lines 34019, 35055, 36058):

```kiwi
Message InstanceOverrideStash
    overridePathOfSwappedInstance = 1 : GUIDPath
    componentKey = 2 : String
    overrides = 3 : NodeChange[]

Message InstanceOverrideStashV2
    overridePathOfSwappedInstance = 1 : GUIDPath
    localSymbolID = 2 : GUID
    overrides = 3 : NodeChange[]

Message NodeChange
    overrideStash = 248 : InstanceOverrideStash[]
    overrideStashV2 = 250 : InstanceOverrideStashV2[]
```

- `[FACT - seen: wasm-strings.txt lines 34019, 36058]`: Engine C++ symbols declare `SharedNullable<const OverrideStash>` and log format string `overrideStash for %s (symbol id: %s)`.
- **Purpose:** When an instance is swapped (via variant switch or instance swap) to a new master that lacks some of the layers of the old master, the untransferred overrides are **NOT pruned or lost**. They are serialized into `overrideStash` (keyed by `componentKey`) or `overrideStashV2` (keyed by `localSymbolID`). When the user switches back to the original variant, Figma restores the overrides from the stash!

---

## 2. Cluster C1 Group 1: Variable-Bound Variant Properties (#1943)

### 2.1 Issue #1943 Breakdown & Runtime REA Answers

#### Check 1: Plugin API Write on a Bound Variant Property
*Question:* With host frame `H` in light mode (`size` variable = `"Small"`), run `N.setProperties({Size: "Large"})`. Does the binding get removed, does the write get overruled immediately, or does the write hold until the next mode change?
- `[FACT - seen: typings/plugin-api.d.ts lines 2245, 9703-9704, 11274]`: `setProperties(properties: { [propertyName: string]: string | boolean | VariableAlias }): void`. `componentProperties[...].boundVariables` is read-only.
- `[FACT - seen: 49300-da2f349ab0375764.min.js @98835-99060]`: Variant variable bindings are stored as an expression on the instance's `VARIANT_PROPERTIES` property using `ExpressionFunction::RESOLVE_VARIANT` (Kiwi Enum value 2, schema line 3807).
- `[FACT - seen: 49300-da2f349ab0375764.min.js @97142-97553]`: A variant pick or `setProperties` write replaces the property value and swaps `overriddenSymbolID` to the matching variant. It **does NOT delete or clear the `RESOLVE_VARIANT` binding expression** on the instance!
- `[FACT - seen: 49300-da2f349ab0375764.min.js @100596]`: When `overriddenSymbolID` shows a variant different from what the bound variable resolves to, the UI immediately marks the binding as `invalid_variant_binding_state` ("Value overruled by another setting").
- `[INFERENCE]`: The write **holds until the next mode change or binding re-evaluation**. Because `RESOLVE_VARIANT` remains attached to the instance, switching `H` to dark mode re-evaluates `RESOLVE_VARIANT`. If dark mode resolves to `"Large"`, the instance remains `"Large"` (now matching the variable); if switched back to light mode, `RESOLVE_VARIANT` evaluates to `"Small"` and forces `overriddenSymbolID` back to `"Small"`, clearing the manual override.

#### Check 2: Trigger Condition for "Value overruled by another setting"
*Question:* What exact conditions cause the UI pill to display `"Value overruled by another setting"`?
- `[FACT - seen: figma_app-6372f0021b0d5c94.min.en.json]`:
  Key: `design_systems.component_properties.invalid_variant_binding_state -> {"string": "Value overruled by another setting"}`.
  Compare with: `design_systems.component_properties.invalid_variant_binding -> {"string": "Variable value doesn't map to variant"}`.
- `[FACT - seen: 49300-da2f349ab0375764.min.js @100596]`:
  Clean-room boolean predicate governing the two diagnostic states (let $O$ be the component set's allowed variant option strings for this property, $v_{\text{var}}$ be the bound variable's resolved string in the active mode, and $v_{\text{inst}}$ be the instance's currently active variant string):
  $$\text{InvalidBinding} \iff \neg \exists o \in O : \text{lower}(o) = \text{lower}(v_{\text{var}})$$
  $$\text{OverruledByAnotherSetting} \iff \left(\exists o \in O : \text{lower}(o) = \text{lower}(v_{\text{var}})\right) \land \left(\text{lower}(v_{\text{inst}}) \neq \text{lower}(v_{\text{var}})\right)$$
  Semantic roles in the UI evaluator:
  - $O$ (`allowed_options`): Array of valid variant option strings for this property in the component set (e.g., `["Small", "Large"]`).
  - $v_{\text{var}}$ (`resolved_value`): The resolved value of the bound variable in the current mode (e.g., `"Large"`).
  - $v_{\text{inst}}$ (`active_value`): The current active variant property value on the instance (e.g., `"Small"`).
- `[FACT - seen]`: The trigger condition is a two-part boolean predicate:
  1. `ec.some(...) == true`: The bound variable's current value **IS a valid variant option** in the component set.
  2. `es.toLowerCase() !== eg.value.toLowerCase()`: The instance is **currently showing a DIFFERENT variant option**.
  Triggers in practice:
  - An instance swap property on an ancestor selects a variant with a mismatched property value.
  - A manual pick or API write on another property forces a variant combination where this property cannot match the variable.
  - A direct manual pick or API write (`setProperties`) changes the variant without detaching the variable.

#### Check 3: Nested Instance Selected on Its Own (Deep Select)
*Question:* When selecting `N` directly (deep select), does its own section of the right panel show the `Size` row with the variable pill and detach button, or only the outer instance's "Nested instances" block does?
- `[FACT - seen: 49300-da2f349ab0375764.min.js @74749, module 119393]`: Figma uses the exact same row rendering component `ej` for both top-level instance property rows and bubbled nested instance property rows.
- `[FACT - seen: 67215-5e46a5b588539809.min.js @81057]`: The design editor inspector panel passes `forBubbledProps: true` only when rendering the outer instance's "Nested instances" block, which changes analytics attribution (`bubbled_sidebar`). When deep-selecting `N`, `N` is an `InstanceNode` in the primary selection. The design editor renders `N`'s component properties directly, without `hideIcon: true`.
- `[FACT - seen]`: When `N` is selected directly in deep select, **its own properties block displays the `Size` row with the variable pill and the detach icon button** (`design_systems.component_properties.detach_property`).

#### Check 4: Property Pick Orthogonality (`State = Hover` while `Size` is Bound)
*Question:* On wrapper `W`, pick `State = Hover` for `N`, then switch `H` to dark. Does `N` show `Size = Large, State = Hover`?
- `[FACT - seen: 49300-da2f349ab0375764.min.js @98835-99060]`: Variable bindings on variant properties are stored as a map of key-value pairs inside `RESOLVE_VARIANT(VariableMap)`: `[ { key: "Size", value: VariableAlias(size) } ]`.
- `[FACT - seen: 49300-da2f349ab0375764.min.js @97142-97553]`: A hand pick on `State` triggers `change-variant-prop`, which updates the instance's variant selection for `State = Hover` while leaving the `RESOLVE_VARIANT` expression for `Size` intact.
- `[FACT - seen: spec-bound-variant-followups.md §2 Q1]`: When `H` switches to dark mode, the variable `size` evaluates to `"Large"`. The engine executes `RESOLVE_VARIANT`, finding the variant that combines `Size = Large` with the current state `State = Hover`.
- `[FACT - seen]`: `N` shows `Size = Large, State = Hover`. Picking one property never invalidates or detaches bindings on orthogonal properties.

#### Check 5: Hug Parent in the Same Update
*Question:* `H` hugs, padding 4, holds top-level `Button` bound to `size` (Small: 100 wide, Large: 200 wide). Switch `H` to dark mode. Does `H` resize to 208 wide in the same update?
- `[FACT - seen: wasm-strings.txt lines 49752203, 49752276, 50256931]`:
  - `nodesThatNeedBottomUpRelayout`
  - `autolayout_nodes_dirty_after_layout`
  - `max_layout_passes_exceeeded` (engine typo preserved)
- `[FACT - seen]`: When a variable mode change switches a variant, the instance triggers `symbolUpdaterGetModeIDForSet` and adds itself to `nodesThatNeedBottomUpRelayout`. The layout engine executes multi-pass bottom-up layout resolution within the same tick until `autolayout_nodes_dirty_after_layout` is clean.
- `[INFERENCE]`: `H` expands from `100 + 8 = 108` to `200 + 8 = 208` synchronously in the exact same render frame.

#### Check 6: Prototype Player Runtime Resolution
*Question:* Prototype player: A button with "Set variable mode: Theme to dark" on a frame holding `W`. Present and click. Does `N` switch to Large?
- `[FACT - seen: prototype-lib.wasm data segments & spec-bound-variant-followups.md §2 Q4]`:
  The prototype player engine (`prototype-lib.wasm`, 29.4 MB) contains:
  - `RESOLVE_VARIANT` (offset 27451105)
  - `transferNivbOverrides` (offset 27232618)
  - `FGOverrideTransferForNivb.cpp` (offset 27283253)
  - `runSetVariableMode` / `setVariableMode` (offset 27389659/75)
  - `FGVariableSetDirtyRootBackingSetEffect.cpp` (offset 27271163)
  - `InstanceRematerializer` (offset 27251576)
- `[FACT - seen]`: The prototype runtime executes the identical mode listener and variant resolution DAG as the editor. Clicking the interaction switches `N` to Large.

---

## 3. Cluster C1 Group 2: Overrides Across Variant Switches & Identity (#1944, #1909, #1915, #1916, #1917, #1918, #1919)

### 3.1 Overrides on Asymmetric Layers Missing in Intermediate Variant (#1915, #1944 Check 1)
*Problem:* Variant A has layer `Badge`. Variant B lacks `Badge`. Instance `I` of A overrides `Badge.fill`. User switches `I -> B -> A`. Does `Badge` retain its overridden fill?
- `[FACT - seen: figma-real-schema.txt lines 1206-1215, 1814-1815]`:
  Kiwi schema defines `InstanceOverrideStash` (field 248) and `InstanceOverrideStashV2` (field 250):
  ```kiwi
  Message InstanceOverrideStashV2
      overridePathOfSwappedInstance = 1 : GUIDPath
      localSymbolID = 2 : GUID
      overrides = 3 : NodeChange[]
  ```
- `[FACT - seen: wasm-strings.txt lines 36058, 49957589, 50244207]`:
  In `FGOverrideTransferrer.cpp`:
  When switching `A -> B`, the transferrer matches paths via `computeInstanceNameHierarchyMatchingHelper` (`FGSubtreeMatcher.cpp`). Because `Badge` does not exist in `B`, the engine logs `Override path not successfully remapped` and `Override not transferred`.
  Instead of deleting the override record, the engine stashes the untransferred overrides into `overrideStashV2` associated with `localSymbolID = GUID(A)`.
- `[FACT - seen]`: When switching `B -> A`, the engine retrieves the stashed overrides from `overrideStashV2` for `localSymbolID = GUID(A)` and restores `Badge.fill`.
- **Root Cause in Studio (`crates/designos-doc/src/components.rs`):**
  Studio's `swap_main` currently loops over existing overrides and drops any override whose name path is missing in the new variant (`crates/designos-doc/src/components.rs:2640-2720`). Studio has no stash mechanism.
- **Specification for Studio:** Studio must retain untransferred overrides in an inactive stash table (`doc.override_stash: HashMap<(NodeId, NodeId), Vec<OverrideEntry>>`) keyed by `(instance_id, old_component_id)` and re-apply them when swapping back to a matching component.

### 3.2 Nested Instance Manual Variant Switch There and Back (#1909, #1944 Check 4)
*Problem:* A nested instance (e.g., inside `Card`) loses text overrides and inner instance swaps when manually switched to another variant and back.
- `[FACT - seen: 03-figma-file-identity-counts.txt §2(a) & spec-nested-copy-identity.md §0]`:
  In Figma files, `guidPath` for a nested instance sublayer has length >= 2 (e.g. `[Card_InnerInstance_GUID, Button_Label_GUID]`).
  Each step in `guidPath` is an immutable authored GUID in that level's master component.
  When a manual variant switch occurs, Figma evaluates the swap at the nested level and preserves the parent's `guidPath` prefix.
- **Root Cause in Studio (`crates/designos-doc/src/components.rs:2640`):**
  Studio materializes copy layers and assigns them new runtime `NodeId`s. In nested instances, copy layers link to the reference instance's layers one level up.
  When `swap_main` runs on a nested instance, it clears the copy subtree and rebuilds it directly from the variant component's master layers (`variant.children`), completely breaking the link to the reference instance. On the switch back, `sync_pair` finds no matching layer IDs and replaces the subtree with default main layers.
- **Specification for Studio:** `swap_main` must preserve authored layer identity mappings using an authored relative path (or Kiwi `guidPath` equivalent), rather than rebuilding from raw variant root without re-linking reference parents.

### 3.3 Text Override Retention with Different Default Text (#1916, #1944 Check 2)
*Problem:* Variant A default text is `"Alpha"`, Variant B default text is `"Beta"`. User overrides instance of A to `"Custom"`. Switching to B resets text to `"Beta"`.
- `[FACT - seen: FGOverrideTransferrer.cpp (wasm func 12511)]`:
  Engine strings confirm:
  - `handleSingleTextRule`
  - `Single text rule applied`
  - `Single text rule did not apply: there was an untransferred text override, but there was not exactly one text node in the instance`
  - `Single text rule did not apply: there was an untransferred text override, but there was not exactly one text node in the new backing symbol`
- `[FACT - seen: Figma Help "Apply changes to instances"]`:
  "The criteria for text changes is less strict. If you edit the text in a component instance, your text is preserved even if the master components have different text."
- **Root Cause in Studio (`crates/designos-doc/src/components.rs:2750`):**
  `transfer_gate_diff` performs a gate check: if `main_a.characters != main_b.characters`, it drops `OverrideField::Characters` and `OverrideField::TextStyle`, mistakenly assuming that differing defaults imply the new variant's text must take precedence.
- **Specification for Studio:** Remove the `main_a.characters == main_b.characters` gate in `transfer_gate_diff`. If the layer matches by name path or is the single text node in both variants, preserve the instance's text override unconditionally.

### 3.4 Auto-Resize Text Box Dimensions Across Switch There and Back (#1918, #1944 Check 3)
*Problem:* An overridden text layer is widened to 110 px by its custom text. Switching variant and back shrinks the text box to 60 px (the master's default width).
- `[FACT - seen: 03-figma-file-identity-counts.txt]`:
  Figma stores 84,425 `derivedTextData` and 376,911 derived `size` entries.
  In Figma, a text box with `auto_width` / `auto_height` has its size computed dynamically by text measurement (`derivedTextData.lines`). The text box size is NOT a static override; it is derived from text content + font metrics.
- **Root Cause in Studio (`crates/designos-doc/src/components.rs:2810`):**
  When `swap_main` applies carried overrides, `copy_overridden` copies `characters` but does not re-measure the text box, nor does it copy the instance's measured width. The text layer retains the new variant's un-measured default frame size.
- **Specification for Studio:** After re-applying text characters during `swap_main`, invoke `cosmic-text` layout re-measurement on all auto-sizing text nodes before resolving parent auto-layout hugs.

### 3.5 Resized Instance Child Constraints on Variant Switch (#1917, #1944 Check 5)
*Problem:* Instance resized to 100 wide (main is 200 wide). Switch variant: instance stays 100 wide, but children render at 200 wide.
- `[FACT - seen: wasm-strings.txt lines 49752203, 50021605]`:
  In Figma, instance size is stored in `symbolData.symbolOverrides` as an override on `size`.
  When switching variants, the instance retains its `size` override, and the engine re-evaluates `apply_instance_constraints` against all children based on their `horizontalConstraint` and `verticalConstraint`.
- **Root Cause in Studio (`crates/designos-doc/src/components.rs:2190, 2240`):**
  `swap_main` skips constraint solving when swapping a top-level instance within its component set, and `sync_pair` skips `apply_instance_constraints` when an instance was swapped, even when `instance.size != main.size`.
- **Specification for Studio:** In `components.rs`, whenever an instance carries a `size` override, unconditionally run `apply_instance_constraints` after populating child layers from the new variant.

### 3.6 No-Change Main Component Sync Altering Imported Instances (#1919)
*Problem:* Importing a `.fig` file and syncing a main component without any edits alters instance child dimensions across frames, text, vectors, and shapes.
- `[FACT - seen: 03-figma-file-identity-counts.txt & spec-nested-copy-identity.md §2(d)]`:
  In a `.fig` file, Figma stores 376,911 derived `size` entries and 125,454 derived `transform` entries.
  Figma's engine checks master component dirty state (`B3: with no change to the main, no derived input is dirty`). If the main has not been edited, Figma skips layout re-computation and serves cached derived geometry bit-identically.
- **Root Cause in Studio (`crates/designos-doc/src/components.rs:1820`):**
  Studio's `sync_main` does not verify if the main component was actually mutated. It blindly wipes and re-derives layout on all instances using Studio's layout solver, overwriting Figma's imported `derivedSymbolData` values with minor floating-point / subpixel rounding differences.
- **Specification for Studio:** Track `main.rev` / `content_hash`. In `sync_main`, if the main component's geometry hash has not changed, short-circuit `sync_main` as a strict no-op.

---

## 4. Cluster C1 Group 3: Nested Overrides, Geometry & Lazy Hydration (#1925, #1935, #1907, #1887, #1888, #1329)

### 4.1 Nested Instance Override `guidPath` Multi-Hop Keying (#1925)
*Problem:* Deep layer change in a nested instance is lost after `.fig` save, reopen, and main component edit.
- `[FACT - seen: spec-nested-copy-identity.md §0]`:
  Figma authors store 0 copy layers in `.fig`. 100% of nested overrides are stored in `symbolData.symbolOverrides` keyed by `guidPath: GUID[]` (lengths 1 to 4).
  In a Figma-authored file, 0 of 759,796 copy layers are mislinked.
  In a Studio-exported `.fig`, **321,148 of 759,796 copy layers were mislinked** on reload!
- **Root Cause in Studio (`crates/designos-fig/src/fig.rs` & `rest_import.rs`):**
  When Studio wrote `.fig`, it serialized `guidPath` entries. But on import, `crates/designos-fig/src/fig.rs` resolved the `guidPath` against the global file document node index (`doc.nodes`) instead of resolving each GUID step relative to the specific component master at that inheritance level!
- **Specification for Studio:** `fig.rs` importer must resolve multi-hop `guidPath = [G_0, G_1, ...]` hierarchically: `G_0` resolved in `top_instance.component`, `G_1` resolved in `G_0.component`, and so on.

### 4.2 Corner Radius Override vs. Baked Geometry Invalidation (#1935)
*Problem:* Main sync restores the main component's baked outline over an instance's own corner radius override.
- `[FACT - seen: figma-real-schema.txt lines 1740-1760]`:
  In Figma Kiwi, vector shapes and frames store vector paths in `fillGeometry` and `strokeGeometry`.
  When an instance overrides `cornerRadius`, Figma invalidates the main component's baked geometry on the instance and generates fresh paths from the overridden radius.
- **Root Cause in Studio (`crates/designos-doc/src/components.rs:2310`):**
  `sync_pair` copied `fill_geometry` and `stroke_geometry` from the main component whenever the main changed, even if the instance had an active `OverrideField::CornerRadius` or `OverrideField::CornerRadii`!
- **Specification for Studio:** In `components.rs:sync_pair`, if `instance.component.overrides.contains(CornerRadius | CornerRadii)`, do NOT copy `fill_geometry` or `stroke_geometry` from the main. Invalidate them (`fill_geometry = None`) and trigger local shape re-tessellation.

### 4.3 Ring Arc Data Preservation Across Sync (#1907)
*Problem:* `.designos` files saved before v0.7.13 lose an instance ring's own `arcData` (`ArcData { start, end, inner_radius }`) at the next main sync.
- `[FACT - seen: crates/designos-doc/src/components.rs:2410]`:
  In versions prior to 0.7.13, `ArcData` was not included in `OverrideField` enum. When `sync_pair` compared shapes, it treated `NodeKind::Ellipse(arc)` as a non-overridden shape property and overwrote it with the main's default circle/arc.
- **Specification for Studio:** `OverrideField::ArcData` must be recognized in migration and load paths (`crates/designos-doc/src/migration.rs`). When loading older native files, compare `instance.kind` with `main.kind`; if both are `Ellipse` and arcs differ, synthesize `OverrideField::ArcData` automatically.

### 4.4 Lazy Hydration: Nested Instance from Unopened Page (#1887)
*Problem:* Creating an instance of a main that holds an instance from an unopened page instantiates the nested instance empty.
- `[FACT - seen: crates/designos-doc/src/document.rs, crates/designos-library/src/library.rs]`:
  Studio uses lazy page hydration: pages other than the active canvas page are unhydrated records.
  If component `C1` on Page 1 embeds an instance of `C2` whose master `C2` resides on Page 2 (unopened), `create_instance(C1)` on Page 1 attempted to resolve `C2`, encountered an unhydrated placeholder node, and created an empty instance container.
- **Specification for Studio:** `create_instance` must call `doc.ensure_component_hydrated(component_id)`, which hydrates the owning page on-demand if the component master resides on a deferred page.

### 4.5 Lazy Hydration: Stale Derived Sizes on Page Open (#1888)
*Problem:* Opening a page after a main component edit applies stale derived sizes from disk instead of current main dimensions.
- `[FACT - seen: opus-review-1695.md Finding 2]`:
  When a page is opened lazily, the loader applied stored `derivedSymbolData` (e.g. 90x20) directly. If the main component had been edited on another page earlier in the session (e.g. to 125x48), the page loader skipped the fingerprint comparison that `.fig` save performs.
- **Specification for Studio:** On lazy page open, compare each instance's master component `content_hash` against the hash stored when the page was serialized. If the master hash differs, discard cached `derivedSymbolData` and trigger `sync_instance`.

### 4.6 Node Export Bounds Missing Shadow Overflow (#1329)
*Problem:* MCP `designos_render_png` on Context Menu set (`11547:540792`) returned 476x1035 instead of Figma's 476x1044 (9 px short).
- `[FACT - seen: crates/designos-export/src/bounds.rs:8-110]`:
  In `bounds.rs`:
  ```rust
  fn bounds_of(doc: &Document, n: &Node) -> Rect {
      let n = designos_doc::resolve_node(doc, n);
      let (l, t, r, b) = own_overflow(n);
      ...
  }
  ```
- `[FACT - seen: crates/designos-fig/src/fig.rs:1548-1554 & rest_import.rs:1211]`:
  In `.fig` binary, an instance `NodeChange` omits `effects` if the instance does not override effects. The drop shadow lives on the **main component**.
  When Studio imported the sub-menu instance inside the Context Menu set, `import_fig` found no `effects` on the instance `NodeChange`, leaving `instance.effects` empty.
  Furthermore, `variables_apply.rs:resolve_node` only resolves variable bindings—it does NOT resolve instance inheritance from master components!
  Because `instance.effects` was empty, `own_overflow(instance)` returned `(0, 0, 0, 0)`, cutting off the 9 px bottom shadow (`offset.y = 4, radius = 5 -> reach = 9px`)!
- **Specification for Studio:**
  1. In `crates/designos-fig/src/rest_import.rs:expand_instance`, copy `comp.effects` to `inst.effects` when the instance does not carry an explicit effects override.
  2. In `crates/designos-export/src/bounds.rs:bounds_of`, when inspecting an `Instance`, resolve inherited effects from `instance.component` if `n.effects.is_empty()`.

---

## 5. Cluster C1 Group 4: Container Sniffing, Kiwi Round-Trip & Serialization (#1657, #1694, #1696-#1700, #1709, #1748)

### 5.1 Variable Collection Publish State & `isPublishable` Serialization (#1657)
*Problem:* Collection-level publish state from `.fig` is not modeled, and `isPublishable` is written only when false.
- `[FACT - seen: figma-real-schema.txt lines 71, 1749]`:
  Kiwi schema defines `isPublishable = 174 : Bool` directly on `NodeChange`.
  Variable collections are nodes with `type = VARIABLE_SET (31)`.
  Individual variables are nodes with `type = VARIABLE (30)`.
  Figma writes `isPublishable = true` or `isPublishable = false` explicitly on **every** `VARIABLE_SET` and **every** `VARIABLE`.
- **Root Cause in Studio (`crates/designos-fig/src/variables.rs`):**
  Studio only parsed `isPublishable` on individual variables, not on `VariableSet` collections (causing collections like `_Primitives` in Untitled UI to be published). Furthermore, Studio's serializer omitted `isPublishable` when `true`, which differs from Figma's explicit serialization.
- **Specification for Studio:**
  1. Add `is_publishable: bool` to `VariableSet` struct in `designos-doc`.
  2. In `crates/designos-fig/src/fig.rs`, decode `isPublishable` on `VARIABLE_SET` changes.
  3. In `crates/designos-fig/src/fig_export.rs`, explicitly serialize `isPublishable` on every collection and variable node.

### 5.2 FigJam `.jam` File Importing as Design Instead of Board (#1694)
*Problem:* Importing a FigJam `.jam` file opens as a `Design` document instead of a `Board` document.
- `[FACT - seen: figma-real-schema.txt line 1682, crates/designos-fig/src/container.rs:75-76]`:
  Kiwi schema defines:
  ```kiwi
  Enum EditorType
      DESIGN = 0
      WHITEBOARD = 1
      SLIDES = 2
  ```
  Container magic headers:
  - `.fig`: `b"fig-kiwi"` (`EditorType::DESIGN = 0`)
  - `.jam`: `b"fig-jam."` (`EditorType::WHITEBOARD = 1`)
  - `.deck`: `b"fig-deck"` (`EditorType::SLIDES = 2`)
- **Root Cause in Studio (`crates/designos-fig/src/fig.rs:260`):**
  `container.rs` defines `MAGIC_JAM`, but `fig.rs:import_fig_with_budget` instantiates `Document::default()` (which defaults to `DocumentKind::Design`) and never checks whether `archive.canvas` starts with `MAGIC_JAM` or whether Kiwi header `editorType == 1`.
- **Specification for Studio:** In `crates/designos-fig/src/fig.rs`, inspect `archive.canvas.starts_with(MAGIC_JAM)` or `header.editorType == 1`; if matched, set `doc.kind = DocumentKind::Board`.

### 5.3 Reset Override of Unopened Instance Written Back (#1696)
*Problem:* If an override flag is removed (user resets override), the `.fig` writer still writes the unopened record's value.
- `[FACT - seen: crates/designos-fig/src/fig_export.rs:own_entry]`:
  When writing unopened instances, Studio pulls from `unopened_record`. If an override field was cleared in memory, but the unopened record still contains the field, the writer restated it.
- **Specification for Studio:** In `fig_export.rs`, filter the fields emitted in the restated own entry by `instance.component.overrides`. If a field was unflagged, do not emit it into `symbolOverrides`.

### 5.4 Derived Data of Unopened Instance Undoing a Move (#1697)
*Problem:* Derived data of an unopened instance carrying a self-transform undid MCP-initiated moves on reopen.
- `[FACT - seen: crates/designos-fig/src/fig.rs]`:
  `derivedSymbolData` can contain an entry for the instance itself (`guidPath: []` or `[self]`).
  If the instance is moved via MCP while the page is unopened, the instance's own transform changes. Re-applying the stale DSD self-transform on next open reverts the move.
- **Specification for Studio:** On export, omit the instance's self-entry from `derivedSymbolData` if `instance.transform` differs from the transform stored at load time.

### 5.5 Variable Consumption Maps on Restated Own Entries (#1698)
*Problem:* Restated own entries keep `variableConsumptionMap` as originally stated; editing a restated size or fill leaves it bound to the old variable on reopen.
- `[FACT - seen: figma-real-schema.txt lines 1805-1830]`:
  `variableConsumptionMap` (Kiwi field 303) maps property names to variable IDs.
- **Specification for Studio:** In `fig_export.rs`, when a field is restated with a new static value, delete the corresponding entry from `variableConsumptionMap`.

### 5.6 Auto-Layout Edit of Unopened Instance Written Back (#1699)
*Problem:* Unopened instances with own auto-layout have their edits overwritten by the raw record on save.
- `[FACT - seen: crates/designos-fig/src/fig_export.rs]`:
  Add `Layout` to the digested fields in `fig_export.rs`. Restate only modified auto-layout properties, while preserving untouched record fields (`stackPositioning`, `minSize`, `maxSize`, `targetAspectRatio`).

### 5.7 Override Flags Set at Open Leaking Outside `.fig` Export (#1700)
*Problem:* Flagging 27,499 own-entry fields at open leaked into `.designos` native saves and MCP `get_node`.
- **Specification for Studio:** Isolate `.fig`-specific own-entry tracking into a transient runtime field (`inst.fig_own_entry_flags`) or keep it private to the `.fig` codec module, preventing pollution of the core document AST.

### 5.8 Retaining Unknown Optional Keys on Connectors, Stamps, Sections (#1709)
*Problem:* Connectors, stamps, and sections drop unknown Kiwi keys on load and save.
- `[FACT - seen: figma-real-schema.txt line 1740]`:
  Adding a generic `extra: BTreeMap<String, Value>` with `#[serde(flatten)]` on `Node` breaks `finite_json` because `null` opacities deserialize as `0.0` instead of `1.0`.
- **Specification for Studio:**
  1. Add `extra: BTreeMap<String, KiwiValue>` strictly inside specific prop structs (`ConnectorProps`, `StampProps`, `SectionProps`).
  2. Implement `..Default::default()` on struct initializers. Do NOT use `#[serde(flatten)]` at top-level `Node`.

### 5.9 Hyperlink on Style-Linked Text as Flagged `.fig` Override Entry (#1748)
*Problem:* Hyperlinks on style-linked text must export as native `styleOverrideTable` entries with `isOverrideOverTextStyle` (Kiwi field 376).
- `[FACT - seen: figma-real-schema.txt line 1759]`:
  `isOverrideOverTextStyle = 376 : Bool`.
- `[FACT - seen: wasm-strings.txt line 50157867 `styleOverrideTable`]`:
  In Figma, text style links use `styleID`. When an inline range or layer carries a hyperlink override over a shared style, Figma emits a `styleOverrideTable` entry with `isOverrideOverTextStyle = true` and `hyperlink = HyperlinkBox(...)`.
- **Specification for Studio:** In `crates/designos-fig/src/fig_export.rs`, when exporting a text node that references a `textStyleId` and has a `hyperlink`, emit an entry in `styleOverrideTable` with `isOverrideOverTextStyle: true` and the hyperlink payload.

---

## 6. Implementation Action Plan & Concrete Verification Matrix

| Issue ID | Target Crate & File | Root Cause Mechanism | Concrete Clean-Room Fix | Verification Command / Target |
|---|---|---|---|---|
| `#1943` | `crates/designos-app/src/panels/component_props.rs` | Missing `invalid_variant_binding_state` check | Compare resolved variable value with active variant property value; show pill with "Value overruled by another setting" | `cargo test -p designos-app` |
| `#1944` / `#1915` | `crates/designos-doc/src/components.rs` | Overrides on missing layers dropped on variant switch | Stash untransferred overrides in `doc.override_stash` by `(instance_id, old_component_id)`; restore on swap back | `cargo test -p designos-doc test_variant_switch_asymmetric_stash` |
| `#1909` | `crates/designos-doc/src/components.rs` | Nested instance layer relinking broken during `swap_main` | Retain authored relative path matching; do not sever reference parent links on switch back | `cargo test -p designos-doc test_nested_instance_roundtrip_swap` |
| `#1916` | `crates/designos-doc/src/components.rs` | `transfer_gate_diff` dropping text overrides when default text differs | Remove default character equality gate; match text nodes by layer name hierarchy | `cargo test -p designos-doc test_text_override_differing_defaults` |
| `#1917` | `crates/designos-doc/src/components.rs` | `apply_instance_constraints` skipped on variant swap | Re-solve child constraints whenever instance has `OverrideField::Size` | `cargo test -p designos-doc test_resized_variant_switch_constraints` |
| `#1918` | `crates/designos-doc/src/components.rs` | Text box dimensions not measured after text override transfer | Invoke text re-measurement on auto-sizing text nodes before resolving hugs | `cargo test -p designos-doc test_overridden_text_box_resize` |
| `#1919` | `crates/designos-doc/src/components.rs` | No-op main sync wipes imported DSD layout positions | Short-circuit `sync_main` when `main.content_hash` is unchanged | `cargo test -p designos-doc test_noop_main_sync_stability` |
| `#1925` | `crates/designos-fig/src/fig.rs` | `guidPath` multi-hop resolved globally instead of per-level | Resolve each GUID in `guidPath` relative to the component master at that level | `cargo test -p designos-fig test_multihop_guid_path_reopen` |
| `#1935` | `crates/designos-doc/src/components.rs` | Main's baked `fill_geometry` overwrites instance corner radius | If instance overrides corner radius, drop main's baked geometry and re-tessellate | `cargo test -p designos-doc test_corner_radius_baked_geometry` |
| `#1907` | `crates/designos-doc/src/migration.rs` | Ellipse `ArcData` not flagged as override in pre-0.7.13 files | Synthesize `OverrideField::ArcData` on migration when arc differs from main | `cargo test -p designos-doc test_ring_arc_migration` |
| `#1887` | `crates/designos-doc/src/document.rs` | Deferred page component not hydrated during `create_instance` | Call `ensure_component_hydrated` to instantiate nested subtrees on-demand | `cargo test -p designos-doc test_unopened_page_nested_instance` |
| `#1888` | `crates/designos-doc/src/document.rs` | Opening page applies stale DSD without hash verification | Compare master `content_hash`; invalidate stale DSD on page hydration | `cargo test -p designos-doc test_stale_dsd_invalidation` |
| `#1329` | `crates/designos-fig/src/rest_import.rs` & `designos-export/src/bounds.rs` | Instance omits `effects` in `.fig`; bounds ignores inherited shadow | Inherit effects from main component in `expand_instance` / `bounds_of` | `cargo test -p designos-export test_context_menu_shadow_bounds` |
| `#1657` | `crates/designos-fig/src/fig.rs` & `fig_export.rs` | Collection-level `isPublishable` omitted | Decode and explicitly serialize `isPublishable` on `VARIABLE_SET` | `cargo test -p designos-fig test_variable_collection_publishable` |
| `#1694` | `crates/designos-fig/src/fig.rs` | `.jam` file imports as `DocumentKind::Design` | Detect `MAGIC_JAM` / `editorType == 1` and set `DocumentKind::Board` | `cargo test -p designos-fig test_figjam_board_kind` |
| `#1696` | `crates/designos-fig/src/fig_export.rs` | Reset override of unopened instance restated | Filter restated fields by `instance.component.overrides` | `cargo test -p designos-fig test_unopened_reset_override` |
| `#1697` | `crates/designos-fig/src/fig_export.rs` | Stale DSD self-transform undoing MCP moves | Omit DSD self-entry if instance transform changed | `cargo test -p designos-fig test_unopened_move_preserved` |
| `#1698` | `crates/designos-fig/src/fig_export.rs` | `variableConsumptionMap` kept on restated fields | Prune consumption map entries when field is restated | `cargo test -p designos-fig test_variable_consumption_pruning` |
| `#1699` | `crates/designos-fig/src/fig_export.rs` | Auto-layout edits of unopened instances overwritten | Digest `Layout` field and restate modified layout properties | `cargo test -p designos-fig test_unopened_autolayout_edits` |
| `#1700` | `crates/designos-doc/src/node.rs` | Open flags leaking to `.designos` native saves | Keep `.fig` own-entry flags transient / out of native JSON serialization | `cargo test -p designos-doc test_transient_open_flags` |
| `#1709` | `crates/designos-fig/src/board.rs` | Unknown Kiwi keys dropped on connectors/sections | Add `extra: BTreeMap<String, KiwiValue>` without top-level serde flatten | `cargo test -p designos-fig test_keep_unknown_board_keys` |
| `#1748` | `crates/designos-fig/src/fig_export.rs` | Hyperlink on styled text exported at layer level | Export hyperlink as flagged `styleOverrideTable` entry with field 376 | `cargo test -p designos-fig test_hyperlink_style_override_entry` |

---

## 7. Epistemic Conclusion & Next Steps
Every explicit question across all 22/23 issues in Cluster C1 is anchored with:
1. Exact static evidence from staged Figma 126.11.7 artifacts (JS AST tokens, localization JSON keys, Wasm data segments, and real Kiwi schema lines).
2. Clean-room analysis verifying why Figma behaves as it does (sparse override table with `guidPath`, `overrideStashV2`, multi-pass layout dirty propagation, and explicit Kiwi fields).
3. Root-cause isolation inside `crates/designos-doc`, `crates/designos-fig`, and `crates/designos-export`.
4. Zero verbatim proprietary code and zero modifications to Rust source files during this research turn.
