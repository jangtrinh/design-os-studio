# Issue #1477 — Edit Menu Parity: Select Same Text / Style / Variant, Paste and Match Style, Swap Fill and Stroke

## Unverified / Blocked Gate Surface (Reported First per Common Brief)

- **`python3 scripts/zero_fake_ui_gate.py` (`Z16_ux_writing_labels`)**:
  - **What failed**: `16/17 invariants satisfied` (`plans/reports/agy-1477-edit-menu-evidence/fmt-clippy-gate.log:72,75,77`). Only `Z16_ux_writing_labels` failed with:
    `["panels/menus.rs [words] 'Paste and match style'", "panels/menus.rs [words] 'Swap fill and stroke'"]`
  - **Why**:
    1. `brief-agy-1477-edit-menu.txt` Ruling 1 mandates: *"Where a label or shortcut differs from Figma, change it to Figma's."* Figma's exact strings in `18b103ae06d13a46_0__figma_app-b446bd325f74b39b.min.en.json` are `"Paste and match style"` (`@1980312`) and `"Swap fill and stroke"` (`@2010775`) (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:21,30`).
    2. `design/ux-writing.md` Rule 1 and `scripts/ux_writing_labels.py:39` enforce `MAX_ACTION_WORDS = 3` on `leaf("...", ...)` calls in `crates/designos-app/src/panels/menus.rs`, using `scripts/ux_writing_baseline.json` for grandfathered 4-word Figma menu labels (such as `"Select all with same"` at `scripts/ux_writing_baseline.json:67`).
    3. `scripts/ux_writing_baseline.json` is outside `Files you may modify` in `brief-agy-1477-edit-menu.txt:38-42`. Per `brief-common-argon.txt:42-43` (*"Do not touch files outside the brief's file list. If a fix needs another file, stop and ask. If one part is blocked, finish every other part in full and say exactly what you left out and why. ... A conflict between the request and a repo constraint is stop-and-surface information, never an implementation detail to code around"*), we kept the exact 4-word Figma labels in `crates/designos-app/src/panels/menus.rs:532-533` without coding around `ux_writing_labels.py` and without modifying `scripts/ux_writing_baseline.json`.
  - **What is needed from the coordinator**: Either (a) authorize adding `"panels/menus.rs [words] 'Paste and match style'"` and `"panels/menus.rs [words] 'Swap fill and stroke'"` to `scripts/ux_writing_baseline.json`, or (b) rule 3-word labels (`"Paste matching style"` / `"Swap fill, stroke"`) as an explicit exception to Ruling 1.

---

## Step 1 — Re-Anchor in Studio & Reverse-Engineer Figma

### Step 1.1 — Studio Re-Anchor (`origin/main` `e7ace20be2b92423d7d50b899c0ec2ccb5ba9bd9`)

1. **`Select all with same text properties` (`Action::SelectSame(MatchCriteria::TextProperties)`)**
   - **Where it appears**:
     - Edit menu under `Edit > Select all with same` (`crates/designos-app/src/panels/menus.rs:463`, submenu mounted at `crates/designos-app/src/panels/menus.rs:521`).
     - Canvas right-click context menu under `Select all with same` (`crates/designos-app/src/panels/context.rs:43`, submenu mounted at `crates/designos-app/src/panels/context.rs:145`).
     - Command palette via `commands(app)` walking `main_nodes(app)` (`crates/designos-app/src/panels/menus.rs:939-943`).
   - **Label & shortcut**: `"Select all with same text properties"`, shortcut `""` (`crates/designos-app/src/panels/menus.rs:463`, `crates/designos-app/src/panels/context.rs:43`).
   - **Enabled predicate on `origin/main`**: Enabled whenever `!app.selection.is_empty()` (`has_sel` at `crates/designos-app/src/panels/menus.rs:455,463` and `has` at `crates/designos-app/src/panels/context.rs:28,43`), whereas `has_criteria(doc, id, MatchCriteria::TextProperties)` requires `matches!(n.kind, NodeKind::Text(_))` (`crates/designos-doc/src/find.rs:425`).
   - **Behaviour**: Dispatches `Action::SelectSame(c) => app.select_matching(c)` (`crates/designos-app/src/panels/menus.rs:184`) -> `App::select_matching` (`crates/designos-app/src/find/select.rs:55-77`) -> `designos_doc::find::find_same` (`crates/designos-doc/src/find.rs:430-468`), which compares `same_text_properties`: `x.style == y.style && a.style_refs.get("text") == b.style_refs.get("text")` across visible, unlocked `NodeKind::Text` nodes on the active page (`crates/designos-doc/src/find.rs:387-392,436`).

2. **`Select all with same properties` (`Action::SelectSame(MatchCriteria::Properties)`)**
   - **Where it appears**:
     - Edit menu under `Edit > Select all with same` (`crates/designos-app/src/panels/menus.rs:459`).
     - Canvas right-click context menu under `Select all with same` (`crates/designos-app/src/panels/context.rs:39`).
     - Command palette (`crates/designos-app/src/panels/menus.rs:939-943`).
   - **Label & shortcut**: `"Select all with same properties"`, shortcut `""` (`crates/designos-app/src/panels/menus.rs:459`, `crates/designos-app/src/panels/context.rs:39`).
   - **Enabled predicate**: Enabled when `!app.selection.is_empty()` (`crates/designos-app/src/panels/menus.rs:455,459`, `crates/designos-app/src/panels/context.rs:28,39`), and `has_criteria` filters out `Page`, `Group`, and `Slice` (`crates/designos-doc/src/find.rs:420-422,428`).
   - **Behaviour**: `same_properties(a, b)` compares `same_kind`, `same_fill`, `same_stroke`, `same_effects`, `corner_radii`, `corner_smoothing`, `opacity`, `blend`, `is_mask`, `style_refs`, and per-kind `TextProps::style` / `FrameProps::clips_content` across visible, unlocked layers on the active page (`crates/designos-doc/src/find.rs:403-418,450`).

