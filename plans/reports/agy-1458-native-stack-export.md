# Native Stack & Frame Export Report (`#1454`, `#1458`)

> **Correction (2026-11-04).** This report records the first round of #1563. The second round, which is what merged, changed one rule: a child hidden by a variable is **not** moved into the overlay. It stays in the stack flow exactly as before, and only children with `layout_child.absolute` are overlaid. Rows 1(c) and 1(d) below, and the test names `absolute_and_variable_hidden_children_in_stacks_are_emitted_in_overlay_out_of_flow` and `..._in_grid_...`, describe the first round. The merged tests are `variable_hidden_child_stays_in_the_stack_flow`, `grid_overlays_its_absolute_child_and_leaves_a_variable_hidden_child_in_place` and `variable_hidden_grid_child_exports_where_it_was_without_an_overlay` (`crates/designos-export/tests/native_stack_export.rs`). The merged code also re-recorded two Dev Mode layout baselines (`g-dev-annotate`, `g-dev-compare`): +54 px, the three extra SwiftUI lines of the overlay.

## What Could Not Be Verified

1. **Native Mach-O decompilation of `/Applications/Figma.app/Contents/MacOS/Figma`**: Running `rea inspect` (`plans/reports/agy-1458-native-stack-export-evidence/rea-inspect-figma.json:1`) and `rea execute-rizin-command` (`plans/reports/agy-1458-native-stack-export-evidence/rea-rizin-figma.json:1`) against the Electron wrapper binary `/Applications/Figma.app/Contents/MacOS/Figma` returned `provider_unavailable` (`plans/reports/agy-1458-native-stack-export-evidence/rea-inspect-figma.json:1`) and `capability_unavailable` (`plans/reports/agy-1458-native-stack-export-evidence/rea-rizin-figma.json:1`) because Ghidra, Hopper, IDA, and Rizin are not installed on this workstation. In addition, `/Applications/Figma.app/Contents/Resources/app.asar` is only the desktop shell and returned `Malformed ASAR during inventory` (`plans/reports/agy-1458-native-stack-export-evidence/rea-figma-asar.json:1`). Instead, Figma's actual Dev Mode codegen bundle (`6413-8e031fdc4fad2bf8.min.js.br`, webpack module `117343`) was extracted from the local Chromium Simple Cache entry `~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data/a6f2bcc873943b57_0` (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:1-3`) and analyzed with `rea analyze-javascript-application`, producing REA evidence `ev_dbf5a79550ba6e1ad5d143246aa53f051bdf24a9b7733dc628804a790c8cb345` (`plans/reports/agy-1458-native-stack-export-evidence/rea-figma-codegen-117343-summary.json:1`).
2. **Multi-node tree wrapper in Figma's built-in Dev Mode SwiftUI and Compose codegen**: Figma's built-in first-party iOS (`nu` / `na`) and Android (`eh` / `ea`) codegen plugins in webpack module `117343` (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:3,15-19,45-49`) are single-node property inspectors: for auto-layout containers they emit placeholder bodies (`HStack(...) { ... }` / `VStack(...) { ... }` at `plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:45-49`, and `Row(...) { // Child views. }` / `Column(...) { // Child views. }` at `plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:17-19`) rather than recursively walking child subtrees. Consequently, how a multi-node exporter wraps an auto-layout stack or grid containing out-of-flow children in `ZStack(alignment: .topLeading)` (SwiftUI) or `Box` (Compose) cannot be read from a multi-node tree walker in `117343` (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:3`) and is instead governed by Ruling 2 and the existing `compose.rs` grid wrapper (`crates/designos-export/src/compose.rs:414-428,510-516`).
3. **Standalone `kotlinc` typecheck on Compose output**: Kotlin/Compose compiler jars are not bundled in the repository test harness, so Compose output is verified structurally in `crates/designos-export/tests/native_stack_export.rs` (`crates/designos-export/tests/native_stack_export.rs:202-322`), while SwiftUI output is verified both structurally and with `/usr/bin/swiftc -typecheck` (`crates/designos-export/tests/native_stack_export.rs:514-552`).

---

## Step 1 — Facts Recorded Before Editing Code

### 1.1 Re-anchor (`origin/main` at `481b0e7ea76f3f7feca001aafe168be4941e49a9`)

1. **`frame_args` in `crates/designos-export/src/swiftui.rs` (`crates/designos-export/src/swiftui.rs:210-244`)**:
   - Signature: `fn frame_args(w: Dim, h: Dim, align: Option<&str>, n: &Node, doc: &Document) -> Option<String>` (`crates/designos-export/src/swiftui.rs:210`).
   - Width string `expr` is resolved via `dimension(doc, n, "width", px)` (`crates/designos-export/src/swiftui.rs:215`), and height string `expr` is resolved via `dimension(doc, n, "height", px)` (`crates/designos-export/src/swiftui.rs:227`).
   - On base `481b0e7e` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:10-28,85-166`), the `9` `(w, h)` pairs of `(Dim::Fixed(W), Dim::Fill, Dim::Hug)` with optional `align = Some(A)` produced:
     1. `(Dim::Hug, Dim::Hug)` -> `None` (`crates/designos-export/src/swiftui.rs:223,235,237-239`).
     2. `(Dim::Fixed(W), Dim::Hug)` -> `Some(".frame(width: {expr}[, alignment: {A}])")` (`crates/designos-export/src/swiftui.rs:214-220,235,243`).
     3. `(Dim::Hug, Dim::Fixed(H))` -> `Some(".frame(height: {expr}[, alignment: {A}])")` (`crates/designos-export/src/swiftui.rs:223,226-232,243`).
     4. `(Dim::Fixed(W), Dim::Fixed(H))` -> `Some(".frame(width: {expr}, height: {expr}[, alignment: {A}])")` (`crates/designos-export/src/swiftui.rs:214-220,226-232,243`).
     5. `(Dim::Fill, Dim::Hug)` -> `Some(".frame(maxWidth: .infinity[, alignment: {A}])")` (`crates/designos-export/src/swiftui.rs:222,235,243`).
     6. `(Dim::Hug, Dim::Fill)` -> `Some(".frame(maxHeight: .infinity[, alignment: {A}])")` (`crates/designos-export/src/swiftui.rs:223,234,243`).
     7. `(Dim::Fill, Dim::Fill)` -> `Some(".frame(maxWidth: .infinity, maxHeight: .infinity[, alignment: {A}])")` (`crates/designos-export/src/swiftui.rs:222,234,243`).
     8. `(Dim::Fill, Dim::Fixed(H))` -> `Some(".frame(maxWidth: .infinity, height: {expr}[, alignment: {A}])")` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:16,89`) — **invalid SwiftUI overload** (`View.frame` has `frame(width:height:alignment:)` and `frame(minWidth:idealWidth:maxWidth:minHeight:idealHeight:maxHeight:alignment:)`, never mixing `maxWidth` and `height`).
     9. `(Dim::Fixed(W), Dim::Fill)` -> `Some(".frame(width: {expr}, maxHeight: .infinity[, alignment: {A}])")` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:21,161`) — **invalid SwiftUI overload** (mixes `width` and `maxHeight`).
   - Call sites of `frame_args` in `crates/designos-export/src/swiftui.rs`:
     - `NodeKind::Instance` (`crates/designos-export/src/swiftui.rs:431`)
     - `text_of(n)` (`crates/designos-export/src/swiftui.rs:441`)
     - Grid container (`crates/designos-export/src/swiftui.rs:456`)
     - `HStack` / `VStack` container (`crates/designos-export/src/swiftui.rs:499`)
     - `ZStack` container (`crates/designos-export/src/swiftui.rs:506`)
     - Leaf shape (`crates/designos-export/src/swiftui.rs:530`)

2. **How `designos-doc` and `designos-export` mark an absolute / out-of-flow child (`crates/designos-doc/src/layout_apply.rs:86-91`, `crates/designos-export/src/codegen.rs:451,500-505,680-685`)**:
   - Each node stores `n.layout_child.absolute: bool` (`crates/designos-doc/src/node.rs:1104`).
   - `Document::flow_child_input(&self, id: NodeId, horiz: bool) -> Option<ChildIn>` (`crates/designos-doc/src/layout_apply.rs:86-91`) resolves variable bindings on the child (`let bound = crate::resolve_node(self, raw);` at `crates/designos-doc/src/layout_apply.rs:88`) and returns `None` when `!bound.visible || raw.layout_child.absolute` (`crates/designos-doc/src/layout_apply.rs:89-91`).
   - In `crates/designos-export/src/codegen.rs`, `sizes_along` computes `let in_flow = parent.al.is_some() && !n.layout_child.absolute && !parent.root;` (`crates/designos-export/src/codegen.rs:451`), and `offset(n, parent, sz)` returns `None` when `parent.root || sz.in_flow`, else `Some((n.transform.tx, n.transform.ty))` (`crates/designos-export/src/codegen.rs:500-505`).
   - For grid frames, `solved_grid(doc, n)` collects only children where `doc.flow_child_input(*id, true).is_some()` into `grid.children` (`crates/designos-export/src/codegen.rs:680-685`).

3. **How `swiftui.rs` and `compose.rs` emitted auto-layout stacks (`HStack`/`VStack`, `Row`/`Column`) and grids (`Grid`, `LazyVerticalGrid`) on base `481b0e7e`**:
   - **SwiftUI `HStack`/`VStack` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:58-82`)**:
     - `body` was populated with all `kids = visible_children(doc, n)` (`crates/designos-export/src/swiftui.rs:380`), so every child (including `k.layout_child.absolute == true` and variable-hidden children where `doc.flow_child_input(k.id, horizontal).is_none()`) was emitted directly inside the `HStack` or `VStack` braces (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:60-77`), participating in stack flow and `SpaceBetween` `Spacer()` insertion (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:64,68,73`).
   - **SwiftUI `Grid` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:32-56`)**:
     - After emitting `GridRow` blocks, out-of-flow children (`!grid.children.iter().any(|(cid, _)| *cid == k.id)`) were emitted inside `Grid(...) { ... }` before the closing `}` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:45-52`), turning each out-of-flow child into a full-width row of `Grid`.
   - **Compose `Row`/`Column` (`crates/designos-export/src/compose.rs:518-602`)**:
     - On base `481b0e7e`, `emit` opened `Row` or `Column` and emitted all `kids` inside the `Row`/`Column` block, placing absolute and variable-hidden children in stack flow (`crates/designos-export/src/compose.rs:518-602`).
   - **Compose `LazyVerticalGrid` (`crates/designos-export/src/compose.rs:413-517`)**:
     - Already wrapped `LazyVerticalGrid` in an outer `Box` when `has_oof` was true (`crates/designos-export/src/compose.rs:421-428`), but passed `me` (`Parent::of(n)` with `al: Some(...)`) when emitting `out_of_flow` children (`crates/designos-export/src/compose.rs:510-515`), causing variable-hidden children (`k.layout_child.absolute == false`) to have `sz.in_flow == true` (`crates/designos-export/src/codegen.rs:451`) and omit `.offset(...)` (`crates/designos-export/src/codegen.rs:500-505`).

