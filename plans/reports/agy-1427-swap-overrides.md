# Fix #1427: Preserve Swapped Slot Child Overrides, Variable Bindings, Visibility, Name, and Resized Slot/Glyph Geometry

## 1. What Could NOT Be Verified (Stated First)

1. **Interactive desktop GUI pointer click on the Properties panel combo box**: Verified headlessly in `crates/designos-app/src/panels/components.rs:409` (`properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo`) by calling the exact `App` mutation methods that the Properties panel combo boxes invoke (`app.set_instance_prop(inst_id, "Icon", Some(PropValue::Instance(icon_check)))` at `crates/designos-app/src/panels/components.rs:628` inside `instance_props` at `:568`, and `app.swap_instance(slot_id, icon_plus)` at `crates/designos-app/src/panels/components.rs:104` inside `instance_header` at `:91`), plus end-to-end MCP subprocess verification in `plans/reports/agy-1427-swap-overrides-evidence/r3-swap-probe.txt` against a scratch copy of `studio-ui.designos`. Interactive OS window click injection was not run.
2. **Pre-existing Clippy lints in untouched `designos-app` files on `origin/main`**: Running `cargo clippy -p designos-doc -p designos-mcp --all-targets --locked -- -D warnings` on the touched library/MCP crates exits `0` with zero warnings. Running `cargo clippy --workspace --all-targets --locked -- -D warnings` across the full workspace hits pre-existing lints in `designos-app` (`clippy::needless_return` at `crates/designos-app/src/agent_permissions.rs:1163,1181`, `clippy::ptr_arg` at `crates/designos-app/src/panels/components.rs:564`, `clippy::items_after_test_module` at `crates/designos-app/src/panels/components.rs:263`, and `unused_must_use` at `crates/designos-app/src/panels/components.rs:363` introduced on `origin/main` when `m4` reverted the 3 out-of-scope edits in `panels/components.rs`). Passing `-A clippy::needless_return -A clippy::ptr_arg -A clippy::items_after_test_module -A unused_must_use` exits `0` across the entire workspace.
3. **macOS 104-byte `SUN_LEN` Unix domain socket path limit in `designos-mcp` `session_file_io::tests::a_folder_at_a_rename_target_and_a_socket_are_refused_as_not_plain_files`**: At `crates/designos-mcp/src/session_file_io/tests.rs:138-139`, the test binds a Unix domain socket at `std::env::temp_dir().join(format!("ds{}.sock", std::process::id()))`. Because our worktree path `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1427-swap-overrides` is 73 characters, setting `TMPDIR="$(pwd)/out/test-home/tmp"` (92 chars) produces a 105-byte socket path that exceeds Darwin's 104-byte `SUN_LEN` limit, whereas setting `TMPDIR="$(pwd)/out/t"` (80 chars, still inside the worktree's `out/` folder) produces a 93-byte socket path and passes all 297 unit + 199 integration tests in `designos-mcp` (`0 failed`).

---

## 2. Shared Swap Path & Caller Inventory

All instance swaps in DESIGN:OS Studio converge on `crates/designos-doc/src/components.rs`:

| Surface / Crate | Entry Point | Call Chain into Shared Swap Engine | Notes |
|---|---|---|---|
| **`designos-doc`** (core scene graph) | `swap_instance(doc, instance, new_comp)` (`crates/designos-doc/src/components.rs:1631`) | `swap_instance` -> `swap_main(&mut c, instance, new_comp)` (`:1303`) -> `sync_one(&mut c, instance, ...)` -> `sync_pair` (`:1465`) | Direct swap of a top-level or nested `NodeKind::Instance`. |
| **`designos-doc`** (component props & variant sync) | `set_instance_prop(doc, instance, name, value)` (`crates/designos-doc/src/components.rs:1651`) & `sync_instances(doc, component)` (`:792`) | `set_instance_prop` -> `sync_one` -> `sync_pair` (`:1465`) -> detects `PropValue::Instance(sw)` on `OverrideField::MainComponent` (`:1479`) -> `swap_main(c, i, desired)` (`:1493`) -> `apply_reference` (`:1076`) -> recurses into children | Evaluates `InstanceSwap` property bindings (`"Icon": "Icon/Check"`) and variant switches on nested slots. |
| **`designos-mcp`** (`designos_apply_spec` / `designos_apply_batch`) | `spec.props` (`crates/designos-doc/src/spec_instance.rs:1023`, `apply_instance_props_with_ctx`) | `set_instance_prop(doc, instance_id, &canonical_name, Some(PropValue::Instance(target_comp_id)))` |Previously had a post-hoc `saved_slot_names_and_sizes` workaround in `spec_instance.rs` that only restored root slot `name`/`size` and missed child overrides, variable bindings, visibility, and `spec.overrides[].swap`. Removed now that `components.rs` preserves them at the document layer. |
| **`designos-mcp`** (`designos_apply_spec` / `designos_apply_batch`) | `spec.overrides[].swap` (`crates/designos-doc/src/spec_instance.rs:1181`, `apply_single_override_with_ctx`) | `swap_instance(doc, target_id, new_comp)` | Shares the exact same `swap_main` + `sync_one` path. |
| **`designos-app`** (Properties panel) | `instance_header` (`crates/designos-app/src/panels/components.rs:91`) & `instance_props` (`:568`) | `app.swap_instance(id, comps[i])` (`panels/components.rs:104`) and `app.set_instance_prop(inst, &def.name, Some(PropValue::Instance(comps[i])))` (`panels/components.rs:628`) | Both snapshot undo via `self.modify(...)` and call `designos_doc::swap_instance` / `designos_doc::set_instance_prop`. |
| **`designos-fig`** (`.fig` / REST import) | `crates/designos-fig/src/fig.rs` & `crates/designos-fig/src/rest_import.rs` | Constructs `NodeKind::Instance` directly from Kiwi `symbolData` / REST JSON and applies `symbolOverrides` in-place during import. | Does not invoke `swap_instance` or `set_instance_prop` (never-touch files; untouched). |

### Root Causes Fixed in `crates/designos-doc/src/components.rs`

1. **`effective_child_overrides` across nested component chains (`components.rs:641-687`)**:
   - Previously, `swap_main` only carried child overrides when `!kn.component.overrides.is_empty()` and only copied `kn.component.overrides`.
   - When an outer component/variant (e.g. `Button/Primary` `Size=Md, State=Disabled`) overrides a nested `Icon` slot's `Glyph` (setting fill `#0A0A0A` at `0.5` paint opacity, binding `fills/0 -> s.color.text-on-brand`, or resizing `Glyph` from `16x16` to `20x20`), those overrides live on the variant's slot child (`308`), whereas the outer instance's materialized copy (`2401`) has `component.source = Some(308)` and `component.overrides = []`. Moreover, `record_override` did not inspect `variables.bindings`.
   - `effective_child_overrides(d, old_comp, kn)` now walks `kn.component.source` up to `old_comp`, unions explicit `OverrideField`s along the chain, diffs `kn` against its counterpart `orig` inside `old_comp` via `record_override(orig, kn)`, and diffs `orig.variables.bindings` vs `kn.variables.bindings`.
   - Matched by child name (`by_name`) and `std::mem::discriminant(&n.kind) == std::mem::discriminant(&kn.kind)`: matching children (e.g. `Glyph` `Vector`) receive `copy_overridden(&kn, &mut n, &ov, copy_bindings)` + `n.component.overrides = ov`; non-matching children drop overrides cleanly.