3. **`Select all with same variant` (`Action::SelectSame(MatchCriteria::Variant)`)**
   - **Where it appears**:
     - Edit menu under `Edit > Select all with same` (`crates/designos-app/src/panels/menus.rs:466`).
     - Canvas right-click context menu under `Select all with same` (`crates/designos-app/src/panels/context.rs:46`).
     - Command palette (`crates/designos-app/src/panels/menus.rs:939-943`).
   - **Label & shortcut**: `"Select all with same variant"`, shortcut `""` (`crates/designos-app/src/panels/menus.rs:466`, `crates/designos-app/src/panels/context.rs:46`).
   - **Enabled predicate**: `designos_doc::find::can_select_same_variant(&app.doc, &app.selection)` (`crates/designos-app/src/panels/menus.rs:457,466`, `crates/designos-app/src/panels/context.rs:30,46`), which checks `selection.len() == 1 && variant_set_of(doc, selection[0]).is_some()` (`crates/designos-doc/src/find.rs:352-357,367-369`).
   - **Behaviour**: Resolves `ref_set = variant_set_of(doc, r)` (the `NodeKind::ComponentSet` parent of the selected instance's main component) and selects all visible, unlocked `NodeKind::Instance` nodes on the active page whose main component has `parent == Some(ref_set)` (`crates/designos-doc/src/find.rs:427,444-447`).

4. **`Paste and match style`**
   - **Studio on `origin/main`**: Absent from `crates/designos-app/src/panels/menus.rs`, `crates/designos-app/src/panels/context.rs`, `crates/designos-app/src/ops.rs`, and `crates/designos-app/src/shell.rs`.
   - **Adjacent paste commands in Studio**:
     - `Paste` (`⌘V`, `Action::Paste` at `crates/designos-app/src/panels/menus.rs:225-231,508`, `App::paste_from_system` at `crates/designos-app/src/ops.rs:727-737`).
     - `Paste Over Selection` (`⇧⌘V`, `Action::PasteOverSelection` at `crates/designos-app/src/panels/menus.rs:232,509`, `crates/designos-app/src/shell.rs:956-959`, `App::paste_over_selection` at `crates/designos-app/src/ops.rs:805-852`).
     - `Paste to Replace` (`⇧⌘R`, `Action::PasteToReplace` at `crates/designos-app/src/panels/menus.rs:233,510`, `crates/designos-app/src/shell.rs:960-963`, `App::paste_to_replace` at `crates/designos-app/src/ops.rs:856-922`).
     - `Copy Properties` / `Paste Properties` (`⌥⌘C` / `⌥⌘V`, `Action::CopyProperties` / `Action::PasteProperties` at `crates/designos-app/src/panels/menus.rs:234-235,514-515`, `crates/designos-app/src/shell.rs:964-971`, `App::copy_properties` / `App::paste_properties` at `crates/designos-app/src/ops.rs:926-976`).

5. **`Swap fill and stroke`**
   - **Studio on `origin/main`**: Absent from `crates/designos-app/src/panels/menus.rs`, `crates/designos-app/src/panels/context.rs`, `crates/designos-app/src/ops.rs`, and `crates/designos-app/src/shell.rs`.

---

### Step 1.2 — Figma Facts (REA + Web Bundle + WASM Disassembly)

- **REA Static Analysis Evidence**:
  - Executed `/opt/homebrew/bin/rea analyze-javascript-application` on the extracted Figma desktop + web bundle target (`evidence_id: ev_9ce6d706cab536374076b47814259cbc3d6d4b7ab7df4eb4257dde6a889d93cd`, `inventory_manifest_id: agm_a0cb3e6d340cde62e9cbb171df7970b4287b4458f8882ca4e0467602b6fb73e0`, `graph_id: jsrg_b5388bd2f8353a56f916248a571dc2e94a05679f0ce865808755b402c364d71b`) (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:1-18`).
- **Exact Labels (`18b103ae06d13a46_0__figma_app-b446bd325f74b39b.min.en.json`)**:
  - `fullscreen_actions.paste-and-match-style` (`@1980312`): `"Paste and match style"` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:21`).
  - `fullscreen_actions.select-same-style` (`@1995736`): `"Select all with same properties"` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:22`).
  - `fullscreen_actions.select-same-fill` (`@1995416`): `"Select all with same fill"` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:23`).
  - `fullscreen_actions.select-same-stroke` (`@1995655`): `"Select all with same stroke"` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:24`).
  - `fullscreen_actions.select-same-effect` (`@1995335`): `"Select all with same effect"` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:25`).
  - `fullscreen_actions.select-same-text` (`@1995820`): `"Select all with same text properties"` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:26`).
  - `fullscreen_actions.select-same-font` (`@1995493`): `"Select all with same font"` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:27`).
  - `fullscreen_actions.select-same-instance` (`@1995570`): `"Select all with same instance"` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:28`).
  - `fullscreen_actions.select-same-variant` (`@1995908`): `"Select all with same variant"` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:29`).
  - `fullscreen_actions.swap-fill-and-stroke` (`@2010775`): `"Swap fill and stroke"` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:30`).
- **Menu Structure & Ordering (`9a625d73e63287bd_0__85743-236ad693470d06fb.min.js` & `b14e95537840f572_0__23242-a098d58d7265f1cd.min.js`)**:
  - In `9a625d73e63287bd_0__85743-236ad693470d06fb.min.js @77758..82500` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:33,1403`), the Edit menu groups:
    1. `undo`, `redo`, separator
    2. `cut`, `copy`, `copy-as-menu`, `paste`, `paste-over-selection`, `paste-to-replace`, `duplicate-in-place`, `delete-selection`, separator
    3. `canvas-search`, `canvas-search-next`, `canvas-search-prev`, `canvas-search-replace`, separator
    4. `set-default-style`, `copy-properties`, `paste-properties` (with `paste-and-match-style` in the copy/paste/style group; see `b14e95537840f572_0__23242-a098d58d7265f1cd.min.js @576300..576800` at `plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:1406,1409`), separator
    5. `toggle-dropper`, `swap-fill-and-stroke` (`9a625d73e63287bd_0__85743-236ad693470d06fb.min.js @100250..100600`, `flags: ["design","edit","slides","cooper","sites"]`, `searchSynonyms: ["switch fill stroke"]` at `plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:34`; placed in the Edit menu and command palette per Ruling 2), separator
    6. `select-all`, `select-matching`, `deselect-all`, `select-inverse`, `select-all-with-menu-options` (`@81005`) containing in exact order:
       - `select-same-style` (`"Select all with same properties"`)
       - `select-same-fill` (`"Select all with same fill"`)
       - `select-same-stroke` (`"Select all with same stroke"`)
       - `select-same-effect` (`"Select all with same effect"`)
       - `select-same-text` (`"Select all with same text properties"`)
       - `select-same-font` (`"Select all with same font"`)
       - `select-same-instance` (`"Select all with same instance"`)
       - `select-same-variant` (`"Select all with same variant"`)
- **Keyboard Shortcuts & Clash Resolution (`d48a6add9e1dc400_0__compiled_wasm.wasm @48425800..48428400`)**:
  - `swap-fill-and-stroke`: `"Shift+X"` (`⇧X`) in `["GLOBAL", "!TEXT"]` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:1407`).
  - `select-same-*`: no keyboard shortcuts (`""`).
  - `paste-and-match-style`: In Figma's context menu (`b14e95537840f572_0__23242-a098d58d7265f1cd.min.js @576300..576800`), `paste-and-match-style` displays `${L}\u21E7V` (`⇧⌘V`) during text edit mode, while in Figma's global shortcut table (`d48a6add9e1dc400_0__compiled_wasm.wasm @48425812..48425995`, `plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:1407`), `"Meta+Shift+V"` (`⇧⌘V`) is bound to `"paste-over-selection"`, `"Meta+Alt+V"` (`⌥⌘V`) is bound to `"paste-properties"`, and `"Meta+Shift+Alt+V"` (`⇧⌥⌘V`) is bound as the alternate paste chord. In Studio, `⇧⌘V` is already bound to `Paste Over Selection` (`crates/designos-app/src/shell.rs:957`, `crates/designos-app/src/panels/menus.rs:527`), `⇧⌘R` is bound to `Paste to Replace` (`crates/designos-app/src/shell.rs:962`), and `⌥⌘V` is bound to `Paste Properties` (`crates/designos-app/src/shell.rs:970`). Per Ruling 1, we keep `⇧⌘V` on `Paste Over Selection`, bind `⇧⌥⌘V` (`Ctrl+Shift+Alt+V`) to `Paste and match style`, and document this shortcut clash resolution here.
- **Enabled Predicates (`a6f2bcc873943b57_0__6413-8e031fdc4fad2bf8.min.js @1318261..1321696, @1865900..1867750`)**:
  - `swap-fill-and-stroke` (`enabled: eO`, `@1321096`, `@1865924`): `(e.recursiveSelectionCounts.geometryNodeCount > 0 || e.recursiveSelectionCounts.frameCount > 0) && 0 === e.recursiveSelectionCounts.widgetCount && ...` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:38,1410`).
  - `select-same-style`, `select-same-fill`, `select-same-stroke`, `select-same-effect` (`enabled: eC`, `@1318261`, `@1866197..1866950`): `e.recursiveSelectionCounts.geometryNodeCount > 0 && e.appFrameworkState.isSelectionMode` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:38,1411`).
  - `select-same-text`, `select-same-font` (`enabled: ek`, `@1318345`, `@1866950..1867250`): `e.recursiveSelectionCounts.textNodeCount > 0 && (e.appFrameworkState.isSelectionMode || e.appFrameworkState.editMode === B.m1T.TEXT)` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:38,1411`).
  - `select-same-instance` (`@1867300`): `e.singleNodeSelection?.isOrInInstance === !0 && e.appFrameworkState.isSelectionMode` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:38,1405`).
  - `select-same-variant` (`@1867450`): `e.singleNodeSelection?.isInstanceBackedByStateSymbol === !0 && e.appFrameworkState.isSelectionMode` (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:1405`).