### 1.2 Figma Dev Mode Facts (REA Evidence `ev_dbf5a79550ba6e1ad5d143246aa53f051bdf24a9b7733dc628804a790c8cb345`)

- **Artifact analyzed**:
  - Cache file: `~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data/a6f2bcc873943b57_0` (`https://www.figma.com/webpack-artifacts/assets/6413-8e031fdc4fad2bf8.min.js.br`, decompressed `15,095,123` bytes, line `161`, webpack module `117343` at byte offset `2613978..2779999`) (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:1-3`).
  - REA analysis summary: `evidence_id = ev_dbf5a79550ba6e1ad5d143246aa53f051bdf24a9b7733dc628804a790c8cb345`, `root_artifact_sha256 = 63970d4a1b989a8a7fad56aa790b938890182f9847d3cd84e9b31a5af9bd5550`, `inventory_manifest_id = agm_524af7b502a61093af282c83c9745e59a94c413a5644b2f166c33f421473733e` (`plans/reports/agy-1458-native-stack-export-evidence/rea-figma-codegen-117343-summary.json:1`).

1. **Figma SwiftUI `.frame(...)` emission for `FILL_PARENT` + `FIXED` (`tY` at `6413-8e031fdc4fad2bf8.min.js:161`, byte offset `2748195..2749065`, `plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:29`)**:
   - Exact code in `tY(e,t,n,i,r,a,s,o)` (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:29`):
     ```js
     t!==ee.XQ.FILL_PARENT&&n!==ee.XQ.FILL_PARENT?(t===ee.XQ.FIXED&&l.push(`width: ${c}`),n===ee.XQ.FIXED&&l.push(`height: ${p}`)):(t===ee.XQ.FIXED?l.push(`minWidth: ${c}`,`maxWidth: ${c}`):t===ee.XQ.FILL_PARENT&&(l.push("maxWidth: .infinity"),m[0]=void 0),n===ee.XQ.FIXED?l.push(`minHeight: ${p}`,`maxHeight: ${p}`):n===ee.XQ.FILL_PARENT&&(l.push("maxHeight: .infinity"),m[1]=void 0)),l.length&&s&&l.push(`alignment: ${s}`);
     ```
   - When neither axis is `FILL_PARENT`, Figma emits `width: {c}` and/or `height: {p}` (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:29`).
   - When either axis is `FILL_PARENT`, Figma switches to SwiftUI's `frame(minWidth:idealWidth:maxWidth:minHeight:idealHeight:maxHeight:alignment:)` overload (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:29`):
     - `w = FILL_PARENT`, `h = FIXED(p)` -> `.frame(maxWidth: .infinity, minHeight: {p}, maxHeight: {p}[, alignment: {s}])` (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:29`).
     - `w = FIXED(c)`, `h = FILL_PARENT` -> `.frame(minWidth: {c}, maxWidth: {c}, maxHeight: .infinity[, alignment: {s}])` (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:29`).
     - All other `7` `(w, h)` combinations produce the exact same arguments (`width: {c}`, `height: {p}`, `maxWidth: .infinity`, `maxHeight: .infinity`, or empty when both are `HUG_CONTENTS`) (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:29`).

2. **Figma Dev Mode absolute positioning in Compose and SwiftUI (`K` at byte offset `2621936..2622690` and `tH` at byte offset `2749161..2751260`, `plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:9-11,29-31`)**:
   - In Compose (`K` at `plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:9-11`), when `e.layout.shouldUseAbsolutePosition()` is true, Figma reads `e.layout.relativeBounds().bounds.x` and `.y` (relative to the parent frame's top-left) and emits:
     ```js
     if(e.layout.shouldUseAbsolutePosition()){let i=(0,D.hX)(e.layout.relativeBounds().bounds.x,t),r=(0,D.hX)(e.layout.relativeBounds().bounds.y,t);n.offset=[(0,g.n8)(`.offset(x = ${i}.dp, y = ${r}.dp)`)]}
     ```
     With modifier ordering `T=["offset","shadow","blur","borders","size","transform","backgrounds","padding"]` (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:3`).
   - In SwiftUI (`tH` at `plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:31`), Figma's single-node inspector emits `.position(x:${r}, y:${a})` using center coordinates (`bounds.x + bounds.width/2, bounds.y + bounds.height/2`). For multi-node tree export in Studio where containers use `ZStack(alignment: .topLeading)` (`crates/designos-export/src/swiftui.rs:504,557-560`), Ruling 2 specifies wrapping the stack or grid in `ZStack(alignment: .topLeading)` (SwiftUI) / `Box` (Compose) and placing the absolute child over it at its offset from the frame's top-left (`.offset(x: {x}, y: {y})` in SwiftUI at `crates/designos-export/src/swiftui.rs:557-560`, `.offset(x = {dx}, y = {dy})` in Compose at `crates/designos-export/src/compose.rs:331-335`), after the in-flow content in paint order (`crates/designos-export/src/swiftui.rs:647-652,693-698`, `crates/designos-export/src/compose.rs:510-516,574-578`).