2. **Preserving the swapped slot's own `name`, `size`, `visible`, and `component.bindings` in `sync_pair` (`components.rs:549-592`)**:
   - Previously, after `swap_main` swapped a nested slot (`!top`), `sync_pair` replaced `rn` with `new_comp`'s root (`Icon/Check`, `name = "Icon/Check"`, `visible = true`, `size = 16x16`, `bindings = {}`) and called `apply_reference(&rn, &mut n, false, &values)`, clobbering the slot's name (`"Leading icon"`), boolean-bound visibility (`visible = false`, `Visible -> "Has Leading Icon"`), and slot size (`20x20`).
   - Now, when `!top && instance_component(&inode) != instance_component(&slot_rn)`, `sync_pair` keeps `slot_rn` (the outer component's slot node) as the base `node_ref` so the slot's `name`, `visible`, `transform`, and `component.bindings` remain authoritative, while adopting non-overridden visual/layout properties from `new_comp` (`rn`) only where `slot_rn` had not customized them relative to its default main component.

---

## 3. Round: Resized Slot (`20 px` Stays `20`), Child Vector Scaling, and Post-Hoc Resize Diagnosis

Per the controller scope addition on `#1427` (`#issuecomment-6538129630`):

1. **Swap keeps the resized slot size (`20x20`) AND scales the swapped child `Glyph` (`20x20` + vector paths)**:
   - **Rule matched to Figma**:
     - In `swap_main` (`crates/designos-doc/src/components.rs:719-747`): a swapped instance or nested slot keeps its existing `n.size` (`20x20`) whenever `OverrideField::Size` is set OR `old_comp_node.size != n.size` (e.g. a `20x20` slot referencing a `16x16` `Icon/*` main component). When `target_size != comp.size`, `swap_main` runs `plan_constraint_pass(&c.s, i, comp.size, target_size, 8)` + `apply_constrained_geometry` on cloned children.
     - Furthermore, when the slot's child `Glyph` in the outer variant was itself resized from `16x16` to `20x20` (as in `studio-ui.designos` `Button/Primary` `Size=Md`, `Button/Secondary Destructive` `Size=Md`, and `Tool Button`), `effective_child_overrides` detects `OverrideField::Size` (`20x20` vs `16x16` in `old_comp`), and both `copy_overridden` (`components.rs:619-632`) and `apply_reference` (`components.rs:845-859`) call `crate::layout_constraints::apply_constrained_geometry(&mut n, old_tf, orig_size, target_size)` when preserving `OverrideField::Size` on a non-text, non-frame child (`NodeKind::Vector`).
   - **Verified on `studio-ui.designos` (`swap-native-after.txt`)**:
     - `tool-sel-swap`: nested `'Icon'` `size={'x': 20.0, 'y': 20.0}`, `glyph id=2394 size={'x': 20.0, 'y': 20.0}`
     - `tool-hover-swap`: nested `'Icon'` `size={'x': 20.0, 'y': 20.0}`, `glyph id=2398 size={'x': 20.0, 'y': 20.0}`
     - `prim-dis-swap`: nested `'Leading icon'` `size={'x': 20.0, 'y': 20.0}`, `glyph id=2407 size={'x': 20.0, 'y': 20.0}`
     - `sd-hover-swap`: nested `'Leading icon'` `size={'x': 20.0, 'y': 20.0}`, `glyph id=2416 size={'x': 20.0, 'y': 20.0}`
     - `close-hover-swap` (16px icon button): nested `'Icon'` `size={'x': 16.0, 'y': 16.0}`, `glyph id=2411 size={'x': 16.0, 'y': 16.0}`
   - **Unit tests added (`designos-doc` & `designos-app`)**:
     - Both `swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo` (`crates/designos-doc/src/components.rs`) and `properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo` (`crates/designos-app/src/panels/components.rs`) resize the `Leading icon` slot and its `Glyph` from `16x16` to `20x20` before swapping to `Icon/Check` (`16x16` with path `MoveTo(4.0, 8.0), LineTo(12.0, 12.0)`) and `Icon/Plus` (`16x16`), and assert that after both `set_instance_prop` and `swap_instance`:
       - `slot.size == Pt::new(20.0, 20.0)`
       - `glyph.size == Pt::new(20.0, 20.0)`
       - `glyph`'s `NodeKind::Vector` path commands are scaled by `20/16 = 1.25x` to `MoveTo(5.0, 10.0), LineTo(15.0, 15.0)`.
2. **Why post-hoc resizing via `designos_update_node` does not scale vector paths (Separate Bug — Not Modified Here)**:
   - Inspecting `crates/designos-mcp/src/tools/mod.rs:266-271` (`build_updated_node`) shows that `designos_update_node` mutates `node.size.x = width; node.size.y = height;` directly on the target node without calling `designos_doc::layout_constraints::apply_constrained_geometry` (for `NodeKind::Vector` / `BooleanOp` paths) or `plan_constraint_pass` (for container children).
   - Additionally, in `design/studio-ui.designos`, the `Icon/*` main components (`Icon/Plus`, `Icon/Check`, `Icon/X`, etc.) have `constraints: None` (which defaults to `Constraint::Min, Constraint::Min`) on their `Glyph` child rather than `Constraint::Scale, Constraint::Scale`.
   - Therefore, post-hoc `designos_update_node` resizing not scaling vector paths is a **separate bug** in `designos-mcp`'s `build_updated_node` (and asset constraint defaults), not part of instance swap (`swap_main` / `sync_pair`). Per controller instructions, it is reported here and left untouched in this PR.

---

## 4. Skills Loaded & How They Shaped the Fix

| Skill | Path | Concrete Effect on This Task |
|---|---|---|
| **`es:dev` / `es:feature-dev`** | `/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md` | Enforced root-cause fix in the shared `designos-doc` layer rather than duplicating caller workarounds across `spec_instance.rs` and `panels/components.rs`; removed the partial `saved_slot_names_and_sizes` workaround in `spec_instance.rs`. |
| **`es:rust`** | `/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md` | Used borrowed slice/node lookups inside `effective_child_overrides` and `sync_pair` before taking mutable borrows on `d.s`, avoiding unnecessary whole-tree clones and keeping `OverrideField` deduplication allocation-minimal. |
| **`es:rust-gpu-canvas`** | `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md` | Verified that scaling `NodeKind::Vector` `PathCmd` coordinates via `apply_constrained_geometry` updates the canonical geometry consumed by both `designos-render` (CPU `tiny-skia`) and `designos-gpu` (`lyon` tessellation). |
| **`es:fable-thinking`** | `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md` | Falsified the initial hypothesis that `kn.component.overrides` held the variant's child overrides by inspecting `studio-ui.designos` node `2401` (`source: 308, overrides: []`), leading directly to `effective_child_overrides` walking the `component.source` chain and diffing `variables.bindings`. |
| **`es:astra-work`** | `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md` | Captured red/green logs before/after the code change, ran both reviewer probes (`swap_probe.py`, `swap_vis.py`) against a scratch copy of `studio-ui.designos`, and executed all 5 verification gates. |

---

## 5. Acceptance & Verification Table

Durable evidence files are saved under `plans/reports/agy-1427-swap-overrides-evidence/`:
- `plans/reports/agy-1427-swap-overrides-evidence/red.log`
- `plans/reports/agy-1427-swap-overrides-evidence/green.log`
- `plans/reports/agy-1427-swap-overrides-evidence/swap-probe-after.txt`
- `plans/reports/agy-1427-swap-overrides-evidence/swap-native-after.txt`
- `plans/reports/agy-1427-swap-overrides-evidence/swap-visibility-after.txt`

| Check / Gate | Exact Command | Result / Output Summary |
|---|---|---|
| **Red test (before fix)** | `cargo test -p designos-doc --locked -- swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo --nocapture` & `cargo test -p designos-app --bin designos --locked -- properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo --nocapture` | **FAIL (expected red)**: `assertion left == right failed: left: "Icon/Check", right: "Leading icon"` (`plans/reports/agy-1427-swap-overrides-evidence/red.log`). |
| **Green test (after fix)** | Same commands as above | **PASS (`0 failed`)**: `test components::tests::swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo ... ok` and `test panels::components::props_panel_tests::properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo ... ok` (`plans/reports/agy-1427-swap-overrides-evidence/green.log`). |
| **Reviewer Probe 1 (`swap_probe.py` on `studio-ui.designos`)** | `python3 "$S/swap_probe.py"` against `out/cargo/debug/designos-mcp` + `/Users/jangtrinh/Products/design-os-studio/.claude/worktrees/agent-a1b72103724702a19/design/studio-ui.designos` | **PASS**: `tool-sel-swap` glyph `#FFFFFF op1.00 bind={'fills/0': 's.color.icon'} size=20x20`; `tool-hover-swap` glyph `#E5E5E5 op1.00 bind={'fills/0': 's.color.icon-secondary-hover'} size=20x20`; `prim-dis-swap` glyph `#0A0A0A op0.50 bind={'fills/0': 's.color.text-on-brand'} size=20x20`; `close-hover-swap` glyph `#A3A3A3 op1.00 bind={'fills/0': 's.color.icon-quaternary-hover'} size=16x16`; `sd-hover-swap` glyph `#FCA5A5 op1.00 bind={'fills/0': 's.color.text-error-hover'} size=20x20` (`swap-native-after.txt`). |
| **Reviewer Probe 2 (`swap_vis.py` on `studio-ui.designos`)** | `python3 "$S/swap_vis.py"` against `out/cargo/debug/designos-mcp` + `studio-ui.designos` | **PASS**: `off-swap Leading icon visible= False bindings= {'Visible': 'Has Leading Icon', 'MainComponent': 'Icon'} glyph=#A3A3A3 w= {'x': 60.75, 'y': 36.0}` (matches `off-noswap` `60.75x36.0`; `on-swap` is `visible= True`, `84.75x36.0`) (`swap-visibility-after.txt`). |
| **Gate 1: Rustfmt** | `cargo fmt --all -- --check` | **PASS (`exit 0`)** |
| **Gate 2: Clippy** | `cargo clippy -p designos-doc -p designos-mcp --all-targets --locked -- -D warnings` & `cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::needless_return` | **PASS (`exit 0`, 0 warnings in touched crates/files)** |
| **Gate 3: Workspace lib tests** | `cargo test --workspace --locked --lib` | **PASS (`exit 0`)**: `designos-doc` 673 passed; `designos-mcp` 263 passed; `designos-render` 143 passed; `designos-fig` 153 passed; `designos-gpu` 56 passed; 0 failed across all 12 workspace lib targets. |
| **Gate 4: MCP tests** | `cargo test -p designos-mcp --locked` | **PASS (`exit 0`)**: 263 unit + 185 integration tests passed, 0 failed. |
| **Gate 5: App binary tests** | `cargo test -p designos-app --bin designos --locked -- panels::components::tests::properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo --exact --nocapture` & full `cargo test -p designos-app --bin designos --locked` | **PASS (`1 passed; 0 failed` on new test; `1 passed; 0 failed` on isolated rerun of `agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled`; `3860 passed` in full run)**. |

### Pasted Key Outputs

```text
=== Red -> Green (plans/reports/agy-1427-swap-overrides-evidence/red.log & green.log) ===
# Before fix:
test components::tests::swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo ... FAILED
assertion `left == right` failed
  left: "Icon/Check"
 right: "Leading icon"

# After fix:
test components::tests::swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo ... ok
test panels::components::props_panel_tests::properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo ... ok

=== Reviewer Probes After Fix (swap-native-after.txt & swap-visibility-after.txt) ===
tool-sel-swap     nested 'Icon' size={'x': 20.0, 'y': 20.0} tx_ty=(6.0, 6.0) visible=True comp={'source': 629, 'bindings': {'MainComponent': 'Icon'}}
tool-sel-swap       glyph id=2394 size={'x': 20.0, 'y': 20.0} max_path_xy=None fill=['{'r': 1.0, 'g': 1.0, 'b': 1.0, 'a': 1.0} op1.00'] bind={'fills/0': 's.color.icon'} comp={'source': 43, 'overrides': ['Size', 'Fills']}
tool-hover-swap   nested 'Icon' size={'x': 20.0, 'y': 20.0} tx_ty=(6.0, 6.0) visible=True comp={'source': 616, 'bindings': {'MainComponent': 'Icon'}}
tool-hover-swap     glyph id=2398 size={'x': 20.0, 'y': 20.0} max_path_xy=None fill=['{'r': 0.8980392, 'g': 0.8980392, 'b': 0.8980392, 'a': 1.0} op1.00'] bind={'fills/0': 's.color.icon-secondary-hover'} comp={'source': 43, 'overrides': ['Size', 'Fills']}
prim-dis-swap     nested 'Leading icon' size={'x': 20.0, 'y': 20.0} tx_ty=(12.0, 10.0) visible=True comp={'source': 307, 'bindings': {'Visible': 'Has Leading Icon', 'MainComponent': 'Icon'}}
prim-dis-swap       glyph id=2407 size={'x': 20.0, 'y': 20.0} max_path_xy=None fill=['{'r': 0.039215688, 'g': 0.039215688, 'b': 0.039215688, 'a': 1.0} op0.50'] bind={'fills/0': 's.color.text-on-brand'} comp={'source': 41, 'overrides': ['Size', 'Fills']}
close-hover-swap  nested 'Icon' size={'x': 16.0, 'y': 16.0} tx_ty=(8.0, 8.0) visible=True comp={'source': 637, 'bindings': {'MainComponent': 'Icon'}}
close-hover-swap    glyph id=2411 size={'x': 16.0, 'y': 16.0} max_path_xy=None fill=['{'r': 0.6392157, 'g': 0.6392157, 'b': 0.6392157, 'a': 1.0} op1.00'] bind={'fills/0': 's.color.icon-quaternary-hover'} comp={'source': 27, 'overrides': ['Fills']}
sd-hover-swap     nested 'Leading icon' size={'x': 20.0, 'y': 20.0} tx_ty=(12.0, 8.0) visible=True comp={'source': 533, 'bindings': {'Visible': 'Has Leading Icon', 'MainComponent': 'Icon'}}
sd-hover-swap       glyph id=2416 size={'x': 20.0, 'y': 20.0} max_path_xy=None fill=['{'r': 0.9882353, 'g': 0.64705884, 'b': 0.64705884, 'a': 1.0} op1.00'] bind={'fills/0': 's.color.text-error-hover'} comp={'source': 165, 'overrides': ['Size', 'Fills']}

off-swap Leading icon visible= False bindings= {'Visible': 'Has Leading Icon', 'MainComponent': 'Icon'} glyph=#A3A3A3 w= {'x': 60.75, 'y': 36.0}
off-noswap Leading icon visible= False bindings= {'Visible': 'Has Leading Icon', 'MainComponent': 'Icon'} glyph=#A3A3A3 w= {'x': 60.75, 'y': 36.0}
on-swap Leading icon visible= True bindings= {'Visible': 'Has Leading Icon', 'MainComponent': 'Icon'} glyph=#A3A3A3 w= {'x': 84.75, 'y': 36.0}
```

---

## 6. Epistemic Ledger, Weakest Link & Unresolved Questions

- **`FACT`**: Every instance swap path (`swap_instance`, `set_instance_prop`, `designos_apply_spec` `props` & `overrides[].swap`, and the Properties panel combo boxes) executes `swap_main` + `sync_pair` in `crates/designos-doc/src/components.rs`.
- **`FACT`**: Both reviewer scripts (`swap_probe.py` and `swap_vis.py`) and the unit tests in `designos-doc` and `designos-app` confirm that swapping a slot via `InstanceSwap` prop or direct `swap_instance` preserves child fill overrides, paint/node opacity, `variables.bindings`, boolean-bound slot visibility (`visible = false`), slot name (`"Leading icon"`), resized slot & glyph dimensions (`20x20`), and scaled vector path geometry, while dropping overrides when child names do not match and restoring the previous main component + overrides in a single `undo()`.
- **`FACT`**: `designos_update_node` (`crates/designos-mcp/src/tools/mod.rs:266-271`) mutates `node.size` directly without calling `apply_constrained_geometry`, which is why post-hoc MCP node resizing does not scale vector paths (separate from instance swap).
- **`INFERENCE`**: Removing the partial `saved_slot_names_and_sizes` workaround in `crates/designos-doc/src/spec_instance.rs` has no regressions because `swap_main` and `sync_pair` now preserve slot `name` and `size` for all callers (verified by all 496 tests in `designos-mcp`).
- **`ASSUMPTION`**: Matching child overrides by base-name hierarchy path + field-exists guard + originally-matched gate across swapped components matches Figma's override preservation rules for icon/component swaps.
- **Weakest link**: None identified after Round 3.
- **Unresolved questions**: None.

---

## Round 3: Glyph Geometry Double-Scaling Fix & Figma-Aligned Swap Rules

### 1. What Could NOT Be Verified (Round 3)

- Same items documented at the top of Section 1 (`What Could NOT Be Verified`): no interactive OS pointer click injection on the Properties panel combo box (verified headlessly via `App::set_instance_prop` / `App::swap_instance` in `crates/designos-app/src/panels/components.rs:409` and via `designos-mcp` stdio JSON-RPC in `plans/reports/agy-1427-swap-overrides-evidence/r3-swap-probe.txt`), pre-existing Clippy lints on `origin/main` in `crates/designos-app/src/agent_permissions.rs:1163,1181` and `crates/designos-app/src/panels/components.rs:263,363,564`, and macOS's 104-byte `SUN_LEN` Unix domain socket path limit in `crates/designos-mcp/src/session_file_io/tests.rs:138-139` when `TMPDIR` exceeds 90 characters (avoided by setting `TMPDIR="$(pwd)/out/t"` inside the worktree's `out/` folder).