- **WASM Semantics (`d48a6add9e1dc400_0__compiled_wasm.wasm`)**:
  1. **`swap-fill-and-stroke` (`func[52668]` `@47140518..47140779` -> `func[40384]` `@37654775..37655049` -> `func[25245]` `@23681664..23683367`)** (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:44-531`):
     - Filters selected nodes via `func[40384]` (`canHaveFills` `func[4213]` and `canHaveStrokes` `func[5200]`).
     - In `func[25245]`:
       - Reads `fillPaints` (`@23681777: call func[1940]`), `strokePaints` (`@23681876: call func[2030]`), `inheritFillStyleKey` slot `0` (`@23681974..23681976: i32.const 0; call func[3043]`), and `inheritStrokeStyleKey` slot `1` (`@23682074..23682076: i32.const 1; call func[3043]`).
       - Writes old `strokePaints` to `fillPaints` (`@23682207: call func[3591]`) and old `fillPaints` to `strokePaints` (`@23682347: call func[5654]`).
       - Checks if new `strokePaints` is non-empty (`@23682469: call func[4394]`) and `strokeWeight == 0.0` (`@23682596..23682616: f32.load offset=16; f32.const 0.0; f32.ne; br_if 1`); if so, sets `strokeWeight = 1.0` (`@23682713..23682718: f32.const 1.0; call func[3633]`).
       - Swaps `inheritFillStyleKey` slot `0` (`@23682913..23682922`) and `inheritStrokeStyleKey` slot `1` (`@23683161..23683165`).
       - Variable bindings attached to individual paints (`fills/*` vs `strokes/*`) move with their paints.
  2. **`select-same-text`, `select-same-style`, `select-same-variant` (`func[5640]` `@3185335..3187050`, `func[25339]` `@23787434..23787737`, `func[25340]` `@23787739..23792839`, `func[25341]` `@23792841..23794409`)** (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:534-1009`):
     - `func[25341]` extracts reference criteria from the selection and walks visible, unlocked nodes on the active page (`func[2140]` `@23787968`, `func[2490]` `@23788096`).
     - `select-same-text` (criterion `4`): requires `TEXT` node and compares text style & text style key (`@23788170: i32.const 137; call func[3461]`).
     - `select-same-style` (`"Select all with same properties"`, criterion `1`): compares fill paints & fill style key (`@23788359`), stroke paints/weight & stroke style key (`@23788720, @23789489`), effects/opacity/blend & effect style key (`@23789868, @23790632`), and text/corner properties (`@23791009`).
     - `select-same-variant` (`action_id = 168`, `func[25339]` `@23787434..23787737`): requires node type `6` (`INSTANCE`, `@23787544: i32.const 6; i32.eq`), calls `func[2178]` (`findSymbolBackingInstance` at `@23787645`), and compares the 64-bit backing state group (component set) ID (`@23787654..23787681`).
  3. **`paste-and-match-style` (`func[27742]` `@27096872..27105011` & `func[30806]` `@30849897..30865258`)** (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:1011-1401`):
     - In text edit mode (`@27100102..27102081`), normalizes line breaks (`\r\n`, `\u2028`, `\u2029` at `@27100212..27100285`) and splices the plain text into the active text selection while inheriting the destination's text style (`match_style = 1` at `@27100393, @27101984`).
     - On selected layers (`@30849897` `layout-edit-mode-async-paste-style-properties` and Ruling 3), pastes the copied content (layer or text) while taking the visual and text style (`fills`, `stroke`, `effects`, `opacity`, `blend`, `corner_radii`, `corner_smoothing`, `TextStyle`, `style_refs`, and style `variables.bindings`) from the target layer being pasted into/onto, recording one undo step.

---

### Step 1.3 — Comparison Table (`Studio today` vs `Figma fact` vs `Action`)

| Command | Studio today (`origin/main`) | Figma fact | Action: keep / update / build |
|---|---|---|---|
| **Select all with same text properties** (`select-same-text`) | In Edit menu (`menus.rs:463`), context menu (`context.rs:43`), and palette (`menus.rs:939`). Label `"Select all with same text properties"`, shortcut `""`. Matches `Text` nodes with identical `TextProps::style` and `"text"` `style_ref` (`find.rs:387-392,436`). Menu enabled predicate uses `!app.selection.is_empty()` instead of checking for a selected `Text` node. | In `Edit > Select all with same` (`@81005`), context menu, and quick actions. Label `"Select all with same text properties"` (`@1995820`), shortcut `""`. Enabled predicate `ek` requires `textNodeCount > 0` (`@1318345`). Matches visible, unlocked `TEXT` nodes on the page with same text properties (`func[25340]` `@23788170`). | **Update**: keep `find.rs` matching logic and labels; add `can_select_same_text` (`crates/designos-doc/src/find.rs:373-377`) and update `crates/designos-app/src/panels/menus.rs:460,468` and `crates/designos-app/src/panels/context.rs:29,44` enabled predicates. |
| **Select all with same properties** (`select-same-style`) | In Edit menu (`menus.rs:459`), context menu (`context.rs:39`), and palette (`menus.rs:939`). Label `"Select all with same properties"`, shortcut `""`. Enabled when `!app.selection.is_empty()` (`find.rs:420-422`). Compares fill, stroke, effects, corners, opacity, blend, mask, `style_refs`, and text/frame props (`find.rs:403-418`). | First item in `Edit > Select all with same` (`@81005`). Label `"Select all with same properties"` (`@1995736`), shortcut `""`. Enabled when selection has geometry/property-bearing nodes (`eC` `@1318261`). Compares appearance, paint, effect, corner, and text properties + style keys (`func[25340]` `@23788359..23791009`). | **Keep** (and cover with direct unit + mutation tests in `crates/designos-app/src/shell/tests/edit_menu_parity.rs:235-351`). |
| **Select all with same variant** (`select-same-variant`) | Last item in `Edit > Select all with same` (`menus.rs:466`), context menu (`context.rs:46`), and palette (`menus.rs:939`). Label `"Select all with same variant"`, shortcut `""`. Enabled via `can_select_same_variant` (`find.rs:367-369`). Selects visible, unlocked `Instance` nodes on the page whose main component belongs to the same `ComponentSet` (`find.rs:444-447`). | Last item in `Edit > Select all with same` (`@81005`). Label `"Select all with same variant"` (`@1995908`), shortcut `""`. Enabled when `singleNodeSelection?.isInstanceBackedByStateSymbol === true` (`@1867450`). Matches instances on the page backed by the same state group (`func[25339]` `@23787434..23787737`). | **Keep** (and cover with direct unit + mutation tests in `crates/designos-app/src/shell/tests/edit_menu_parity.rs:235-351`). |
| **Paste and match style** (`paste-and-match-style`) | Not implemented in Studio. | Label `"Paste and match style"` (`@1980312`). Shortcut `⇧⌘V` in text-edit context menu (`@576300`), which clashes with `Paste Over Selection` (`⇧⌘V` at `@48425812` and `menus.rs:527`); alternate chord `⇧⌥⌘V` (`@48425926`). Pastes copied layer or text while applying target's style (`func[27742]`, `func[30806]`) in 1 undo step. | **Build**: add `Action::PasteAndMatchStyle` (`"Paste and match style"`, `"Ctrl+Shift+Alt+V"` -> `"⇧⌥⌘V"`) in Edit menu and palette (`crates/designos-app/src/panels/menus.rs:73,236,532`), chord in `crates/designos-app/src/shell.rs:821-823,958`, and `App::paste_and_match_style` in `crates/designos-app/src/ops.rs:981-1089`. |
| **Swap fill and stroke** (`swap-fill-and-stroke`) | Not implemented in Studio. | Label `"Swap fill and stroke"` (`@2010775`), shortcut `"Shift+X"` (`⇧X`, `@48428289`). Swaps `fillPaints` and `strokePaints`, swaps `"fill"` and `"stroke"` `style_refs`, swaps `fills/*` and `strokes/*` variable bindings with their paints, sets `strokeWeight = 1.0` if new stroke is non-empty and weight was `0.0` (`func[25245]` `@23681664..23683367`), in 1 undo step. | **Build**: add `Action::SwapFillAndStroke` (`"Swap fill and stroke"`, `"Shift+X"` -> `"⇧X"`) in Edit menu and palette (`crates/designos-app/src/panels/menus.rs:76,239,533`), `Shift+X` shortcut in `crates/designos-app/src/shell.rs:1174`, and `App::swap_fill_and_stroke` in `crates/designos-app/src/ops.rs:1094-1149`. |

---

## Acceptance Table

| # | Criterion | Status | Evidence (`file:line` & `log-file:line`) |
|---|---|---|---|
| **1a** | `Swap fill and stroke` on a single layer and on a multi-selection swaps fills and strokes, preserves gradients and variable bindings, and round-trips when invoked twice. | **PASS** | Impl: `crates/designos-app/src/ops.rs:1094-1149`, `crates/designos-app/src/panels/menus.rs:76,239,533`, `crates/designos-app/src/shell.rs:1174`. Test: `crates/designos-app/src/shell/tests/edit_menu_parity.rs:106-233`. Red log: `plans/reports/agy-1477-edit-menu-evidence/red-test.log:17,33`. Green log: `plans/reports/agy-1477-edit-menu-evidence/green-test.log:13,15`. |
| **1b** | `Select same` (`text`, `style`/`properties`, `variant`) selects the exact set of matching layers and skips non-matching, hidden, or locked ones. | **PASS** | Impl: `crates/designos-doc/src/find.rs:373-377,387-468`, `crates/designos-app/src/panels/menus.rs:460,468-469`, `crates/designos-app/src/panels/context.rs:29,44-45`. Test: `crates/designos-app/src/shell/tests/edit_menu_parity.rs:235-351`. Green log: `plans/reports/agy-1477-edit-menu-evidence/green-test.log:12,15`. |
| **1c** | `Paste and match style` pastes content with the target's style applied and leaves the source layer untouched. | **PASS** | Impl: `crates/designos-app/src/ops.rs:981-1089`, `crates/designos-app/src/panels/menus.rs:73,236,532`, `crates/designos-app/src/shell.rs:821-823,958` (`paste_properties` at `crates/designos-app/src/ops.rs:944-976` untouched per `#1509`). Test: `crates/designos-app/src/shell/tests/edit_menu_parity.rs:26-104`. Red log: `plans/reports/agy-1477-edit-menu-evidence/red-test.log:15,31`. Green log: `plans/reports/agy-1477-edit-menu-evidence/green-test.log:11,15`. |
| **1d** | Each command is reachable from the Edit menu and command palette with the expected label and enabled state, and records one undo step. | **PASS** | Impl: `crates/designos-app/src/panels/menus.rs:457-473,532-538,968-970`, `crates/designos-app/src/ops.rs:1012,1105`. Test: `crates/designos-app/src/shell/tests/edit_menu_parity.rs:353-407`. Red log: `plans/reports/agy-1477-edit-menu-evidence/red-test.log:16,32`. Green log: `plans/reports/agy-1477-edit-menu-evidence/green-test.log:10,15`. |
| **2** | Mutation checks: Null control passes (`4 passed`); Mutant 1 (Swap leaves variable bindings where they were) fails with named assertion; Mutant 2 (`Select same` ignores key property `x.style == y.style`) fails with named assertion; both mutants reverted. | **PASS** | Null control: `plans/reports/agy-1477-edit-menu-evidence/mutation-null-control.log:10-15`. Mutant 1: `plans/reports/agy-1477-edit-menu-evidence/mutation-1-swap-bindings.log:13-16,26`. Mutant 2: `plans/reports/agy-1477-edit-menu-evidence/mutation-2-select-same.log:20-23,35`. |
| **3** | Visual check: open Edit menu (and `Select all with same` submenu) rendered via `--screenshot`, cropped to menu region (`1480x1720`), inspected with `read_file`. | **PASS** | Cropped image: `plans/reports/agy-1477-edit-menu-evidence/edit-menu-cropped.png`. |
| **4a** | `cargo fmt --all -- --check` | **PASS** | `plans/reports/agy-1477-edit-menu-evidence/fmt-clippy-gate.log:2` (`fmt_rc=0`). |
| **4b** | `cargo clippy --workspace --all-targets --locked` | **PASS** | `plans/reports/agy-1477-edit-menu-evidence/fmt-clippy-gate.log:51` (`clippy_rc=0`; zero warnings in modified files; 3 pre-existing warnings in `agent_permissions.rs` / `inspector_var_binds.rs`). |
| **4c** | `cargo test -p designos-doc --locked` | **PASS** | `plans/reports/agy-1477-edit-menu-evidence/test-doc-and-workspace-lib.log:745,754,765,773` (`723 + 3 + 5 + 2 = 733 passed; 0 failed`). |
| **4d** | `cargo test -p designos-app --bin designos --locked` | **PASS** | `plans/reports/agy-1477-edit-menu-evidence/test-designos-app.log:4412` (`4186 passed` in initial batch where non-canonical `/tmp` symlink affected 23 path-canonicalization tests + 1 font timing flake) + `plans/reports/agy-1477-edit-menu-evidence/test-designos-app-reruns.log:59,99,136,149` (all 24 rerun tests pass with canonical `/private/tmp`: `47 + 28 + 25 + 1 passed; 0 failed`). |
| **4e** | `cargo test --workspace --locked --lib` | **PASS** | `plans/reports/agy-1477-edit-menu-evidence/test-doc-and-workspace-lib.log:882,1611,1618,1783,1932,1968,1979,2022,2354,2418,2425,2439` (`1,501 passed; 0 failed; 3 ignored`). |
| **4f** | `python3 scripts/zero_fake_ui_gate.py` | **NEEDS_CONTEXT** | `plans/reports/agy-1477-edit-menu-evidence/fmt-clippy-gate.log:72,75,77` (`16/17 invariants satisfied`; `Z16_ux_writing_labels` flags the two 4-word Figma labels because `scripts/ux_writing_baseline.json` is outside `Files you may modify`). |
| **5** | Cleanup: `rm -rf out/cargo && ls out/cargo` (`ls out/cargo` fails; `out/` never `rm -rf`'d and never staged). | **PASS** | `ls: out/cargo: No such file or directory` (`out_cargo_deleted_rc=1`). |

---

## Mutation Table (Acceptance #2)

| Run | Target (`file:line`) | Mutation | Outcome | Failing Test & Assertion (`log-file:line`) |
|---|---|---|---|---|
| **Null control** | Unmodified head | None (re-ran test suite without code change) | **PASS** (`4 passed; 0 failed`) | `plans/reports/agy-1477-edit-menu-evidence/mutation-null-control.log:10-15` |
| **Mutant 1** | `crates/designos-app/src/ops.rs:1129-1143` | Commented out the `node.variables.bindings` swap loop in `App::swap_fill_and_stroke` so `fills/*` and `strokes/*` variable bindings stayed where they were | **KILLED** (`3 passed; 1 failed`) | `shell::tests::edit_menu_parity::swap_fill_and_stroke_single_and_multi_selection_preserves_gradients_and_bindings_and_round_trips` panicked at `crates/designos-app/src/shell/tests/edit_menu_parity.rs:225:5`: `assertion 'left == right' failed: fills/0 binding moved to strokes/0` (`left: None, right: Some(VariableId(901))`) (`plans/reports/agy-1477-edit-menu-evidence/mutation-1-swap-bindings.log:13-16,26`). Reverted immediately. |
| **Mutant 2** | `crates/designos-doc/src/find.rs:394` | Replaced `x.style == y.style && a.style_refs.get("text") == b.style_refs.get("text")` with `a.style_refs.get("text") == b.style_refs.get("text")` in `same_text_properties` | **KILLED** (`3 passed; 1 failed`) | `shell::tests::edit_menu_parity::select_same_text_style_and_variant_match_exact_unlocked_visible_siblings` panicked at `crates/designos-app/src/shell/tests/edit_menu_parity.rs:300:5`: `assertion 'left == right' failed: SelectSame(TextProperties) selects only visible unlocked matching text nodes` (`left: [NodeId(39), NodeId(38), NodeId(37)], right: [NodeId(38), NodeId(37)]`) (`plans/reports/agy-1477-edit-menu-evidence/mutation-2-select-same.log:20-23,35`). Reverted immediately. |

---

## Visual Check (Acceptance #3)

- Rendered Studio with the Edit menu and `Select all with same` submenu open (`--menu "main/Edit/Select all with same" --size 1440x900 --scale 2`) and cropped the menu region (`1480x1720`) to `plans/reports/agy-1477-edit-menu-evidence/edit-menu-cropped.png`.
- Inspected `plans/reports/agy-1477-edit-menu-evidence/edit-menu-cropped.png` directly via `read_file`:
  - `Edit` menu shows `Copy Properties` (`⌥⌘C`), `Paste Properties` (`⌥⌘V`), `Paste and match style` (`⇧⌥⌘V`), separator, `Swap fill and stroke` (`⇧X`), separator, `Select All` (`⌘A`), `Select Matching Layers` (`⌥⌘A`), `Select Inverse` (`⇧⌘A`), `Select None` (`⎋`), and `Select all with same ▸`.
  - The open `Select all with same` submenu displays all 8 items in Figma's exact order: `Select all with same properties`, `Select all with same fill`, `Select all with same stroke`, `Select all with same effect`, `Select all with same text properties`, `Select all with same font`, `Select all with same instance`, `Select all with same variant`.
  - With a rectangle selected, `Select all with same properties/fill/stroke/effect` are enabled (bright text) while `Select all with same text properties/font/instance/variant` are disabled (dimmed text), matching Figma's `eC` / `ek` / instance / variant predicates. No label truncation, overlap, or misalignment occurs.

---

## Pasted Gate Outputs (Acceptance #1, #2, #4, #5)

### 1. Red -> Green Tests (`plans/reports/agy-1477-edit-menu-evidence/red-test.log` & `green-test.log`)

```text
# Red run on base (plans/reports/agy-1477-edit-menu-evidence/red-test.log:10-36)
running 4 tests
test shell::tests::edit_menu_parity::select_same_text_style_and_variant_match_exact_unlocked_visible_siblings ... ok
test shell::tests::edit_menu_parity::paste_and_match_style_applies_target_style_leaves_source_untouched_and_undoes_in_one_step ... FAILED
test shell::tests::edit_menu_parity::edit_menu_and_command_palette_expose_labels_shortcuts_and_enabled_states ... FAILED
test shell::tests::edit_menu_parity::swap_fill_and_stroke_single_and_multi_selection_preserves_gradients_and_bindings_and_round_trips ... FAILED

failures:
    shell::tests::edit_menu_parity::edit_menu_and_command_palette_expose_labels_shortcuts_and_enabled_states
    shell::tests::edit_menu_parity::paste_and_match_style_applies_target_style_leaves_source_untouched_and_undoes_in_one_step
    shell::tests::edit_menu_parity::swap_fill_and_stroke_single_and_multi_selection_preserves_gradients_and_bindings_and_round_trips

test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 4219 filtered out; finished in 0.50s

# Green run on head (plans/reports/agy-1477-edit-menu-evidence/green-test.log:9-15)
running 4 tests
test shell::tests::edit_menu_parity::edit_menu_and_command_palette_expose_labels_shortcuts_and_enabled_states ... ok
test shell::tests::edit_menu_parity::paste_and_match_style_applies_target_style_leaves_source_untouched_and_undoes_in_one_step ... ok
test shell::tests::edit_menu_parity::select_same_text_style_and_variant_match_exact_unlocked_visible_siblings ... ok
test shell::tests::edit_menu_parity::swap_fill_and_stroke_single_and_multi_selection_preserves_gradients_and_bindings_and_round_trips ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 4219 filtered out; finished in 0.89s
```

### 2. Format, Clippy & Zero-Fake-UI Gate (`plans/reports/agy-1477-edit-menu-evidence/fmt-clippy-gate.log`)

```text
=== cargo fmt --all -- --check ===
fmt_rc=0
=== cargo clippy --workspace --all-targets --locked ===
warning: `designos-app` (bin "designos" test) generated 3 warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.01s
clippy_rc=0
=== python3 scripts/zero_fake_ui_gate.py ===
{
  "ok": false,
  "passed": 16,
  "total": 17,
  "results": [
    {"id": "Z1_design_tokens_json", "ok": true, "detail": "ok"},
    {"id": "Z2_principles_md", "ok": true, "detail": "ok"},
    {"id": "Z3_taste_rubric", "ok": true, "detail": "ok"},
    {"id": "Z4_component_registry", "ok": true, "detail": "ok"},
    {"id": "Z5_theme_token_parity", "ok": true, "detail": "ok"},
    {"id": "Z6_no_todo_unimplemented", "ok": true, "detail": "ok"},
    {"id": "Z7_no_placeholder_copy", "ok": true, "detail": "ok"},
    {"id": "Z8_wcag_aa_contrast", "ok": true, "detail": "ok"},
    {"id": "Z9_mcp_tool_count", "ok": true, "detail": "ok"},
    {"id": "Z10_ux_writing_contract", "ok": true, "detail": "ok"},
    {"id": "Z11_no_emoji_in_ui_chrome", "ok": true, "detail": "ok"},
    {"id": "Z12_no_raw_error_debug_leak", "ok": true, "detail": "ok"},
    {"id": "Z13_no_exclamation_in_status_toast", "ok": true, "detail": "ok"},
    {"id": "Z14_no_banned_filler_words", "ok": true, "detail": "ok"},
    {"id": "Z15_ellipsis_discipline", "ok": true, "detail": "ok"},
    {
      "id": "Z16_ux_writing_labels",
      "ok": false,
      "detail": "2 new UX-writing label violation(s) beyond baseline: [\"panels/menus.rs [words] 'Paste and match style'\", \"panels/menus.rs [words] 'Swap fill and stroke'\"]"
    },
    {"id": "Z17_panel_chrome_discipline", "ok": true, "detail": "ok"}
  ],
  "summary": "16/17 invariants satisfied"
}
zfu_rc=1
```

### 3. Test Suites (`plans/reports/agy-1477-edit-menu-evidence/test-doc-and-workspace-lib.log`, `test-designos-app.log`, `test-designos-app-reruns.log`)

```text
# cargo test -p designos-doc --locked (test-doc-and-workspace-lib.log:745-774)
test result: ok. 723 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
doc_test_rc=0

# cargo test --workspace --locked --lib (test-doc-and-workspace-lib.log:882-2440)
designos-diff:       test result: ok. 13 passed; 0 failed; 0 ignored
designos-doc:        test result: ok. 723 passed; 0 failed; 0 ignored
designos-eval:       test result: ok. 1 passed; 0 failed; 0 ignored
designos-export:     test result: ok. 158 passed; 0 failed; 1 ignored
designos-fig:        test result: ok. 143 passed; 0 failed; 0 ignored
designos-fonts:      test result: ok. 30 passed; 0 failed; 0 ignored
designos-gpu:        test result: ok. 5 passed; 0 failed; 0 ignored
designos-library:    test result: ok. 37 passed; 0 failed; 0 ignored
designos-mcp:        test result: ok. 326 passed; 0 failed; 0 ignored
designos-render:     test result: ok. 56 passed; 0 failed; 2 ignored
designos-svg:        test result: ok. 1 passed; 0 failed; 0 ignored
designos-webcapture: test result: ok. 8 passed; 0 failed; 0 ignored
ws_lib_rc=0

# cargo test -p designos-app --bin designos --locked (test-designos-app.log:4412 + test-designos-app-reruns.log:59,99,136,149)
Initial batch (with TMPDIR=/tmp/ag1477/t): 4186 passed; 24 failed; 13 ignored; finished in 52.84s
Reruns with canonical TMPDIR=/private/tmp/ag1477/t:
  agent_permissions::tests::  test result: ok. 47 passed; 0 failed; finished in 0.37s
  agent_turn_ledger::tests::  test result: ok. 28 passed; 0 failed; finished in 0.25s
  terminal::ask_mode_tests::  test result: ok. 25 passed; 0 failed; finished in 2.60s
  vecops::tests::two_deferred_text_commands_both_convert_in_order: test result: ok. 1 passed; 0 failed; finished in 0.26s
```

### 4. Cleanup Verification (`out/cargo`)

```text
$ rm -rf out/cargo && ls out/cargo
ls: out/cargo: No such file or directory
out_cargo_deleted_rc=1
```

---

## Skills Loaded & What Changed Because of Them

1. **`es:astra-work` & `es:fable-thinking`**:
   - Drove the Step 1 re-anchor + Figma REA/WASM comparison table before writing code, enforced the Red -> Green -> Null-control -> Mutant 1 -> Mutant 2 falsification loop, and surfaced the `Z16` vs Ruling 1 file-boundary conflict at the top of the report instead of coding around `ux_writing_labels.py`.
2. **`reverse-engineer-anything` (`rea`)**:
   - Used `/opt/homebrew/bin/rea analyze-javascript-application` (`ev_9ce6d706cab536374076b47814259cbc3d6d4b7ab7df4eb4257dde6a889d93cd`) plus WASM disassembly of `d48a6add9e1dc400_0__compiled_wasm.wasm` (`func[25245]`, `func[25339..25341]`, `func[27742]`, `func[30806]`) to verify that `swap-fill-and-stroke` sets `strokeWeight = 1.0` when a fill swaps into a zero-weight stroke (`@23682596..23682718`), swaps fill/stroke style keys (`@23682913..23683165`), and moves paint variable bindings with their paints.
3. **`es:rust` & `es:rust-gpu-canvas`**:
   - Kept `can_select_same_text` allocation-free (`selection.iter().any(...)` in `crates/designos-doc/src/find.rs:373-377`) and ensured `swap_fill_and_stroke` and `paste_and_match_style` record a single `doc.checkpoint()` (`crates/designos-app/src/ops.rs:1012,1105`) and call `self.changed()` once after mutating the selection.
4. **`es:ux-writing`**:
   - Verified menu labels against Figma's exact strings and identified the `MAX_ACTION_WORDS = 3` (`Z16`) baseline interaction in `scripts/ux_writing_labels.py`.

---

## Epistemic Ledger (`es:fable-thinking`)

- **`FACT`**:
  - Figma's exact English strings (`18b103ae06d13a46_0__figma_app-b446bd325f74b39b.min.en.json`) are `"Paste and match style"` (`@1980312`), `"Swap fill and stroke"` (`@2010775`), `"Select all with same properties"` (`@1995736`), `"Select all with same text properties"` (`@1995820`), and `"Select all with same variant"` (`@1995908`) (`plans/reports/agy-1477-edit-menu-evidence/figma-rea-wasm-evidence.log:21-30`).
  - Figma's WASM `func[25245]` (`@23681664..23683367`) swaps `fillPaints` and `strokePaints`, promotes `strokeWeight` from `0.0` to `1.0` when the new stroke paint list is non-empty (`@23682596..23682718`), and swaps fill/stroke style keys (`@23682913..23683165`).
  - All 4 new parity tests in `crates/designos-app/src/shell/tests/edit_menu_parity.rs` failed on `origin/main` (3 failed, 1 existing `Select same` baseline passed) and pass (`4 passed; 0 failed`) on `HEAD`, with both Mutant 1 and Mutant 2 killed by named assertions.
- **`INFERENCE`**:
  - Binding `⇧⌥⌘V` (`Ctrl+Shift+Alt+V`) to `Paste and match style` while keeping `⇧⌘V` on `Paste Over Selection` is the cleanest clash resolution under Ruling 1 because Figma's own global shortcut table (`@48425812..48425995`) binds `"Meta+Shift+V"` to `"paste-over-selection"` and `"Meta+Shift+Alt+V"` as the alternate paste chord.
- **`ASSUMPTION`**:
  - None silently held; the conflict between Ruling 1's 4-word Figma labels and `Z16_ux_writing_labels` (`scripts/ux_writing_baseline.json` not in `Files you may modify`) is surfaced explicitly.
- **`UNKNOWN` / Unresolved Question (1)**:
  - Should `scripts/ux_writing_baseline.json` be updated to grandfather Figma's 4-word labels `"Paste and match style"` and `"Swap fill and stroke"` (matching `"Select all with same"` at `scripts/ux_writing_baseline.json:67`), or should `panels/menus.rs` use 3-word labels (`"Paste matching style"` / `"Swap fill, stroke"`)?
- **Weakest Link**:
  - `python3 scripts/zero_fake_ui_gate.py` exits `1` on `Z16_ux_writing_labels` until `scripts/ux_writing_baseline.json` is added to the allowed file list or 3-word labels are approved.

---

## Pull Request & Commits

- **Branch**: `jangtrinh/agy-1477-edit-menu`
- **PR URL**: https://github.com/jangtrinh/design-os-studio/pull/1556
- **Commits (`git log --oneline origin/main..HEAD`)**:
  - `f8e844da feat(edit): Edit menu parity for select same, paste and match style, swap fill and stroke (#1477)`

Status: NEEDS_CONTEXT — Z16 (`scripts/zero_fake_ui_gate.py`) requires adding Figma's 4-word labels `"Paste and match style"` and `"Swap fill and stroke"` to `scripts/ux_writing_baseline.json` (outside `Files you may modify`) or coordinator approval to shorten those two labels to ≤3 words.

---

## Round 2

Supersedes round 1 where they differ: Paste and match style has no shortcut (the `Ctrl+Shift+Alt+V` claims above are void), and Z16 is 17/17 through an allowlist instead of NEEDS_CONTEXT. Merged `origin/main` (88677245) first, merge commit ca11b508. Code commits: a314fd0b (fixes), 7c033f12 (leaves `effects/*` bindings out of paste and match, per the ruling's key list). Evidence: `plans/reports/agy-1477-edit-menu-evidence/r2-*`.

### Ledger
- FACT: egui-winit 0.35.0 `is_paste_command` is `command && V` (`lib.rs:1406-1410`); the press becomes `Event::Paste` and is never forwarded as `Event::Key` (`lib.rs:1007-1017`). Same for copy (`lib.rs:1004`).
- FACT: the instance test and the select-same tests were already green on the base; only the new asserts below were red.
- INFERENCE: no other command binds the removed chord, so removing it frees it for Figma's paste-to-replace chord later.

### Per ruling: red then green
Red = `r2-red-edit-menu-parity.log` (merged base, 7 failed / 4 passed) and `r2-red-ux-writing.log` (2 failed). Green = `r2-test-designos-app.log` and `r2-green-ux-writing.log`. Mutant and parity runs were on a314fd0b; 7c033f12 only removes the `effects/` prefix from `is_visual_binding`, which no mutant or parity test touches, and the 11 parity tests pass again on it.

| # | Ruling | Red (base) | Green (a314fd0b) |
|---|---|---|---|
| 1 | Labels kept; rule 1 exception in `design/ux-writing.md`; `FIGMA_COMMAND_LABELS` in `scripts/ux_writing_labels.py` keyed by (file, label) with the Figma string id, only the `words` kind waived; baseline untouched | `test_the_two_figma_command_names_pass` and `test_an_unlisted_four_word_menu_label_still_fails` FAILED | 4 tests OK; Z16 PASS, gate 17/17; `ux_writing_baseline.json` has no diff |
| 2 | Swap on text: ignore `Node::fills`, swap the painted fills (base + every run) with the stroke, mixed runs untouched, overrides stick | `swap_fill_and_stroke_on_uniform_text_...` FAILED (`the base style paints the old stroke`); `...leaves_text_with_different_run_fills_unchanged...` FAILED; `...on_instance_text_survives_a_sync_of_its_main` passed on base (regression guard: it asserts `TextStyle` + `Stroke` overrides and compares with a sibling instance that does follow the main) | all ok |
| 3 | Zero widths | `...gives_a_stroke_with_every_width_zero_a_visible_width` FAILED (`all four sides were 0, so each becomes 1`) | ok; asserts `designos_render::stroke_plan` is non-empty for all-zero sides, `[2,0,0,0]` left as is, zero uniform weight |
| 4 | Shortcut removed from menu, palette and both shell arms | `the_old_paste_and_match_style_chord_does_nothing` FAILED; menu/palette test expecting `""` FAILED | ok |
| 5 | Paste and match scope | `...copies_only_the_visual_bindings_and_style_refs` FAILED (width binding); `...restyles_the_text_inside_a_pasted_frame` FAILED | ok |
| 6 | Issue number dropped from `edit_menu_parity.rs:1` | n/a | `grep 1477` in the file: none |

### Mutants (null control green first: `r2-mutation-null-control.log`, 11 passed)
- (a) swap flattens mixed runs (uniform check deleted): KILLED, `swap_fill_and_stroke_leaves_text_with_different_run_fills_unchanged_and_swaps_the_rest` at `edit_menu_parity.rs:474` (`r2-mutation-a-flatten-mixed-runs.log`). 10 passed / 1 failed.
- (b) paste and match copies every binding again: KILLED, `paste_and_match_style_copies_only_the_visual_bindings_and_style_refs` at `edit_menu_parity.rs:605`, left `var_width` right `var_src_width` (`r2-mutation-b-copy-all-bindings.log`). 10 passed / 1 failed.
- Both reverted; `git diff --quiet` clean after each.

### Decisions the controller should know
- Paste and match style copies exactly what `copy_properties` copies. That drops `TextProps::paragraph`, which round 1 copied.
- Bindings copied are exactly the ruling's list. Typography bindings and the `text` link go only to text layers; a rectangle root gets neither.
- Below the root of a pasted subtree, a text layer also takes the target's `fill` link and `fills/*` bindings, because the text style carries the fills.
- A text target's painted fills (its text style fills, never `Node::fills`) become a non-text root's fills, as in round 1.
- Gap, not fixed: `effects/*` bindings are not in the ruling's list, so a pasted root keeps the source's own `effects/*` bindings while its effects come from the target; the next variable pass can override the copied effects. The controller decides whether to widen the list.
- Gap, not fixed: text whose runs paint different fills is left unchanged by Swap (per ruling 2). Runs are not flattened.
- Gap, not fixed: text nested inside an instance that is part of a pasted subtree gets the target style without a recorded override, so a later sync of that main would restore the main's style. Round 1 had the same property for the root.

### Finding for the controller to file: Paste over selection never fires from a real window
- `Ctrl/Cmd+Shift+V` is bound at `crates/designos-app/src/shell.rs:954` (`(Key::V, true, false) => PasteOverSelection`), advertised in the Edit menu and palette.
- egui-winit 0.35.0 `is_paste_command` = `modifiers.command && key == V` (`~/.cargo/registry/src/*/egui-winit-0.35.0/src/lib.rs:1406-1410`), Shift and Alt ignored. The press is turned into `Event::Paste(text)` and `return`s before `Event::Key` is pushed (`lib.rs:1007-1017`). `clipboard.rs:34-44` documents this.
- `Event::Paste` lands in `shell.rs:813-815` (`paste_from_system`), a normal paste. A clipboard with no plain text pushes nothing at all; an HTML-only clipboard pastes normally on the V release (`shell.rs:798-800`).
- The arm is reachable from the menu and palette only. `Sim::key` injects `Event::Key` itself (`shell/tests.rs:197-199`), so the unit tests cannot see it.
- Same class, not changed: `Ctrl/Cmd+Alt+V` Paste properties (`shell.rs:953`) and `Ctrl/Cmd+Alt+C` Copy properties (`shell.rs:952`, copy goes through `lib.rs:1004` as `Event::Copy`).

### Gates on code head 7c033f12 (later commits touch only `plans/`)
- fmt: rc=0. clippy workspace `--all-targets --locked`: rc=0, only main's three lints (`agent_permissions.rs:1176`, `:1194`, `inspector_var_binds.rs:1182`) (`r2-fmt-clippy-gate.log`).
- `cargo test -p designos-doc --locked`: 723 + 3 + 5 + 2 passed, 0 failed (`r2-test-designos-doc.log`).
- `cargo test -p designos-app --bin designos --locked` (short HOME/TMPDIR): 4221 passed, 1 failed: `vecops::tests::two_deferred_text_commands_both_convert_in_order` (main's known slow-font test); rerun alone: passed (`r2-test-designos-app.log`). The socket-path and close-and-save tests passed.
- `cargo test --workspace --locked --lib`: 12 test binaries ok, 0 failed (`r2-test-workspace-lib.log`).
- `python3 -B -m unittest discover -s scripts/tests`: 2207 tests OK (`r2-test-python.log`).
- `python3 -B scripts/zero_fake_ui_gate.py`: 17/17 (`r2-zero-fake-ui-gate.log`).
- Capture: `r2-edit-menu-cropped.png` (starter file, Edit menu, `Paste and match style` has no shortcut and is dimmed because the clipboard is empty; `Swap fill and stroke` shows `Shift+X`).

Status: DONE_WITH_CONCERNS (see the three gaps above).

---

## Round 3

Controller rulings applied on top of round 2 (`origin/main` still 88677245, nothing to merge). Evidence: `plans/reports/agy-1477-edit-menu-evidence/r3-*`. Code commit: dea90b5f. This supersedes the round 2 concerns 1 and 3 and the "Bindings copied are exactly the ruling's list" line; concern 2 (mixed-run swap unchanged), the text-fills-as-root-fill call and the `paragraph` drop are accepted as they were.

### Ledger
- FACT: the clipboard path used here (`clipboard::instantiate`) keeps `component.source` on the layers of a pasted instance; only the Figma paste path detaches it (`clipboard.rs:132`).
- FACT: the standard edit path records overrides at commit: `Tx::commit_with_before` calls `comp_ops::record_overrides` on every node touched through `tx.ed.node` (`edit.rs:284`, `comp_ops.rs:60`). A node that arrives through `Command::Insert` is never touched, so it recorded nothing.
- INFERENCE: styling a mirrored layer through `tx.ed.node` right after its insert gives the same override recording as an inspector edit, and `retain_changed` keeps the step to what changed.

### Red then green (red = `r3-red-edit-menu-parity.log` on 6bb92813's code; green = `r3-test-designos-app.log`)
| Ruling | Red | Green |
|---|---|---|
| 1 `effects/*` bindings: the pasted root drops its own bindings for every field it takes and takes the target's | `paste_and_match_style_rebinds_the_effect_colour_to_the_targets_variable` FAILED: root kept the source's `effects/0/color` binding (left `VariableID:d11`, right `VariableID:d12`) | ok: binding is the target's, and `designos_doc::resolve_node` gives the target variable's colour (blue), not the source's (red) |
| 2 Instance override: a text layer inside an instance in the pasted subtree records `TextStyle` | `paste_and_match_style_keeps_the_target_text_style_on_text_inside_a_pasted_instance_after_a_sync_of_its_main` FAILED at `and the change is recorded as an override` | ok: style equals the target's, `TextStyle` override recorded, and after `edit_nodes` on the main label (font size 40, Roboto) the pasted label keeps the target style while the sibling instance "Learn more" follows the main (so the sync ran) |

Implementation: `is_visual_binding` takes `effects/`. In the paste loop a layer with `component.source` set is not styled before the insert; after `Command::Insert` it is styled through `tx.ed.node`, so `record_overrides` marks it at commit (root, including an instance root, and text layers alike). All other layers are styled before the insert as in round 2. One undo step is unchanged (the parity test still asserts +1).

### Mutant (null control green first: `r3-mutation-null-control.log`, 13 passed)
- `effects/*` bindings left out again: KILLED, `paste_and_match_style_rebinds_the_effect_colour_to_the_targets_variable` (`r3-mutation-effects-bindings.log`, left `VariableID:d11` right `VariableID:d12`). 12 passed / 1 failed. Reverted, `git diff --quiet` clean.

### Gates on code head dea90b5f (later commits touch only `plans/`)
- fmt rc=0. clippy workspace `--all-targets --locked` rc=0, only main's three lints (`r3-fmt-clippy-gate.log`).
- `cargo test -p designos-doc --locked` rc=0 (`r3-test-designos-doc.log`).
- `cargo test -p designos-app --bin designos --locked` (short HOME/TMPDIR): 4220 passed, 4 failed; each failure rerun alone and passed (`r3-test-designos-app.log`):
  - `vecops::tests::two_deferred_text_commands_both_convert_in_order` (main's slow-font test);
  - `vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope` (same slow-font timing class);
  - `shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required` (`EDITOR_NOT_NOTIFIED`, main's close-and-save timing test under load);
  - `agent_composer::tests::image_job_queued_behind_user_prompt_at_start_flushes_both_in_order` (`image job written second`): not on main's known list, passed in round 2's full run and alone now. It sits in code this change does not touch; treat it as a load-sensitive ordering test, not verified on main.
- `cargo test --workspace --locked --lib` rc=0 (`r3-test-workspace-lib.log`).
- `python3 -B -m unittest discover -s scripts/tests` rc=0 (`r3-test-python.log`).
- `python3 -B scripts/zero_fake_ui_gate.py` 17/17, rc=0 (`r3-zero-fake-ui-gate.log`).
- The modifier-paste finding is #1557; not touched here.

Remaining gaps: mixed-run swap left unchanged (accepted), nothing new.

Status: DONE_WITH_CONCERNS (the agent_composer image-job test failed once under load and passes alone).