---

## Existing Native Fixtures Updated by `#1454`

- `crates/designos-export/tests/grid_native_fixtures.rs` — `fill_child_stacked_inside_column_span_uses_its_own_column_width` (`crates/designos-export/tests/grid_native_fixtures.rs:1496`): updated expected `InnerFill` cell modifier from `.frame(maxWidth: .infinity, height: 20)` to `.frame(maxWidth: .infinity, minHeight: 20, maxHeight: 20)` (`crates/designos-export/tests/grid_native_fixtures.rs:1496`).

---

## Skills Loaded and Concrete Changes Driven by Them

1. **`es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md:1`)**:
   - Change made: Shared `oof_ids: Vec<NodeId>` and `inner_mods: Vec<String>` across the mutually exclusive `grid_body` and `stack` branches in `view` (`crates/designos-export/src/swiftui.rs:385-386,447-452,492-497`) instead of widening `grid_body`'s tuple signature, eliminating `clippy::type_complexity` (`plans/reports/agy-1458-native-stack-export-evidence/cargo-clippy.log:178`).
2. **`es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md:1`)**:
   - Change made: Used `Document::flow_child_input` (`crates/designos-doc/src/layout_apply.rs:86-91`) as the single source of truth for partitioning `kids` into `flow_kids` and `oof_kids` in both `swiftui.rs` (`crates/designos-export/src/swiftui.rs:462`) and `compose.rs` (`crates/designos-export/src/compose.rs:521-522`), guaranteeing parity with the canvas layout solver.