### 2. Rulings Implemented & Verified Against Final Head (`m3`)

Every file path, symbol, and line number below is verified against the final `HEAD` of `jangtrinh/agy-1427-swap-overrides`:

| Ruling | Spec / Decision | File, Symbol & Exact Line Range (Final Head) | Implementation Summary |
|---|---|---|---|
| **B1** (`fill_geometry` / `stroke_geometry` double-scaling) | Rescaling in `apply_reference` must NOT scale a node's own `fill_geometry` / `stroke_geometry` a second time after `copy_overridden` or `swap_main`'s constraint pass already scaled it. | `crates/designos-doc/src/components.rs:1201-1207` (`apply_reference`) | Saves `let own = (n.fill_geometry.take(), n.stroke_geometry.take());` before calling `apply_constrained_geometry(n, n.transform, target_size)` (`:1205`) and restores `(n.fill_geometry, n.stroke_geometry) = own;` (`:1206`). |
| **B2 + M2** (top-level variant switch) | Revert top-level variant-switch size logic to base (`a2cf6a84`). A variant switch creates no `Size` overrides by fitting, and a difference from the old main does not become an override. | `crates/designos-doc/src/components.rs:1307-1311`, `:1321-1327`, `:1362-1365`, `:1383` (`swap_main`) | `top_swap` (`is_top(&inode)`) collects only explicit `kn.component.overrides` minus `MainComponent` (`:1321-1325`), never calling `effective_child_overrides`. When `top_swap && same_family`, `n.size = comp.size` unless `n.component.overrides.contains(&OverrideField::Size)` (`:1362-1365`), and the child constraint pass (`:1383`) is skipped. |
| **M1** (Matching by base name + field-exists guard, Figma fact) | Overrides match by the layer's base name in `old_comp`, walking the hierarchy, with no layer-type (`discriminant`) gate; field-specific guards prevent copying inapplicable fields. | `crates/designos-doc/src/components.rs:1225-1240` (`orig_in_main`), `:1319-1320` (`swap_main::collect`), `:992-998` & `:1019-1045` (`copy_field`), `:1440-1446` (`swap_main`) | `orig_in_main` resolves `kn`'s counterpart in `old_comp`; `collect` pushes `orig.map(\|o\| o.name.clone()).unwrap_or_else(\|\| kn.name.clone())` (`:1320`). `copy_field` copies `fill_geometry`/`stroke_geometry` for `OverrideField::Size` only onto `NodeKind::Vector { .. } \| NodeKind::BooleanOp(_)` (`:994-997`), `Characters`/`TextStyle` only onto `NodeKind::Text(_)` (`:1020-1037`), and `Layout` `frame_props` only onto frames (`:1040-1042`). `swap_main` removes `Characters`/`TextStyle` when `!matches!((&old.kind, &nn.kind), (NodeKind::Text(_), NodeKind::Text(_)))` (`:1440-1443`) and `Layout` when either node lacks `frame_props()` (`:1444-1446`). |
| **"Originally matched" gate** (Figma fact) | On non-variant-switch swaps (`!(top_swap && same_family)`), a non-text field override (`!matches!(f, OverrideField::Characters \| OverrideField::TextStyle \| OverrideField::Name)`) survives only if the old main's value for that field equals the new main's. | `crates/designos-doc/src/components.rs:1447-1452` (`swap_main`) | Compares `orig` (in `old_comp`) against `new_orig` (`nn.component.source.and_then(\|s\| c.s.get(s))` in `new_comp`) via `let diff = record_override(orig, new_orig);` (`:1449`) and retains `f` only when `matches!(f, OverrideField::Characters \| OverrideField::TextStyle \| OverrideField::Name) \|\| !diff.contains(f)` (`:1450`). |
| **M3** (Name, Studio decision) | An instance whose name equals its old main's name and has no `OverrideField::Name` counts as auto-named and takes the new main's name after a cross-component swap; any other name is kept. | `crates/designos-doc/src/components.rs:1359-1361` (`swap_main`) | `if !same_family && !n.component.overrides.contains(&OverrideField::Name) && old_comp_node.as_ref().is_some_and(\|oc\| oc.name == n.name) { n.name = comp.name.clone(); }`. |
| **m1** (Size per axis, Studio decision) | Per axis on non-variant-switch swaps: if the instance's size equals its old main's on an axis (`<= 1e-4`), it takes `comp.size` on that axis; if it differs, it keeps its own. | `crates/designos-doc/src/components.rs:1366-1375` (`swap_main`), `:1516-1525` (`sync_pair`) | Computes `keep_x` and `keep_y` independently against `old_comp_node` (`:1367-1368`) and `orig_slot_comp` (`:1517-1518`), updating `size.x` and `size.y` per axis. |
| **M4** (Child layout & constraints, Figma fact) | Slot-derived child scaling is not an absolute child `Size` override; `Vector` / `BooleanOp` descendants with default constraints (`Min, Min`) scale proportionally during `swap_main`'s constraint pass. | `crates/designos-doc/src/components.rs:1263-1278` (`effective_child_overrides`), `:1383-1422` (`swap_main`) | Removed `OverrideField::Size` from the `record_override(orig, kn)` `matches!` list in `effective_child_overrides` (`:1264-1275`). In `swap_main` (`:1383-1411`), `Vector` / `BooleanOp` descendants with `Constraints::default()` temporarily upgrade to `Scale, Scale` during `plan_constraint_pass(&c.s, i, comp.size, target_size, 8)` (`:1400`) and then restore `Constraints::default()`, so an inset `12x12` glyph at `(2, 2)` in a `16x16` main scales to `15x15` at `(2.5, 2.5)` in a `20x20` slot (never `20x20`). |
| **m4** (Revert unrelated `panels/components.rs` lines) | Revert the 3 unrelated changes in `crates/designos-app/src/panels/components.rs`. | `crates/designos-app/src/panels/components.rs:263`, `:363`, `:564` | Reverted `#[allow(clippy::items_after_test_module)]` at `:263`, `let _ = ctx.run_ui(...)` at `:363`, and `sort_props_by_kind(defs: &mut Vec<ComponentPropDef>)` at `:564` so only `properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo` (`:409-561`) differs from `origin/main`. |
| **m5** (Remove extra node clones in `sync_pair`) | Remove the two extra full node clones (`slot_rn = rn.clone()`, `orig_slot_comp = ...cloned()`) from the top of `sync_pair`. | `crates/designos-doc/src/components.rs:1466-1468`, `:1511-1515` (`sync_pair`) | Keeps only `let slot_comp = instance_component(&rn);` (`:1467`) at the top of `sync_pair`, cloning `c.s.get(r)` (`:1512`) and borrowing `orig_slot_comp = slot_comp.and_then(\|oc\| c.s.get(oc))` (`:1514`) only inside `if !top && instance_component(&inode) != slot_comp`. |

### 3. Callers Touched & Test Coverage Across All 4 Callers

| Caller | Production Code Touched? | Test Function & Exact File + Line | What the Test Covers |
|---|---|---|---|
| **1. `designos-doc` (`components.rs`)** | **Yes** (`crates/designos-doc/src/components.rs:992-998`, `:1201-1207`, `:1225-1459`, `:1467`, `:1511-1525`) | `probe_p1_sync_instances_does_not_double_scale_fill_geometry` (`:4424`), `probe_p2_swap_instance_same_family_preserves_custom_name` (`:4507`), `probe_p3_swap_instance_preserves_overridden_size` (`:4541`), `probe_p4_variant_switch_keeps_new_variant_child_offsets` (`:4585`), `probe_p5_swap_main_matches_renamed_child_by_source_name` (`:4634`), `probe_p6_variant_switch_without_size_override_takes_new_variant_size` (`:4680`), `probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size` (`:4718`), `ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards` (`:4835`) | Covers `P1`–`P7`, `B1`, `B2 + M2`, `M1`, `M3`, `M4`, `m1`, the "originally matched" gate, and all 8 surviving mutants (`M4, M6a, M6b, M7, M9, M10, M11, M12`). |
| **2. `designos-doc` (`spec_instance.rs`)** | **No** (Round 1 removed the partial `saved_slot_names_and_sizes` workaround; `apply_instance_props_with_ctx` at `:1023` and `apply_single_override_with_ctx` at `:1181` delegate directly to `set_instance_prop` and `swap_instance`) | `spec_swap_paths_apply_auto_vs_custom_name_per_axis_size_and_originally_matched_gate` (`crates/designos-doc/src/spec_instance.rs:3673`) | Exercises `apply_spec` with `props` (`InstanceSwap`) and `overrides[].swap`, verifying auto-named vs custom-named swap (`M3`), per-axis size preservation (`m1`), and the originally-matched gate (`IconCheckSame` keeps fill override, `IconDiffFill` drops fill override). |
| **3. `designos-app` (`panels/components.rs`)** | **Reverted 3 out-of-scope lines (`m4`)**; `instance_header` (`:91`) and `instance_props` (`:568`) call `app.swap_instance` and `app.set_instance_prop` | `properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo` (`crates/designos-app/src/panels/components.rs:409`) | Exercises the Properties panel's `app.set_instance_prop` and `app.swap_instance` paths on a `20x20` resized slot (`Leading icon`) and `16x16` `Icon/Check` / `Icon/Plus` components, plus auto-named vs custom-named top-level instance swap and single-step `app.undo()`. |
| **4. `designos-mcp` (`engine.rs`)** | **No** (delegates to `designos_doc::swap_instance` and `apply_spec`) | `apply_batch_and_apply_spec_instance_swap_name_and_override_rules` (`crates/designos-mcp/src/engine.rs:4508`) | Exercises MCP `apply_spec` and `apply_batch` (`swap_instance`) end-to-end, asserting `M3` (auto-named updates, custom name kept), `m1` (per-axis size `(24, 16) -> (24, 32)`), and the originally-matched gate. |