3. **`es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md:1`) & `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md:1`)**:
   - Change made: Recorded the baseline red test run on `481b0e7e` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:180`) and both mutant kills (`plans/reports/agy-1458-native-stack-export-evidence/mutant-1-1454.log:127`, `plans/reports/agy-1458-native-stack-export-evidence/mutant-2-1458.log:47`) as separate artifacts before running the final green gates (`plans/reports/agy-1458-native-stack-export-evidence/branch-green-native-stack-export.log:15`).
4. **`es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md:1`) & `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md:1`)**:
   - Change made: Split the changes into two focused commits — `9b7c2700` for `#1454` (`crates/designos-export/src/swiftui.rs:210-244`, `crates/designos-export/tests/grid_native_fixtures.rs:1496`) and `ea679553` for `#1458` (`crates/designos-export/src/swiftui.rs:385-699`, `crates/designos-export/src/compose.rs:510-590`, `crates/designos-export/tests/native_stack_export.rs:1-552`).
5. **`reverse-engineer-anything` (`/Users/jangtrinh/.gemini/config/skills/reverse-engineer-anything/SKILL.md:1`)**:
   - Change made: Decoded Figma's Chromium Simple Cache bundle `a6f2bcc873943b57_0` (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:1-3`) and ran `rea analyze-javascript-application` (`plans/reports/agy-1458-native-stack-export-evidence/rea-figma-codegen-117343-summary.json:1`), verifying that Figma's `tY` emits `minWidth`/`maxWidth` and `minHeight`/`maxHeight` when paired with `.infinity` (`plans/reports/agy-1458-native-stack-export-evidence/figma-codegen-dump.txt:29`).