### 4. Mutant Kill Matrix (`M4`, `M6a`, `M6b`, `M7`, `M9`, `M10`, `M11`, `M12`)

Every one of the 8 surviving mutants from Round 2 was mutated one at a time in `crates/designos-doc/src/components.rs` and verified `KILLED` (`exit=101`) in `plans/reports/agy-1427-swap-overrides-evidence/r3-mutants.log`:

| Mutant ID | Mutated Line(s) in `crates/designos-doc/src/components.rs` | Test That Kills It | Assertion That Fails When Mutated |
|---|---|---|---|
| **`M4`** | `:1280-1282` (`if orig.variables.bindings.keys().chain(kn.variables.bindings.keys()).any(\|k\| matches!(k.as_str(), "width" \| "height")) { fields.insert(OverrideField::Size); }`) | `ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards` (`:4835`, panic at `:5130`) | `slot_glyph`'s `"width" -> "size.icon.w"` variable binding override is not detected in `effective_child_overrides`, so after swapping `slot` to `icon_v2`, `g.variables.bindings.get("width")` is `None` instead of `Some("size.icon.w")`. |
| **`M6a`** | `:1414-1416` (`if child.size != old_sz { child.component.overrides.insert(OverrideField::Size); }`) | `probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size` (`:4718`, panic at `:4816`) | Constraint-scaled child glyph (`15x15` inside a `20x20` slot) is missing `OverrideField::Size` (and resets to `12x12` on `sync_instances(&mut d, icon_b)`). |
| **`M6b`** | `:1417-1419` (`if child.transform != old_tf { child.component.overrides.insert(OverrideField::Transform); }`) | `probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size` (`:4718`, panic at `:4817`) | Constraint-scaled child glyph (`tx = 2.5, ty = 2.5` inside a `20x20` slot) is missing `OverrideField::Transform` (and resets to `tx = 2.0, ty = 2.0` on `sync_instances(&mut d, icon_b)`). |
| **`M7`** | `:1440-1443` (`if !matches!((&old.kind, &nn.kind), (NodeKind::Text(_), NodeKind::Text(_))) { fields.remove(&OverrideField::Characters); fields.remove(&OverrideField::TextStyle); }`) | `ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards` (`:4835`, panic at `:4969`) | Swapping a component whose `"Item"` child is `NodeKind::Text` (with `Characters` + `TextStyle` overrides) to a component whose `"Item"` child is `NodeKind::Frame` leaves `OverrideField::Characters` / `OverrideField::TextStyle` in `nn.component.overrides`. |
| **`M9`** | `:1258, 1262` (`fields.extend(chain_fields);` in `effective_child_overrides`) | `ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards` (`:4835`, panic at `:5138`) | An explicit `OverrideField::Layout` (`clips_content = true`) set on `slot_wrap` inside `Holder`'s intermediate slot chain (`sn.component.overrides`) is dropped when swapping `outer_inst`'s slot to `icon_v2`. |
| **`M10`** | `:1444-1446` (`if old.kind.frame_props().is_none() \|\| nn.kind.frame_props().is_none() { fields.remove(&OverrideField::Layout); }`) | `ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards` (`:4835`, panic at `:5045`) | Switching a variant whose `"Box"` child is a `Frame` with `OverrideField::Layout` to a variant whose `"Box"` child is a `Rectangle` leaves `OverrideField::Layout` on the `Rectangle`. |
| **`M11`** | `:1353` (`if !n.component.overrides.contains(&OverrideField::Layout)` on `NodeKind::Instance { frame, .. }`) | `ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards` (`:4835`, panic at `:4960`) | Instance root `inst`'s overridden `frame.clips_content = true` (`OverrideField::Layout`) is overwritten by `comp.kind.frame_props()` (`clips_content = false`) during `swap_main`. |
| **`M12`** | `:1413` (`crate::layout_constraints::apply_constrained_geometry(&mut child, new_tf, new_sz);` in `swap_main`) | `probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size` (`:4718`, panic at `:4818`) | Swapped child glyph inside a `20x20` slot remains unscaled at `12x12` (`tx = 2.0`) instead of scaling to `15x15` (`tx = 2.5`). |

### 5. Pasted Key Lines from Round 3 Evidence Logs

Durable Round 3 evidence logs are committed in `plans/reports/agy-1427-swap-overrides-evidence/`:
- `plans/reports/agy-1427-swap-overrides-evidence/r3-red.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r3-green.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r3-mutants.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r3-swap-probe.txt`
- `plans/reports/agy-1427-swap-overrides-evidence/r3-gates.log`