---

## Acceptance Table & Pasted Verification Output

| Requirement | Verdict | Evidence Log & Line Citations |
|---|---|---|
| **1(a)** `Fill`-width + `Fixed`-height and `Fixed`-width + `Fill`-height children emit valid SwiftUI `.frame(...)` modifiers | **PASS** | Red on `481b0e7e`: `fill_width_fixed_height_and_fixed_width_fill_height_emit_valid_swiftui_frames ... FAILED` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:10-28,58`). Green on branch: `fill_width_fixed_height_and_fixed_width_fill_height_emit_valid_swiftui_frames ... ok` (`plans/reports/agy-1458-native-stack-export-evidence/branch-green-native-stack-export.log:11`). |
| **1(b)** Every SwiftUI test fixture (`12` files checked) passes `/usr/bin/swiftc -typecheck` when wrapped in `import SwiftUI` | **PASS** | Before (`481b0e7e`): `checked=12 passed=3 failed=9` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:85-170`). After (branch `ea679553`): `checked=12 passed=12 failed=0` (`plans/reports/agy-1458-native-stack-export-evidence/branch-green-native-stack-export.log:13-14`). |
| **1(c)** Absolute & variable-hidden children in `VStack`/`HStack` and `Column`/`Row` emitted in `ZStack`/`Box` overlay at top-left offset after in-flow content; siblings keep positions | **PASS** | Red on `481b0e7e`: `absolute_and_variable_hidden_children_in_stacks_are_emitted_in_overlay_out_of_flow ... FAILED` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:58-84`). Green on branch: `absolute_and_variable_hidden_children_in_stacks_are_emitted_in_overlay_out_of_flow ... ok` (`plans/reports/agy-1458-native-stack-export-evidence/branch-green-native-stack-export.log:10`). |
| **1(d)** Absolute & variable-hidden children in a grid (SwiftUI and Compose) emitted in `ZStack`/`Box` overlay at top-left offset | **PASS** | Red on `481b0e7e`: `absolute_and_variable_hidden_children_in_grid_are_emitted_in_overlay_out_of_flow ... FAILED` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:32-56,83`). Green on branch: `absolute_and_variable_hidden_children_in_grid_are_emitted_in_overlay_out_of_flow ... ok` (`plans/reports/agy-1458-native-stack-export-evidence/branch-green-native-stack-export.log:12`). |
| **Null control** Frames with no absolute children and non-(`Fill`+`Fixed`) frame pairs export byte-for-byte identically to `481b0e7e` | **PASS** | Passed on `481b0e7e` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:10`) and passed on branch (`plans/reports/agy-1458-native-stack-export-evidence/branch-green-native-stack-export.log:9`). |
| **2(a) Mutant 1 (`#1454`)** Emit `.frame(maxWidth:, height:)` for `Fill` + `Fixed` -> new test fails | **PASS** | Killed (`mutant_1_rc=101`, `3 passed; 2 failed` at `plans/reports/agy-1458-native-stack-export-evidence/mutant-1-1454.log:33,119,127`). |
| **2(b) Mutant 2 (`#1458`)** Emit absolute children in flow again -> new test fails | **PASS** | Killed (`mutant_2_rc=101`, `4 passed; 1 failed` at `plans/reports/agy-1458-native-stack-export-evidence/mutant-2-1458.log:38,47`). |
| **3(a)** `cargo fmt --all -- --check; echo fmt_rc=$?` | **PASS** | `fmt_rc=0` (`plans/reports/agy-1458-native-stack-export-evidence/cargo-fmt.log:1`). |
| **3(b)** `cargo clippy --workspace --all-targets --locked` | **PASS** | `clippy_export_rc=0` with `-D warnings` (`plans/reports/agy-1458-native-stack-export-evidence/cargo-clippy.log:178`) and `clippy_workspace_rc=0` with only `main`'s known `designos-app` warnings (`plans/reports/agy-1458-native-stack-export-evidence/cargo-clippy.log:459-500`). |
| **3(c)** `cargo test -p designos-export --locked` | **PASS** | `export_rc=0`, all `19` test binaries passed (`355` tests passed, `0` failed, `5` ignored) (`plans/reports/agy-1458-native-stack-export-evidence/cargo-test-export.log:172-474`). |
| **3(d)** `cargo test -p designos-mcp --locked` | **PASS** | `mcp_rc=0`, all `28` test binaries passed (`558` tests passed, `0` failed) (`plans/reports/agy-1458-native-stack-export-evidence/cargo-test-mcp.log:489-873`). |
| **3(e)** `cargo test --workspace --locked --lib` | **PASS** | `workspace_lib_rc=0`, all `12` library test targets passed (`1,494` tests passed, `0` failed, `3` ignored) (`plans/reports/agy-1458-native-stack-export-evidence/cargo-test-workspace-lib.log:173-1733`). |
| **3(f)** `python3 -B scripts/zero_fake_ui_gate.py` | **PASS** | `Gate Verdict: PASS (17/17 invariants satisfied)`, `zero_fake_rc=0` (`plans/reports/agy-1458-native-stack-export-evidence/zero-fake-ui-gate.log:23-26`). |
| **Cleanup** `rm -rf out/cargo` and confirm `ls out/cargo` fails | **PASS** | `ls: out/cargo: No such file or directory`, `ls_out_cargo_rc=1` (`crates/designos-export/tests/native_stack_export.rs:1`). |

### Pasted Output — Base Red Run on `481b0e7e` (`plans/reports/agy-1458-native-stack-export-evidence/base-red-481b0e7e.log:83-180`)

```text
test absolute_and_variable_hidden_children_in_grid_are_emitted_in_overlay_out_of_flow ... FAILED
test absolute_and_variable_hidden_children_in_stacks_are_emitted_in_overlay_out_of_flow ... FAILED
swiftc_typecheck_summary: checked=12 passed=3 failed=9
--- FAILED 01_grid_equal_tracks ---
.../01_grid_equal_tracks.swift:9:57: error: extra argument 'height' in call
 9 |                     .frame(maxWidth: .infinity, height: 36)
   |                                                         `- error: extra argument 'height' in call
...
--- FAILED 10_stack_fixed_width_fill_height ---
.../10_stack_fixed_width_fill_height.swift:8:31: error: extra argument 'width' in call
 8 |                 .frame(width: 88, maxHeight: .infinity)
   |                               `- error: extra argument 'width' in call
thread 'every_swiftui_test_fixture_passes_swiftc_typecheck' panicked at crates/designos-export/tests/native_stack_export.rs:952:5:
swiftc -typecheck failed on 9/12 SwiftUI fixtures
test every_swiftui_test_fixture_passes_swiftc_typecheck ... FAILED
test result: FAILED. 1 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.47s
```