```text
=== 1. r3-red.log (Before Fix — All New Tests Fail as Predicted) ===
failures:
    components::tests::probe_p1_sync_instances_does_not_double_scale_fill_geometry
    components::tests::probe_p2_swap_instance_same_family_preserves_custom_name
    components::tests::probe_p4_variant_switch_keeps_new_variant_child_offsets
    components::tests::probe_p5_swap_main_matches_renamed_child_by_source_name
    components::tests::probe_p6_variant_switch_without_size_override_takes_new_variant_size
    components::tests::probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size
    components::tests::ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards
    spec_instance::tests::spec_swap_paths_apply_auto_vs_custom_name_per_axis_size_and_originally_matched_gate
test result: FAILED. 674 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.49s

failures:
    engine::tests::apply_batch_and_apply_spec_instance_swap_name_and_override_rules
test result: FAILED. 296 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.72s

failures:
    panels::components::props_panel_tests::properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3887 filtered out; finished in 0.01s

=== 2. r3-green.log (After Fix — All Tests Pass) ===
running 682 tests
test components::tests::probe_p2_swap_instance_same_family_preserves_custom_name ... ok
test components::tests::probe_p3_swap_instance_preserves_overridden_size ... ok
test components::tests::probe_p4_variant_switch_keeps_new_variant_child_offsets ... ok
test components::tests::probe_p5_swap_main_matches_renamed_child_by_source_name ... ok
test components::tests::probe_p6_variant_switch_without_size_override_takes_new_variant_size ... ok
test components::tests::probe_p1_sync_instances_does_not_double_scale_fill_geometry ... ok
test components::tests::probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size ... ok
test components::tests::ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards ... ok
test spec_instance::tests::spec_swap_paths_apply_auto_vs_custom_name_per_axis_size_and_originally_matched_gate ... ok
test result: ok. 682 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.90s
running 297 tests
test engine::tests::apply_batch_and_apply_spec_instance_swap_name_and_override_rules ... ok
test result: ok. 297 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.74s
running 1 test
test panels::components::props_panel_tests::properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3887 filtered out; finished in 0.02s

=== 3. r3-mutants.log (All 8 Mutants Killed) ===
=== M4 (ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards) -> KILLED (exit=101) ===
=== M6a (probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size) -> KILLED (exit=101) ===
=== M6b (probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size) -> KILLED (exit=101) ===
=== M7 (ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards) -> KILLED (exit=101) ===
=== M9 (ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards) -> KILLED (exit=101) ===
=== M10 (ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards) -> KILLED (exit=101) ===
=== M11 (ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards) -> KILLED (exit=101) ===
=== M12 (probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size) -> KILLED (exit=101) ===
ALL 8 MUTANTS KILLED AND SOURCE RESTORED.

=== 4. r3-swap-probe.txt (studio-ui.designos Swap + Re-Sync Probe) ===
tool-sel-swap     glyph id=2402 size={'x': 20.0, 'y': 20.0} max_vec=17.5 max_fill_geom=17.5 max_stroke_geom=None fill=["{'r': 1.0, 'g': 1.0, 'b': 1.0, 'a': 1.0} op=1.00"] bind={'fills/0': 's.color.icon'} overrides=['Size', 'Fills']
tool-hover-swap   glyph id=2406 size={'x': 20.0, 'y': 20.0} max_vec=17.5 max_fill_geom=17.5 max_stroke_geom=None fill=["{'r': 0.8980392, 'g': 0.8980392, 'b': 0.8980392, 'a': 1.0} op=1.00"] bind={'fills/0': 's.color.icon-secondary-hover'} overrides=['Size', 'Fills']
prim-dis-swap     glyph id=2415 size={'x': 20.0, 'y': 20.0} max_vec=18.1865 max_fill_geom=18.1865 max_stroke_geom=None fill=["{'r': 0.039215688, 'g': 0.039215688, 'b': 0.039215688, 'a': 1.0} op=0.50"] bind={'fills/0': 's.color.text-on-brand'} overrides=['Size', 'Fills']
close-hover-swap  glyph id=2419 size={'x': 16.0, 'y': 16.0} max_vec=14.0 max_fill_geom=14.0 max_stroke_geom=None fill=["{'r': 0.6392157, 'g': 0.6392157, 'b': 0.6392157, 'a': 1.0} op=1.00"] bind={'fills/0': 's.color.icon-quaternary-hover'} overrides=['Fills']
sd-hover-swap     glyph id=2424 size={'x': 20.0, 'y': 20.0} max_vec=17.5 max_fill_geom=17.5 max_stroke_geom=None fill=["{'r': 0.9882353, 'g': 0.64705884, 'b': 0.64705884, 'a': 1.0} op=1.00"] bind={'fills/0': 's.color.text-error-hover'} overrides=['Size', 'Fills']
off-swap   slot='Leading icon' visible=False bindings={'Visible': 'Has Leading Icon', 'MainComponent': 'Icon'} glyph_size={'x': 20.0, 'y': 20.0} btn_size={'x': 60.75, 'y': 40.0}
off-noswap slot='Leading icon' visible=False bindings={'Visible': 'Has Leading Icon', 'MainComponent': 'Icon'} glyph_size={'x': 20.0, 'y': 20.0} btn_size={'x': 60.75, 'y': 40.0}
on-swap    slot='Leading icon' visible=True bindings={'Visible': 'Has Leading Icon', 'MainComponent': 'Icon'} glyph_size={'x': 20.0, 'y': 20.0} btn_size={'x': 84.75, 'y': 40.0}
=== 3. After Repeated Sync / Re-Sync (Checking fill_geometry / stroke_geometry / size stay 20x20, NOT 25) ===
tool-sel-swap   slot_size={'x': 20.0, 'y': 20.0} glyph_size={'x': 20.0, 'y': 20.0} max_vec=17.5 max_fill_geom=17.5 max_stroke_geom=None (before re-sync=({'x': 20.0, 'y': 20.0}, 17.5, 17.5, None))
tool-hover-swap slot_size={'x': 20.0, 'y': 20.0} glyph_size={'x': 20.0, 'y': 20.0} max_vec=17.5 max_fill_geom=17.5 max_stroke_geom=None (before re-sync=({'x': 20.0, 'y': 20.0}, 17.5, 17.5, None))
prim-dis-swap   slot_size={'x': 20.0, 'y': 20.0} glyph_size={'x': 20.0, 'y': 20.0} max_vec=18.1865 max_fill_geom=18.1865 max_stroke_geom=None (before re-sync=({'x': 20.0, 'y': 20.0}, 18.1865, 18.1865, None))
sd-hover-swap   slot_size={'x': 20.0, 'y': 20.0} glyph_size={'x': 20.0, 'y': 20.0} max_vec=17.5 max_fill_geom=17.5 max_stroke_geom=None (before re-sync=({'x': 20.0, 'y': 20.0}, 17.5, 17.5, None))
ALL SWAP & RE-SYNC ASSERTIONS PASSED.
```

### 6. Round 3 Epistemic Ledger, Weakest Link & Unresolved Questions

- **`FACT`**: In `apply_reference` (`crates/designos-doc/src/components.rs:1201-1207`), `apply_constrained_geometry` previously scaled both `NodeKind::Vector { paths }` (copied fresh from `r.kind` at `:1198`) and `n.fill_geometry` / `n.stroke_geometry` (which had already been scaled to `target_size` by `swap_main` or `copy_overridden`), causing `fill_geometry` / `stroke_geometry` to grow by an extra `target_size / r.size` factor (`20 * 20/16 = 25`) on sync (`P1` / `B1`). Taking `(n.fill_geometry.take(), n.stroke_geometry.take())` around `apply_constrained_geometry` keeps `Vector.paths` and `fill_geometry`/`stroke_geometry` bit-identical (`17.5` / `18.1865` in `r3-swap-probe.txt`).
- **`FACT`**: In `studio-ui.designos`, all `Icon/*` main components (`Icon/Plus`, `Icon/Check`, `Icon/Sliders`, `Icon/Trash`, etc.) share the exact same default `Glyph` fill (`#FFFFFF`, opacity `1.0`) and variable binding (`{"fills/0": "s.color.icon"}`), so `record_override(orig, new_orig)` is empty and all button variant icon overrides in `studio-ui.designos` pass the "originally matched" gate (`components.rs:1447-1452`).
- **`FACT`**: All 8 surviving mutants from Round 2 (`M4, M6a, M6b, M7, M9, M10, M11, M12`) are killed by `probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size` and `ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards` (`r3-mutants.log`).
- **`INFERENCE`**: Temporarily upgrading `Constraints::default()` (`Min, Min`) to `Scale, Scale` on `Vector` / `BooleanOp` descendants of a swapped instance during `swap_main`'s `plan_constraint_pass` (`components.rs:1383-1411`) reproduces Figma's proportional icon-glyph scaling inside resized slots (`12x12` at `(2, 2)` in `16x16` -> `15x15` at `(2.5, 2.5)` in `20x20`, `P7`) without altering the node's persisted `Constraints::default()`.
- **`ASSUMPTION`**: Using `1e-4` as the per-axis floating-point tolerance (`(n.size.x - oc.size.x).abs() > 1e-4` at `components.rs:1367-1368` and `:1517-1518`) cleanly separates untouched main-component dimensions from deliberate per-axis size overrides.
- **Weakest link**: Temporary `Scale, Scale` promotion during `swap_main` applies to `Vector` / `BooleanOp` descendants whose constraints are `Constraints::default()` (`Min, Min`); if an icon component intentionally authored a `Vector` child to stay pinned top-left (`Min, Min`) rather than scale with the icon frame, `swap_main` will scale it proportionally with the slot. (Resolved in Round 4 below: `Min, Min` promotion removed.)
- **Unresolved questions**: None.

---

## Round 4: Localize Non-Vector Geometry Guard to `swap_main`, Unify Per-Axis Nested-Slot Size Rule, and Stop Promoting `Min/Min` to `Scale`

### 1. What Could NOT Be Verified (Round 4 — Stated First)

1. **Interactive desktop OS pointer click on the Properties panel combo box**: Verified headlessly via `App::set_instance_prop` (`crates/designos-app/src/panels/components.rs:628`) and `App::swap_instance` (`crates/designos-app/src/panels/components.rs:104`) in `crates/designos-app/src/panels/components.rs:409` (`properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo`), plus end-to-end MCP subprocess verification in `plans/reports/agy-1427-swap-overrides-evidence/r4-swap-check.log`, `r4-redraw-bounds.log`, `r4-30-swaps.log`, and `r4-redraw-syncs.log`.
2. **Pre-existing Clippy lints in untouched `designos-app` files on `origin/main`**: `cargo clippy -p designos-doc -p designos-mcp --all-targets --locked -- -D warnings` exits `0` with zero warnings. Raw `cargo clippy --workspace --all-targets --locked -- -D warnings` hits the 5 pre-existing errors on `origin/main` in `crates/designos-app/src/agent_permissions.rs:1163,1181` and `crates/designos-app/src/panels/components.rs:263,363,566` (left untouched per Round 3 `m4`). Passing `-A clippy::needless_return -A clippy::ptr_arg -A clippy::items_after_test_module -A unused_must_use` exits `0` across the entire workspace (`plans/reports/agy-1427-swap-overrides-evidence/r4-gates.log`).
3. **Full unfiltered `designos-app` binary test suite (`~3,920` tests)**: Per Acceptance Item 3 of the Round 4 brief, ran the four targeted `designos-app` filter suites (`component` 115 passed, `instance` 97 passed, `variant` 19 passed, `librar` 179 passed; `0 failed`).

### 2. Round 4 Rulings, Commits & Verified File:Line Locations (`eae251c9`)

| Item / Ruling | Red Test Commit | Green Fix Commit | File, Symbol & Exact Line Range (`eae251c9`) | Summary of Fix |
|---|---|---|---|---|
| **Merge `origin/main`** | — | `539d98f2a` | `crates/designos-doc/src/components.rs` | Merged latest `origin/main` cleanly into `jangtrinh/agy-1427-swap-overrides`. |
| **1. `BLK-1` (`copy_field` restored to base; non-vector geometry guard local to `swap_main`)** | `5a0f3235` | `799a43a7` | `crates/designos-doc/src/components.rs:1001-1005` (`copy_field`), `:1450-1455` (`swap_main`) | Restored `copy_field(OverrideField::Size)` to unconditionally copy `to.size`, `to.fill_geometry`, and `to.stroke_geometry` (`:1001-1005`) so `apply_reference` updates imported non-`Vector` shapes (`Ellipse`, `Rectangle`, `Polygon`, `Star`) when resized in their main component. In `swap_main` (`:1450-1455`), saved `let own_geo = (nn.fill_geometry.clone(), nn.stroke_geometry.clone());` before `copy_overridden(&old, &mut nn, &fields);` and restored `(nn.fill_geometry, nn.stroke_geometry) = own_geo;` when `std::mem::discriminant(&old.kind) != std::mem::discriminant(&nn.kind)`. |
| **2. `BLK-2` (One per-axis size rule for a nested slot in `swap_main` and `sync_pair`)** | `c0c90eb4` | `3ff6a394` | `crates/designos-doc/src/components.rs:1406-1416` (`swap_main`), `:1467`, `:1515-1523`, `:1620-1636` (`sync_pair`) | Removed `OverrideField::Size` / `OverrideField::Transform` stamping from `swap_main`'s constraint pass (`:1406-1416`). In `sync_pair`, captured `let inst_slot_size = inode.size;` (`:1467`) before `swap_main`, compared `inst_slot_size` per axis against `orig_slot_comp` (`:1515-1523`), and after syncing children of a swapped nested slot (`:1620-1636`), ran `plan_constraint_pass(&c.s, i, rn.size, slot_after, 8)` + `apply_constrained_geometry` for non-overridden children when `slot_after != rn.size`. |
| **3. `MAJ-1` / `M4` (`Min/Min` is not `Scale`)** | `87d84f8d` | `1d5ad830` | `crates/designos-doc/src/components.rs:1406-1416` (`swap_main`) | Removed `default_vec_ids` temporary `Scale` promotion in `swap_main` so `Min/Min` (`Constraints::default()`) stays pinned (`12x12` at `(2, 2)`, `24x24` at `(8, 8)`) while `Scale/Scale` scales proportionally (`15x15` at `(2.5, 2.5)`), matching `layout_constraints::solve_child` (`crates/designos-doc/src/layout_constraints.rs:28-55`). |
| **4. `MIN-4` (Top-level variant switch `Layout` revert)** | `3e63028e` | `228128de` | `crates/designos-doc/src/components.rs:1375-1377` (`swap_main`) | Reverted the `if !n.component.overrides.contains(&OverrideField::Layout)` guard on `if let Some(f) = comp.kind.frame_props() { *frame = f.clone(); }` in `swap_main` (`:1375-1377`), matching base `a2cf6a84`. |
| **5. `MIN-1` (Auto-name for variant instances)** | `dc008a44` | `2ad88748` | `crates/designos-doc/src/components.rs:1379-1384` (`swap_main`) | Extended `auto_named` in `swap_main` (`:1379-1381`) so an instance without `OverrideField::Name` is auto-named when `n.name == oc.name` OR when `oc`'s parent is a `NodeKind::ComponentSet` whose `set.name == n.name` (matching `create_instance` at `crates/designos-doc/src/components.rs:163-167`). |
| **6. `MIN-2` (Inner swap below a swapped slot — count dropped inner swaps)** | `02927a67` | `47d99953` | `crates/designos-doc/src/components.rs:44-51` (`LAST_DROPPED_INNER_SWAPS`, `last_dropped_inner_swaps`), `:1322-1357` (`swap_main`) | Added `pub fn last_dropped_inner_swaps() -> usize` (`:49-51`) and counted inner `NodeKind::Instance` descendants below a swapped instance/slot whose `MainComponent` was overridden (`kn.component.overrides.contains(&OverrideField::MainComponent) \|\| orig.and_then(instance_component) != instance_component(kn)`) at `:1336-1340`. |
| **7. `MIN-3` (Ported Round 2 probes `P2`, `P4`, `P5`, `P6`, 3-sync `B1`, and pinned mutants)** | — | `4a26a19e` | `crates/designos-doc/src/components.rs:4441-4526`, `:5195-5948` | Ported `probe_p2_swap_drops_override_on_type_mismatch` (`:5247`), `probe_p4_variant_switch_does_not_turn_refit_size_into_override` (`:5722`), `probe_p5_size_rule_is_whole_vector` (`:5372`), `probe_p6_top_level_variant_switch_children_match_root` (`:5765`), 3-sync loop in `probe_p1_sync_instances_does_not_double_scale_fill_geometry` (`:4504-4525`), `r3_b1_swap_then_three_syncs_matches_direct_20px_glyph` (`:5808`), and `r4_top_level_swap_does_not_carry_diff_derived_overrides` (`:5910`). |
| **8. `MIN-5` & Format (`sync_pair` redundant clone removal + `rustfmt`)** | — | `9289e34c`, `eae251c9` | `crates/designos-doc/src/components.rs:1465-1467` (`sync_pair`) | Confirmed `sync_pair` has no redundant `slot_rn.clone()` at top (`:1465-1467`), removed redundant `slot_rn` clone inside test helper, and formatted with `cargo fmt --all` (`eae251c9`). |

### 3. Round 4 Mutant Kill Table (`R-AXIS-SYNC`, `R-COPYFIELD-SWAP-GUARD`, `R-COPYFIELD-BASE`, `R-M2`)

Full output saved in `plans/reports/agy-1427-swap-overrides-evidence/r4-mutants.log`:

| Mutant ID | Mutated Line(s) in `crates/designos-doc/src/components.rs` | Test That Kills It | Failure When Mutated |
|---|---|---|---|
| **`R-AXIS-SYNC`** | `:1516-1517` (`let keep_x = false; let keep_y = false;` in `sync_pair`) | `r3_m1_nested_slot_widened_by_outer_instance` (`:5305`) | `exit=101 (KILLED)`: `s1.size` becomes `Vec2 { x: 80.0, y: 40.0 }` instead of `Vec2 { x: 280.0, y: 40.0 }` (`bg1.size` also shrinks on `sync_instances(&mut d, card)`). |
| **`R-COPYFIELD-SWAP-GUARD`** | `:1450` (`let kinds_differ = false;` in `swap_main`) | `probe_p2_swap_drops_override_on_type_mismatch` (`:5247`) | `exit=101 (KILLED)`: `ng.fill_geometry` is `Some(...)` instead of `None` when a `Vector` child with `OverrideField::Size` is swapped onto a `Frame` child. |
| **`R-COPYFIELD-BASE`** | `:1003-1004` (guard `fill_geometry`/`stroke_geometry` copy in `copy_field` to `Vector \| BooleanOp` only) | `r3_copy_field_size_on_imported_shapes_after_main_resize` (`:5195`) | `exit=101 (KILLED)`: `ellipse shape fill_geometry must sync after main resize` panics (`instance` keeps `10x10` `fill_geometry` while `main` has `20x20`). |
| **`R-M2`** | `:1342` (`if false {` instead of `if top_swap {` in `swap_main::collect`) | `r4_top_level_swap_does_not_carry_diff_derived_overrides` (`:5910`) | `exit=101 (KILLED)`: top-level swap carries diff-derived `OverrideField::Visible` (`visible = false`) onto `b`'s `"Badge"` child instead of keeping `b`'s default `visible = true`. |

### 4. `studio-ui.designos` Swap, Redraw Bounds, 30-Swap & 3-Sync Verification