### Pasted Output — Branch Green Run on `ea679553` (`plans/reports/agy-1458-native-stack-export-evidence/branch-green-native-stack-export.log:7-18`)

```text
running 5 tests
test null_control_frames_without_abs_children_or_fill_fixed_pairs_export_identically ... ok
test absolute_and_variable_hidden_children_in_stacks_are_emitted_in_overlay_out_of_flow ... ok
test fill_width_fixed_height_and_fixed_width_fill_height_emit_valid_swiftui_frames ... ok
test absolute_and_variable_hidden_children_in_grid_are_emitted_in_overlay_out_of_flow ... ok
swiftc_typecheck_summary: checked=12 passed=12 failed=0
test every_swiftui_test_fixture_passes_swiftc_typecheck ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.47s

native_stack_rc=0
```

### Pasted Output — Mutant 1 (`#1454`) Kill (`plans/reports/agy-1458-native-stack-export-evidence/mutant-1-1454.log:11-36,117-129`)

```text
thread 'fill_width_fixed_height_and_fixed_width_fill_height_emit_valid_swiftui_frames' panicked at crates/designos-export/tests/native_stack_export.rs:117:5:
Fill-width + Fixed-height leaf must use minHeight/maxHeight overload:
VStack(alignment: .leading, spacing: 8) {
    Rectangle()
        .fill(Color(red: 0.2, green: 0.4, blue: 0.8))
        .frame(maxWidth: .infinity, height: 44)
...
test fill_width_fixed_height_and_fixed_width_fill_height_emit_valid_swiftui_frames ... FAILED
swiftc_typecheck_summary: checked=12 passed=3 failed=9
thread 'every_swiftui_test_fixture_passes_swiftc_typecheck' panicked at crates/designos-export/tests/native_stack_export.rs:700:5:
swiftc -typecheck failed on 9/12 SwiftUI fixtures
test every_swiftui_test_fixture_passes_swiftc_typecheck ... FAILED
test result: FAILED. 3 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.68s
```

### Pasted Output — Mutant 2 (`#1458`) Kill (`plans/reports/agy-1458-native-stack-export-evidence/mutant-2-1458.log:10-49`)