All commands executed against a scratch copy in `/tmp/r4-studio-ui-scratch/` (never writing to `/Users/jangtrinh/Products/design-os-studio/.claude/worktrees/agent-a1b72103724702a19/`):

1. **`add_size_overrides.py`** (`plans/reports/agy-1427-swap-overrides-evidence/r4-sizeov-patch.log`):
   - On pre-patch `3c14eff87:design/studio-ui.designos`: `Size override added: 75 main-level icon slots, 75 glyphs`.
   - On `HEAD` (`ff8151485:design/studio-ui.designos`, where the redraw worktree already committed those 75 icon `Size` overrides): `Size override added: 0 main-level icon slots, 0 glyphs` (all 134 resized main-level icon slots and their glyphs already carry `"Size"` in `overrides`).
2. **`run_swap_check.py`** (`plans/reports/agy-1427-swap-overrides-evidence/r4-swap-check.log`):
   - Output: `SWAP-OK #1427: 6 swapped variants keep colour, binding, opacity, visibility and the drawn glyph's bounds; SWAP_CHECK_FATAL can flip to True`.
3. **`redraw_bounds.py`** (`plans/reports/agy-1427-swap-overrides-evidence/r4-redraw-bounds.log`):
   - Output: `WORST swapped max|delta| vs direct = 0.0000 px (limit 0.5)` across all 6 swapped cases (`swapck-0-swap` through `swapck-5-swap`).
4. **30-swap + hidden button slot + 1-step undo check** (`plans/reports/agy-1427-swap-overrides-evidence/r4-30-swaps.log`):
   - Output: `30-SWAP-OK: prop_swaps=15/15 direct_swaps=15/15 hidden_slot_visible=False undo_ok=True`.
5. **1- and 3-sync check (`r3_redraw_swapped_glyph_after_syncs` + `r3_main_resize_instance_shape_render`)** (`plans/reports/agy-1427-swap-overrides-evidence/r4-redraw-syncs.log`):
   - Across `sync0`, `sync1`, `sync2`, and `sync3`, every swapped variant (`swapck-0-swap` through `swapck-5-swap`) maintains `max|d|=0.0000` against its direct `20x20` / `16x16` icon glyph (`ov={Size, Fills}` on `20x20` slots, `ov={Fills}` on `16x16` slots).
6. **Dropped inner-swap count (`MIN-2`)**:
   - In `r3_inner_swap_below_swapped_slot_by_index` (`crates/designos-doc/src/components.rs:5623`), swapping an outer slot (`Icon/A` -> `Icon/B`) whose inner `Badge` child had been swapped from `Badge/Red` to `Badge/Blue` records `last_dropped_inner_swaps() == 1`.

### 5. Pasted Key Outputs from Round 4 Evidence Logs

Durable Round 4 evidence files committed under `plans/reports/agy-1427-swap-overrides-evidence/`:
- `plans/reports/agy-1427-swap-overrides-evidence/r4-red.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r4-green.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r4-mutants.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r4-sizeov-patch.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r4-swap-check.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r4-redraw-bounds.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r4-30-swaps.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r4-redraw-syncs.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r4-reviewer-probes.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r4-gates.log`

```text
=== 1. r4-red.log (Each Item's Test Fails Before Its Fix Commit) ===
# Item 1 (5a0f3235):
test components::tests::r3_copy_field_size_on_imported_shapes_after_main_resize ... FAILED
panicked at crates/designos-doc/src/components.rs:5238:13:
ellipse shape fill_geometry must sync after main resize

# Item 2 (c0c90eb4):
test components::tests::r3_m1_nested_slot_widened_by_outer_instance ... FAILED
assertion `left == right` failed: slot width must stay 280 after swap
  left: Vec2 { x: 80.0, y: 40.0 }
 right: Vec2 { x: 280.0, y: 40.0 }

# Item 3 (87d84f8d):
failures:
    components::tests::r3_m4_minmin_glyph_swap_vs_shared_solver
    components::tests::r3_m4_minmin_logo_distorts_on_per_axis_kept_width
    components::tests::r3_m4_minmin_logo_distorts_on_user_resized_card
test result: FAILED. 0 passed; 3 failed

# Item 4 (3e63028e):
test components::tests::ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards ... FAILED
assertion `left == right` failed: swap_main must copy the new main's FrameProps onto the instance root (base behavior)

# Item 5 (dc008a44):
test components::tests::r3_name_rule_variant_instance_cross_family ... FAILED
  left: "Button"
 right: "Badge"

# Item 6 (02927a67):
test components::tests::r3_inner_swap_below_swapped_slot_by_index ... FAILED
  left: 0
 right: 1

=== 2. r4-mutants.log (All 4 Mutants Killed) ===
=== Mutant R-AXIS-SYNC ===
R-AXIS-SYNC exit=101 (KILLED)
=== Mutant R-COPYFIELD-SWAP-GUARD ===
R-COPYFIELD-SWAP-GUARD exit=101 (KILLED)
=== Mutant R-COPYFIELD-BASE ===
R-COPYFIELD-BASE exit=101 (KILLED)
=== Mutant R-M2 ===
R-M2 exit=101 (KILLED)

=== 3. studio-ui.designos Swap, Bounds, 30-Swap & 3-Sync Checks ===
SWAP-OK #1427: 6 swapped variants keep colour, binding, opacity, visibility and the drawn glyph's bounds; SWAP_CHECK_FATAL can flip to True
WORST swapped max|delta| vs direct = 0.0000 px (limit 0.5)
30-SWAP-OK: prop_swaps=15/15 direct_swaps=15/15 hidden_slot_visible=False undo_ok=True
PROBE redraw sync3 swapck-0-swap: slot="Icon" size=Vec2 { x: 20.0, y: 20.0 } glyph size=Vec2 { x: 20.0, y: 20.0 } at=(0,0) drawn=Some((2.4164767, 2.4164767, 17.517036, 17.501709)) direct=Some((2.4164767, 2.4164767, 17.517036, 17.501709)) max|d|=0.0000 ov={Size, Fills}
PROBE redraw sync3 swapck-3-swap: slot="Leading icon" size=Vec2 { x: 20.0, y: 20.0 } glyph size=Vec2 { x: 20.0, y: 20.0 } at=(0,0) drawn=Some((2.438536, 4.9386616, 18.186462, 16.250431)) direct=Some((2.438536, 4.9386616, 18.186462, 16.250431)) max|d|=0.0000 ov={Size, Fills}
PROBE redraw sync3 swapck-4-swap: slot="Leading icon" size=Vec2 { x: 20.0, y: 20.0 } glyph size=Vec2 { x: 20.0, y: 20.0 } at=(0,0) drawn=Some((2.5, 1.25, 17.5, 17.5)) direct=Some((2.5, 1.25, 17.5, 17.5)) max|d|=0.0000 ov={Size, Fills}
PROBE redraw sync3 swapck-5-swap: slot="Leading icon" size=Vec2 { x: 20.0, y: 20.0 } glyph size=Vec2 { x: 20.0, y: 20.0 } at=(0,0) drawn=Some((2.438536, 4.9386616, 18.186462, 16.250431)) direct=Some((2.438536, 4.9386616, 18.186462, 16.250431)) max|d|=0.0000 ov={Size, Fills}

=== 4. r4-gates.log (All 6 Acceptance Gates) ===
=== GATE 1: cargo fmt --all -- --check ===
fmt exit=0
=== GATE 2b: cargo clippy -p designos-doc -p designos-mcp --all-targets --locked -- -D warnings ===
clippy_touched_crates exit=0
=== GATE 2c: cargo clippy --workspace --all-targets --locked -- -D warnings (allowing 4 pre-existing origin/main lints in designos-app) ===
clippy_workspace_clean exit=0
=== GATE 3: cargo test -p designos-doc --locked ===
test result: ok. 695 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.46s
designos-doc exit=0
=== GATE 4: cargo test -p designos-mcp --locked ===
test result: ok. 297 passed; 0 failed (plus 199 integration tests passed; 0 failed)
designos-mcp exit=0
=== GATE 5a..5d: cargo test -p designos-app --bin designos --locked {component,instance,variant,librar} ===
component: 115 passed; 0 failed | instance: 97 passed; 0 failed | variant: 19 passed; 0 failed | librar: 179 passed; 0 failed
=== GATE 6: cargo test --workspace --locked --lib ===
workspace lib exit=0 (1,438 unit tests passed across all 12 workspace lib targets; 0 failed)
```

Status: DONE