```text
thread 'absolute_and_variable_hidden_children_in_stacks_are_emitted_in_overlay_out_of_flow' panicked at crates/designos-export/tests/native_stack_export.rs:206:5:
SwiftUI VStack with absolute/hidden children must be wrapped in ZStack(alignment: .topLeading):
VStack(alignment: .leading) {
    Rectangle()
        .fill(Color(red: 0.102, green: 0.2, blue: 0.302))
        .frame(width: 100, height: 24)
    Spacer()
    Rectangle()
        .fill(Color(red: 0.2, green: 0.302, blue: 0.4))
        .frame(width: 100, height: 32)
    Spacer()
    Rectangle()
        .fill(Color(red: 0.902, green: 0.2, blue: 0.2))
        .frame(width: 24, height: 16)
        .offset(x: 260, y: 12)
...
test absolute_and_variable_hidden_children_in_stacks_are_emitted_in_overlay_out_of_flow ... FAILED
test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.57s
```

---

## Assumptions

1. **Variable-hidden children in `visible_children` (`crates/designos-export/src/codegen.rs:239-241`)**: Because `crates/designos-export/src/codegen.rs` is owned by `#1546` and off-limits in this branch, `visible_children(doc, n)` continues to return nodes whose raw `c.visible` is `true` even when a boolean variable binding resolves their effective visibility to `false` (`crates/designos-doc/src/layout_apply.rs:86-91`). Partitioning stack children with `doc.flow_child_input(k.id, horizontal).is_some()` (`crates/designos-export/src/swiftui.rs:462`, `crates/designos-export/src/compose.rs:521-522`) and emitting out-of-flow children with `Parent { al: None, root: false }` (`crates/designos-export/src/swiftui.rs:647,693`, `crates/designos-export/src/compose.rs:511,574`) keeps both `layout_child.absolute` children and variable-hidden children out of stack/grid flow and places them in the `ZStack`/`Box` overlay at their top-left offsets, matching the existing Android XML grid convention (`crates/designos-export/tests/grid_native_fixtures.rs:1955-1972`).

---

## Weakest Link

- **Paint order when an out-of-flow child appears earlier in `n.children` than an in-flow child (`crates/designos-export/src/swiftui.rs:647-652,693-698`, `crates/designos-export/src/compose.rs:510-516,574-578`)**: Because all in-flow children must reside inside the single `HStack`/`VStack`/`Grid` or `Row`/`Column`/`LazyVerticalGrid` container, emitting the `ZStack`/`Box` overlay places all out-of-flow children (in their relative `n.children` paint order) after the in-flow container, as specified by Ruling 2 (`crates/designos-export/src/swiftui.rs:693-698`). If a document places an absolute child underneath in-flow siblings in `n.children` without setting `item_reverse_z_index`, the overlay still renders above the in-flow container unless `zIndex` modifiers are also emitted.

Status: https://github.com/jangtrinh/design-os-studio/pull/1563 | jangtrinh/agy-1458-native-stack-export | ea679553 fix(export): emit absolute and hidden children out of stack and grid flow (#1458); 9b7c2700 fix(export): emit valid SwiftUI min/max frame overload for Fill + Fixed (#1454) | fmt=0 clippy=ok test=ok zero_fake=ok

---

## Correction after review round 2 (supersedes the sections above where they differ)

- **Only children with `layout_child.absolute` leave the flow.** A child hidden by a boolean variable stays where it was before this change: in the stack or grid, with no offset. The round-1 text above (hidden children placed in the overlay, and the test names that asserted it) describes the first version, which the review rejected.
- **Compose:** the inner `Row`/`Column` fills its frame (`fillMaxSize` before the padding) on non-hug axes, so arrangement and counter-axis alignment run in the frame's box.
- **SwiftUI:** the in-flow stack fills the `ZStack` with the frame's layout alignment, and the `ZStack` is placed `.topLeading`, so absolute offsets are measured from the frame's top-left. A frame whose only children are absolute emits just the `ZStack`.
- **Paint order:** absolute children before the first in-flow child are emitted behind the stack or grid; the rest in front, in child order.
- **`swiftc` check:** skipped when `/usr/bin/swiftc` is missing (Linux and Windows CI); 19 of 19 SwiftUI outputs typecheck on macOS.
- Round-2 evidence: `agy-1458-native-stack-export-evidence/r2-*` (red, green, mutants, gates, and SwiftUI `ImageRenderer` before/after renders).
