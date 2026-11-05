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
| **7. `MIN-3` (Ported Round 2 probes `P2`, `P4`, `P5`, `P6`, 3-sync `B1`, and pinned mutants)** | — | `4a26a19e` | `crates/designos-doc/src/components.rs:4441-4526`, `:5195-5948` | Ported `probe_p2_swap_drops_override_on_type_mismatch` (`:5247`), `probe_p4_variant_switch_does_not_turn_refit_size_into_override` (`:5722`), `probe_p5_size_rule_is_per_axis` (originally named `probe_p5_size_rule_is_whole_vector` at `:5372`; renamed in Round 5 `:5394`), `probe_p6_top_level_variant_switch_children_match_root` (`:5765`), 3-sync loop in `probe_p1_sync_instances_does_not_double_scale_fill_geometry` (`:4504-4525`), `r3_b1_swap_then_three_syncs_matches_direct_20px_glyph` (`:5808`), and `r4_top_level_swap_does_not_carry_diff_derived_overrides` (`:5910`). |
| **8. `MIN-5` & Format (`sync_pair` redundant clone removal + `rustfmt`)** | — | `9289e34c`, `eae251c9` | `crates/designos-doc/src/components.rs:1465-1467` (`sync_pair`) | Confirmed `sync_pair` has no redundant `slot_rn.clone()` at top (`:1465-1467`), removed redundant `slot_rn` clone inside test helper, and formatted with `cargo fmt --all` (`eae251c9`). |

### 3. Round 4 Mutant Kill Table (`R-AXIS-SYNC`, `R-COPYFIELD-SWAP-GUARD`, `R-COPYFIELD-BASE`, `R-M2`)

Full output saved in `plans/reports/agy-1427-swap-overrides-evidence/r4-mutants.log` (corrected per Round 5 `MIN-2`):

| Mutant ID | Mutated Line(s) in `crates/designos-doc/src/components.rs` | Test That Kills It | Failure When Mutated |
|---|---|---|---|
| **`R-AXIS-SYNC`** | `:1516-1517` (`let keep_x = orig_slot_comp.is_some_and(\|oc\| inst_slot_size != oc.size); let keep_y = keep_x;` in `sync_pair`, replacing per-axis check with whole-vector comparison) | `probe_p5_size_rule_is_per_axis` (`:5372`, renamed from `probe_p5_size_rule_is_whole_vector`) and `r3_m1_nested_slot_widened_by_outer_instance` (`:5305`) | `exit=101 (KILLED)`: when only width was stretched (`280 != 80`) and height was untouched (`32 == 32`), whole-vector comparison sets `keep_y = true`, keeping the old height `32.0` instead of taking `Btn/B`'s height `40.0` (`s.size == Vec2 { x: 280.0, y: 32.0 } != Vec2 { x: 280.0, y: 40.0 }`). (Note: `r4-mutants.log` originally mutated `keep_x = false; keep_y = false;`, which only tested `r3_m1_nested_slot_widened_by_outer_instance`.) |
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

=== 4. r4-gates.log (All 6 Acceptance Gates — Exact Commands & Flags Stated per MIN-2) ===
=== GATE 1: cargo fmt --all -- --check ===
fmt exit=0
=== GATE 2a: cargo clippy --workspace --all-targets --locked -- -D warnings (no -A flags) ===
exit=101 (5 pre-existing errors on origin/main in crates/designos-app/src/agent_permissions.rs:1163,1181 and crates/designos-app/src/panels/components.rs:263,363,566; 0 warnings/errors in designos-doc or designos-mcp)
=== GATE 2b: cargo clippy -p designos-doc -p designos-mcp --all-targets --locked -- -D warnings ===
clippy_touched_crates exit=0
=== GATE 2c: cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::needless_return -A clippy::ptr_arg -A clippy::items_after_test_module -A unused_must_use ===
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

---

## Round 5: Swapped Nested Instances Keep Following Their Main; Cross-Type Glyph Outlines (`#1427` DEV Round 5)

### 1. What Could NOT Be Verified (Round 5 — Stated First)

1. **Interactive desktop OS pointer click on the Properties panel combo box**: Verified headlessly through the App's `App::set_instance_prop` (`crates/designos-app/src/comp_ops.rs:452`), `App::swap_instance` (`crates/designos-app/src/comp_ops.rs:462`), and `App::edit_nodes` (`crates/designos-app/src/ops.rs:244`) entry points in `properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo` (`crates/designos-app/src/panels/components.rs:409`) and `review_r4_app_swapped_slot_follows_new_main_height_edit` (`crates/designos-app/src/panels/components.rs:566`), plus live `designos-mcp` subprocess verification in `plans/reports/agy-1427-swap-overrides-evidence/r5-swap-check.log`, `r5-redraw-bounds.log`, and `r5-redraw-syncs.log`.
2. **Pre-existing Clippy errors on `origin/main` in `designos-app`**: Running `cargo clippy --workspace --all-targets --locked -- -D warnings` (with no `-A` flags) exits `101` solely because of 5 pre-existing lint errors in untouched `origin/main` lines of `crates/designos-app/src/agent_permissions.rs:1163,1181` (`clippy::needless_return`) and `crates/designos-app/src/panels/components.rs:263,363,613` (`clippy::items_after_test_module`, `unused_must_use`, `clippy::ptr_arg`), which were left untouched per Round 3 `m4` (`plans/reports/agy-1427-swap-overrides-evidence/r5-gates.log`). Zero warnings or errors occur in `designos-doc`, `designos-mcp`, or any modified line.
3. **Full unfiltered `designos-app` binary test suite (`~3,964` tests)**: Per Acceptance Item 3 of the Round 5 brief, ran the four mandatory targeted filter suites (`component` 116 passed, `instance` 97 passed, `variant` 19 passed, `librar` 179 passed; `0 failed` when `HOME` and `TMPDIR` are siblings, and `178 passed, 1 failed` on `terminal::ask_mode_tests::bind_session_on_a_library_file_with_a_held_lock_clears_grants_and_keeps_the_dir` when `HOME=$S/home` sits inside `TMPDIR=$S/`).

### 2. Skills Loaded & Changes Made Because of Them

- **`es:dev` & `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Enforced red-test-commit-before-green-fix-commit sequencing for each ruling (`54c9be78` -> `4bb74244`, `814dbb49` -> `38a649d5`, `f9f20edb` -> `00f711c6`) and strict scope discipline (touching only `crates/designos-doc/src/components.rs`, `crates/designos-app/src/panels/components.rs`, and `plans/reports/agy-1427-swap-overrides*`).
- **`es:rust` & `es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Replaced the `thread_local!` `LAST_DROPPED_INNER_SWAPS` state with a per-context `Ctx::dropped_inner_swaps` counter (`crates/designos-doc/src/components.rs:70,1362`) returned by `swap_main` (`:1312,1463`), `swap_instance_inner` (`:1662,1678`), and `set_instance_prop_inner` (`:1687,1702`), and avoided adding any extra node clones in `sync_pair` (`:1476,1524-1535`).
- **`es:fable-thinking` & `es:astra-work`** (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`): Used falsification-first mutation testing across all 11 reviewer mutants (`r5-mutants.log`), which caught that `probe_p5_size_rule_is_per_axis` needed a post-swap `sync_instances(&mut d, card)` step (`crates/designos-doc/src/components.rs:5443-5446`) so that `sync_pair`'s `!swapped_here` per-axis branch (`:1526-1535`) is exercised and kills `R-AXIS-SYNC`.

### 3. Round 5 Rulings, Commit Pairs & Exact `file:line` References (`fc0c7805`)

| Item / Ruling | Red Test Commit | Green Fix Commit | File, Symbol & Exact Line Range (`fc0c7805`) | Implementation Summary |
|---|---|---|---|---|
| **0. Merge `origin/main`** | — | `185370d3` | `crates/designos-doc/src/components.rs` | Merged latest `origin/main` cleanly onto `b970a463`. |
| **1. `NEW-1` (blocker: swapped nested instance keeps following its new main on later syncs)** | `54c9be78` | `4bb74244` | `crates/designos-doc/src/components.rs:1476`, `:1499-1500`, `:1524-1535` (`sync_pair`); tests at `crates/designos-doc/src/components.rs:6115-6175` and `crates/designos-app/src/panels/components.rs:566-610` | Dropped `inst_slot_size`. Tracked `let mut swapped_here = false;` (`:1476`), set `swapped_here = true;` right after `swap_main(c, i, desired)` (`:1499-1500`). On the swap sync (`if swapped_here`, `:1524-1525`), `node_ref.size = inode.size;` preserves `swap_main`'s per-axis decision (including outer-instance constraint widening). On every later sync (`else`, `:1526-1535`), keeps an axis only where `node_ref.size` was resized away from `orig_slot_comp` (`(node_ref.size.x - oc.size.x).abs() > 1e-4` / `(node_ref.size.y - oc.size.y).abs() > 1e-4`), otherwise takes `rn.size` (the new main's size). Added `r4_swapped_slot_follows_new_main_resize_prop` (`:6115`), `r4_swapped_slot_follows_new_main_resize_direct` (`:6136`), `r4_reswap_takes_new_main_size` (`:6157`), and `review_r4_app_swapped_slot_follows_new_main_height_edit` (`panels/components.rs:566`). |
| **2. `NEW-2` (Cross-type glyph outlines on carried `Size`)** | `814dbb49` | `38a649d5` | `crates/designos-doc/src/components.rs:1455-1460` (`swap_main`); test at `crates/designos-doc/src/components.rs:6249-6315` | Removed `own_geo` save/restore before `copy_overridden`. Instead, after `copy_overridden(&old, &mut nn, &fields);` (`:1456`), if `kinds_differ && fields.contains(&OverrideField::Size) && !matches!(nn.kind, NodeKind::Vector { .. } \| NodeKind::BooleanOp(_))` (`:1457`), clears `nn.fill_geometry = None; nn.stroke_geometry = None;` (`:1458-1459`). Thus `Vector ↔ BooleanOp` and `Ellipse → Vector` keep the 20 px scaled `fill_geometry`/`stroke_geometry` copied from `old` (which `apply_reference` then rescales to the new main's outline at 20 px), while `Vector → Ellipse` and `Vector → Frame` clear stale vector geometry. Added `r4_cross_kind_size_carry_matrix` (`:6249`). |
| **3. `MIN-1` (Log and return per-call dropped inner-swap count without `thread_local!`)** | `f9f20edb` | `00f711c6` | `crates/designos-doc/src/components.rs:44-51` (`mod log`), `:70` (`Ctx::dropped_inner_swaps`), `:1312,1359-1362,1463` (`swap_main`), `:1662-1679` (`swap_instance_inner`), `:1687-1716` (`set_instance_prop_inner`); tests at `:5649-5817` | Removed `thread_local! LAST_DROPPED_INNER_SWAPS` and `pub fn last_dropped_inner_swaps()`. `swap_main` logs `log::warn!("swap_main on instance {i:?} dropped {dropped_inner_swaps} inner instance swap(s)");` whenever `dropped_inner_swaps > 0` (`:1359-1361`), accumulates `c.dropped_inner_swaps += dropped_inner_swaps;` (`:1362`), and returns `dropped_inner_swaps` (`:1463`). `swap_instance_inner` and `set_instance_prop_inner` return `(Command, usize)` per call; `r3_inner_swap_below_swapped_slot_by_index` (`:5649`), `r4c_count_unswapped_nested_chain_is_zero` (`:5773`), and `r4c_count_two_inner_swaps_then_overwrite` (`:5792`) pin the returned per-call counts (`1`, `0`, and `2` then `0`). |
| **4. `MIN-2` (Rename `probe_p5_size_rule_is_per_axis`, pin post-swap sync, and correct Round 4 report)** | — | `fc0c7805` | `crates/designos-doc/src/components.rs:5394-5447` (`probe_p5_size_rule_is_per_axis`); `plans/reports/agy-1427-swap-overrides.md:295-308,394-400` | Renamed `probe_p5_size_rule_is_whole_vector` to `probe_p5_size_rule_is_per_axis` (`:5394`) and added a post-swap `sync_instances(&mut d, card)` + assertion (`:5443-5446`) so `sync_pair`'s `!swapped_here` per-axis check is exercised and kills `R-AXIS-SYNC`. Corrected the Round 4 report's `R-AXIS-SYNC` kill attribution and exact clippy commands/flags. |

### 4. Acceptance Table & Pasted Command Outputs

Durable Round 5 evidence logs are committed in `plans/reports/agy-1427-swap-overrides-evidence/`:
- `plans/reports/agy-1427-swap-overrides-evidence/r5-red.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r5-green.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r5-mutants.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r5-swap-check.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r5-redraw-bounds.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r5-redraw-syncs.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r5-gates.log`

| Acceptance Check | Command | Result | Evidence File |
|---|---|---|---|
| **1. Red/green per Round 5 item** | `cargo test -p designos-doc --locked --lib -- <test>` & `cargo test -p designos-app --bin designos --locked -- review_r4_app_swapped_slot_follows_new_main_height_edit` on each red test commit (`54c9be78`, `814dbb49`, `f9f20edb`) and green fix commit (`4bb74244`, `38a649d5`, `00f711c6`, `fc0c7805`) | **PASS** (every new test fails before its fix commit and passes on its fix commit) | `r5-red.log`, `r5-green.log` |
| **2. All 11 mutants killed** | `python3` runner applying all 11 mutants from `mutants_r4.py` (`R-AXIS-SYNC`, `R-COPYFIELD`, `R-M2`, `R-PROMO`, `N-STAMP`, `N-NO-POSTPASS`, `N-NO-RESTORE`, `N-SLOTRN`, `N-COUNT-OFF`, `N-SETNAME`, `N-LAYOUT`) against `cargo test -p designos-doc --lib --locked` | **PASS** (`11 / 11 KILLED`, source restored cleanly) | `r5-mutants.log` |
| **3. Redraw swap check, bounds & 1/3-sync render check** | `run_swap_check_r4.py`, `redraw_bounds_r4.py`, and `cargo test -p designos-render --test review_probe_1427_render_r4 --locked -- --ignored --nocapture r3_redraw_swapped_glyph_after_syncs` | **PASS** (`SWAP-OK #1427`, `WORST swapped max\|delta\| vs direct = 0.0000 px <= 0.5 px` at `sync0`, `sync1`, `sync2`, `sync3`) | `r5-swap-check.log`, `r5-redraw-bounds.log`, `r5-redraw-syncs.log` |
| **4a. `cargo fmt`** | `cargo fmt --all -- --check` | **PASS** (`exit=0`) | `r5-gates.log` |
| **4b. `cargo clippy` (no `-A` flags)** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS** (`0` warnings/errors in `designos-doc`, `designos-mcp`, or any modified line; only `origin/main`'s pre-existing errors in `agent_permissions.rs:1163,1181` and `panels/components.rs:263,363,613`) | `r5-gates.log` |
| **4c. `designos-doc` & `designos-mcp` tests** | `cargo test -p designos-doc --locked` & `cargo test -p designos-mcp --locked` | **PASS** (`designos-doc`: `703 + 3 + 5 + 2 = 713 passed, 0 failed`; `designos-mcp`: `297 unit + 200 integration = 497 passed, 0 failed`) | `r5-gates.log` |
| **4d. `designos-app --bin designos` filters** | `cargo test -p designos-app --bin designos --locked -- {component,instance,variant,librar}` | **PASS** (`component`: `116 passed`; `instance`: `97 passed`; `variant`: `19 passed`; `librar`: `179 passed, 0 failed` with sibling `HOME` & `TMPDIR`; `178 passed, 1 failed` when `HOME=$S/home` is inside `TMPDIR=$S/`) | `r5-gates.log` |
| **4e. Workspace lib tests** | `cargo test --workspace --locked --lib` | **PASS** (`1,446 passed, 0 failed` across all 12 workspace library targets) | `r5-gates.log` |

```text
=== 1. r5-red.log & r5-green.log (Red -> Green per Item) ===
# Item 1 (NEW-1) RED (54c9be78) -> GREEN (4bb74244):
test components::tests::r4_swapped_slot_follows_new_main_resize_prop ... FAILED (left: (80.0, 40.0), right: (80.0, 48.0)) -> ok
test components::tests::r4_swapped_slot_follows_new_main_resize_direct ... FAILED (left: (80.0, 40.0), right: (80.0, 48.0)) -> ok
test components::tests::r4_reswap_takes_new_main_size ... FAILED (left: (80.0, 40.0), right: (80.0, 48.0)) -> ok
test panels::components::props_panel_tests::review_r4_app_swapped_slot_follows_new_main_height_edit ... FAILED (left: 40.0, right: 48.0) -> ok
test components::tests::r3_m1_nested_slot_widened_by_outer_instance ... ok

# Item 2 (NEW-2) RED (814dbb49) -> GREEN (38a649d5):
test components::tests::r4_cross_kind_size_carry_matrix ... FAILED (["vector->boolean sync#0..3", "boolean->vector sync#0..3", "vector->ellipse sync#0..3", "vector->frame sync#0..3", "ellipse->vector sync#0..3"]) -> ok
test components::tests::probe_p2_swap_drops_override_on_type_mismatch ... ok

# Item 3 (MIN-1) RED (f9f20edb) -> GREEN (00f711c6):
test components::tests::r3_inner_swap_below_swapped_slot_by_index ... FAILED (left: 0, right: 1) -> ok
test components::tests::r4c_count_two_inner_swaps_then_overwrite ... FAILED (left: 0, right: 2) -> ok
test components::tests::r4c_count_unswapped_nested_chain_is_zero ... ok

# Item 4 (MIN-2) GREEN (fc0c7805):
test components::tests::probe_p5_size_rule_is_per_axis ... ok

=== 2. r5-mutants.log (All 11 Mutants Killed) ===
KILLED        R-AXIS-SYNC sync_pair whole-vector keep (r3 all-survivors, ported)  (15s) ['components::tests::probe_p5_size_rule_is_per_axis']
KILLED        R-COPYFIELD r3 Vector/BooleanOp guard back in shared copy_field (r3 all-survivors, inverse on head)  (16s) ['components::tests::r3_copy_field_size_on_imported_shapes_after_main_resize']
KILLED        R-M2 top-level swaps carry diff-derived overrides (r3 all-survivors)  (15s) ['components::tests::r4_top_level_swap_does_not_carry_diff_derived_overrides']
KILLED        R-PROMO Min/Min -> Scale promotion added back (mutant-R-PROMO, inverse on head)  (15s) ['components::tests::r3_m4_minmin_logo_distorts_on_per_axis_kept_width', 'components::tests::r3_m4_minmin_logo_distorts_on_user_resized_card', 'components::tests::r3_m4_minmin_glyph_swap_vs_shared_solver']
KILLED        N-STAMP constraint pass stamps Size/Transform overrides again (r3)  (16s) ['components::tests::r3_m1_nested_slot_widened_by_outer_instance', 'components::tests::probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size']
KILLED        N-NO-POSTPASS drop sync_pair's post-children constraint pass  (15s) ['components::tests::probe_p7_slot_swap_scales_inset_glyph_by_scale_not_slot_size', 'components::tests::r3_m1_nested_slot_widened_by_outer_instance', 'components::tests::r3_m4_minmin_glyph_swap_vs_shared_solver']
KILLED        N-NO-RESTORE drop the kinds_differ geometry clear  (15s) ['components::tests::probe_p2_swap_drops_override_on_type_mismatch', 'components::tests::r4_cross_kind_size_carry_matrix']
KILLED        N-SLOTRN sync_pair keep compares the MAIN's slot even on swap sync (r3 reference)  (15s) ['components::tests::r3_m1_nested_slot_widened_by_outer_instance']
KILLED        N-COUNT-OFF dropped inner swaps never counted  (16s) ['components::tests::r3_inner_swap_below_swapped_slot_by_index', 'components::tests::r4c_count_two_inner_swaps_then_overwrite']
KILLED        N-SETNAME auto-name ignores the set name  (16s) ['components::tests::r3_name_rule_variant_instance_cross_family']
KILLED        N-LAYOUT root Layout override survives a swap again (r3 M11)  (15s) ['components::tests::ruling_m1_field_exists_and_originally_matched_gate_and_mutant_guards']

=== 3. r5-swap-check.log, r5-redraw-bounds.log & r5-redraw-syncs.log ===
server: {"name": "designos-mcp", "version": "0.7.5"}
SWAP-OK #1427: 6 swapped variants keep colour, binding, opacity, visibility and the drawn glyph's bounds; SWAP_CHECK_FATAL can flip to True
WORST swapped max|delta| vs direct = 0.0000 px (limit 0.5)
PROBE redraw sync3 swapck-0-swap: slot="Icon" size=Vec2 { x: 20.0, y: 20.0 } glyph size=Vec2 { x: 20.0, y: 20.0 } at=(0,0) drawn=Some((2.4164767, 2.4164767, 17.517036, 17.501709)) direct=Some((2.4164767, 2.4164767, 17.517036, 17.501709)) max|d|=0.0000 ov={Size, Fills}
PROBE redraw sync3 swapck-1-swap: slot="Icon" size=Vec2 { x: 16.0, y: 16.0 } glyph size=Vec2 { x: 16.0, y: 16.0 } at=(0,0) drawn=Some((0.94104373, 0.9411439, 14.549071, 14.5491705)) direct=Some((0.94104373, 0.9411439, 14.549071, 14.5491705)) max|d|=0.0000 ov={Fills}
PROBE redraw sync3 swapck-2-swap: slot="Icon" size=Vec2 { x: 16.0, y: 16.0 } glyph size=Vec2 { x: 16.0, y: 16.0 } at=(0,0) drawn=Some((1.9508288, 3.9509292, 14.5491705, 13.000344)) direct=Some((1.9508288, 3.9509292, 14.5491705, 13.000344)) max|d|=0.0000 ov={Fills}
PROBE redraw sync3 swapck-3-swap: slot="Leading icon" size=Vec2 { x: 20.0, y: 20.0 } glyph size=Vec2 { x: 20.0, y: 20.0 } at=(0,0) drawn=Some((2.438536, 4.9386616, 18.186462, 16.250431)) direct=Some((2.438536, 4.9386616, 18.186462, 16.250431)) max|d|=0.0000 ov={Size, Fills}
PROBE redraw sync3 swapck-4-swap: slot="Leading icon" size=Vec2 { x: 20.0, y: 20.0 } glyph size=Vec2 { x: 20.0, y: 20.0 } at=(0,0) drawn=Some((2.5, 1.25, 17.5, 17.5)) direct=Some((2.5, 1.25, 17.5, 17.5)) max|d|=0.0000 ov={Size, Fills}
PROBE redraw sync3 swapck-5-swap: slot="Leading icon" size=Vec2 { x: 20.0, y: 20.0 } glyph size=Vec2 { x: 20.0, y: 20.0 } at=(0,0) drawn=Some((2.438536, 4.9386616, 18.186462, 16.250431)) direct=Some((2.438536, 4.9386616, 18.186462, 16.250431)) max|d|=0.0000 ov={Size, Fills}
test r3_redraw_swapped_glyph_after_syncs ... ok

=== 4. r5-gates.log (Mandatory Gates) ===
=== GATE 1: cargo fmt --all -- --check ===
cargo fmt exit: 0
=== GATE 2: cargo clippy --workspace --all-targets --locked -- -D warnings ===
error: unneeded `return` statement --> crates/designos-app/src/agent_permissions.rs:1163:9
error: unneeded `return` statement --> crates/designos-app/src/agent_permissions.rs:1181:9
error: writing `&mut Vec` instead of `&mut [_]` --> crates/designos-app/src/panels/components.rs:613:29
error: items after a test module --> crates/designos-app/src/panels/components.rs:263:1
error: unused return value of `egui::Context::run_ui` --> crates/designos-app/src/panels/components.rs:363:9
cargo clippy exit: 101
=== GATE 3a: cargo test -p designos-doc --locked ===
test result: ok. 703 passed; 0 failed (lib) + 3 + 5 + 2 passed; 0 failed
=== GATE 3b: cargo test -p designos-mcp --locked ===
test result: ok. 297 passed; 0 failed (lib) + 200 integration tests passed; 0 failed (497 total)
=== GATE 4a..4d: cargo test -p designos-app --bin designos --locked -- {component,instance,variant,librar} ===
component: 116 passed; 0 failed | instance: 97 passed; 0 failed | variant: 19 passed; 0 failed | librar: 179 passed; 0 failed (sibling HOME/TMPDIR; 178/1 when HOME sits inside TMPDIR)
=== GATE 5: cargo test --workspace --locked --lib ===
test result: ok. 1,446 passed; 0 failed across all 12 workspace lib targets
```

### 5. Round 5 Epistemic Ledger, Weakest Link & Unresolved Questions

- **`FACT`**: In Round 4 (`b970a463`), `sync_pair` compared `inst_slot_size` (the instance slot's current size on every sync) against `orig_slot_comp` (`Btn/A`, `80x32`). Once a slot was swapped to `Btn/B` (`80x40`), its height `40.0` differed from `Btn/A`'s `32.0`, so every subsequent sync treated `40.0` as a kept axis and ignored later edits to `Btn/B`'s height (`40 -> 48`) or a re-swap `Btn/B -> Btn/C` (`40 -> 48`). Tracking `swapped_here` (`crates/designos-doc/src/components.rs:1476,1524-1535`) so `node_ref.size = inode.size` is used on the swap sync while later syncs compare `node_ref.size` (the outer main's slot size) against `orig_slot_comp` fixes both follow-after-resize and re-swap while preserving outer-instance constraint widening (`r3_m1_nested_slot_widened_by_outer_instance`).
- **`FACT`**: In Round 4 (`b970a463`), restoring `own_geo` (`16x16` outline) whenever `kinds_differ` in `swap_main` caused `apply_reference` (`components.rs:1201-1207`) to preserve that unscaled `16x16` `own_geo` across `apply_constrained_geometry` when `OverrideField::Size` (`20x20`) was carried across `Vector ↔ BooleanOp` or `Ellipse → Vector`. Clearing `nn.fill_geometry` and `nn.stroke_geometry` only when `kinds_differ && fields.contains(&OverrideField::Size) && !matches!(nn.kind, NodeKind::Vector { .. } | NodeKind::BooleanOp(_))` (`components.rs:1457-1460`) ensures all 6 `r4_cross_kind_size_carry_matrix` pairs match a direct `20x20` glyph within `<= 0.5 px` at `0`, `1`, and `3` syncs.
- **`FACT`**: All 11 mutants in `r5-mutants.log` are killed with `exit=101`.
- **`INFERENCE`**: When an outer instance widens its root (e.g. `Card` widened from `100` to `300`, stretching its `Action` slot from `80` to `280` without setting `OverrideField::Size` on `Action`), `set_instance_prop` performs the swap inside `sync_pair` (`swapped_here = true`, keeping `280x40`), and any subsequent `sync_instances(&mut d, card)` or `sync_instances(&mut d, bb)` re-applies `rn.size` (`80x40` or `80x48`) to `Action` unless the outer `Card` instance's own constraint pass re-stretches `Action` or the main's slot was widened.
- **`ASSUMPTION`**: Using a crate-local `mod log` (`crates/designos-doc/src/components.rs:44-51`) that formats `WARN designos_doc::components: ...` to `stderr` satisfies `log::warn!` without modifying `crates/designos-doc/Cargo.toml` (which is outside the allowed-files list).
- **Weakest link**: If an outer instance (`ci`) is widened (`100 -> 300`, stretching its child slot `si` via `Stretch` constraint without recording `OverrideField::Size` on `si`), `si` keeps width `280` on the swap sync (`swapped_here == true`), but a later `sync_instances(&mut d, card)` will reset `si.size.x` to `rn.size.x` (`80`) because `sync_pair` for the outer `ci` does not re-run a constraint pass for `ci`'s own `OverrideField::Size` after syncing `ci`'s children.
- **Unresolved questions**: None.

Status: DONE

---

## Round 6: Test Gaps (`MINOR-2`), Warning Line & Single Channel (`MINOR-1`), Behaviour Test Names (`MINOR-3`), Report Fixes (`MINOR-4`), and PR (`#1427` DEV Round 6)

### 1. What Could NOT Be Verified (Round 6 — Stated First)

1. **Interactive desktop OS pointer click and live Figma comparison**: Verified headlessly through `App::set_instance_prop` (`crates/designos-app/src/comp_ops.rs:452`), `App::swap_instance` (`crates/designos-app/src/comp_ops.rs:462`), and `App::edit_nodes` (`crates/designos-app/src/ops.rs:244`) in `crates/designos-app/src/panels/components.rs:409,566`, and through `designos-mcp` on a scratch copy of `ff815148:design/studio-ui.designos` (`plans/reports/agy-1427-swap-overrides-evidence/r6-swap-check.log`).
2. **Pre-existing `r5_row1..6` test names in `crates/designos-doc/src/styles_ops.rs:2504..3080`**: After renaming all 28 audit-labeled test functions on this branch (`crates/designos-doc/src/components.rs` and `crates/designos-app/src/panels/components.rs`), `git grep -nE 'fn (r[0-9]+_|r4c_|probe_p[0-9]+_|review_r[0-9]+_|ruling_m[0-9]+_)' -- crates/designos-doc/src/components.rs crates/designos-app/src/panels/components.rs` prints nothing (`0` matches), while `git grep -nE 'fn (r[0-9]+_|r4c_|probe_p[0-9]+_|review_r[0-9]+_|ruling_m[0-9]+_)' -- crates` prints 6 lines in `crates/designos-doc/src/styles_ops.rs` that landed on `origin/main` in `ccdac946` (`#1424`) and are outside this task's allowed-files list (`crates/designos-doc/src/components.rs` and `crates/designos-app/src/panels/components.rs`).
3. **Pre-existing Clippy errors on `origin/main` in `designos-app`**: Running `cargo clippy --workspace --all-targets --locked -- -D warnings` (with no `-A` flags) exits `101` solely because of `origin/main`'s 5 known errors (`crates/designos-app/src/agent_permissions.rs:1163,1181` and `crates/designos-app/src/panels/components.rs:263,363,613`; `plans/reports/agy-1427-swap-overrides-evidence/r6-clippy.log`).

### 2. Skills Loaded & Changes Made Because of Them

- **`es:dev` & `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Executed the accepted Round 6 controller rulings in order (merge `origin/main` `c73b53bc` -> three individual `test(doc): ...` commits `8728c34b`, `8baa3ec5`, `cfacb45e` verified red on their review mutant and green on head -> `fix(doc): ...` commit `f26136f3` for `MINOR-1` -> `test(doc,app): ...` commit `e542545c` for `MINOR-3` -> PR creation `https://github.com/jangtrinh/design-os-studio/pull/1449`), modifying only the allowed files.
- **`es:rust` & `es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Removed the crate-local `mod log` shim and `swap_main`'s duplicate `usize` return value (`crates/designos-doc/src/components.rs:1303,1350-1353`), using the crate's `eprintln!("designos-doc: swapping instance {i:?} dropped {dropped_inner_swaps} inner instance swap(s) (#1433)");` convention and keeping a single channel (`Ctx::dropped_inner_swaps`, `:61`).
- **`es:fable-thinking` & `es:astra-work`** (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`): Applied falsification-first mutation testing (`r6-mutants.log`) to confirm each of the 3 new `MINOR-2` tests fails on its review mutant (`V-COUNT-ASSIGN`, `V-CLEAR-NO-SIZE`, `V-CLEAR-FILL-ONLY`) and passes on head, and that all 11 Round 5 mutants remain killed (`14 / 14 KILLED`).

### 3. Round 6 Commits & Exact `file:line` References (`e542545c`)

| Step / Ruling | Commit | File, Symbol & Exact Line Range (`e542545c`) | Summary |
|---|---|---|---|
| **0. Merge `origin/main`** | `c73b53bc` | `scripts/tests/test_check_ds_ontology_parity.py` | Merged `origin/main` (`53d571c7`) into `c76004ca` with a merge commit. |
| **1a. `MINOR-2` (`V-COUNT-ASSIGN`)** | `8728c34b` | `crates/designos-doc/src/components.rs:5814-5863` (`property_swap_accumulates_dropped_inner_swaps_across_bound_slots`) | Pins one property bound to two slots swapped in one call returning `(cmd, 2)` from `set_instance_prop_inner` (fails on `V-COUNT-ASSIGN` with `left: (1, 0, 1, 0), right: (1, 0, 2, 0)`, passes on head). |
| **1b. `MINOR-2` (`V-CLEAR-NO-SIZE`)** | `8baa3ec5` | `crates/designos-doc/src/components.rs:6360-6422` (`direct_cross_type_swap_with_fills_only_keeps_imported_outline_before_sync`) | Pins a `Fills`-only cross-type swap on the direct `swap_instance` path (`Vector -> Frame` and `Vector -> Ellipse` with an imported outline) keeping the new main's imported outline right at the swap before any sync (fails on `V-CLEAR-NO-SIZE` with `left: None, right: Some((3.0, 2.0, 13.0, 14.0))`, passes on head). |
| **1c. `MINOR-2` (`V-CLEAR-FILL-ONLY`)** | `cfacb45e` | `crates/designos-doc/src/components.rs:6425-6451` (`cross_type_swap_with_carried_size_clears_stroke_geometry_on_frame_glyph`) | Pins a stroked cross-type cell (`Vector -> Frame` with an imported `stroke_geometry`) clearing `stroke_geometry` to `None` at the swap and after 1 sync (fails on `V-CLEAR-FILL-ONLY`, passes on head). |
| **2. `MINOR-1` (Warning line & single channel)** | `f26136f3` | `crates/designos-doc/src/components.rs:61` (`Ctx::dropped_inner_swaps`), `:1303,1350-1353` (`swap_main`), `:1652-1669` (`swap_instance_inner`), `:1677-1706` (`set_instance_prop_inner`) | Deleted `mod log`. `swap_main` now returns `()`, writes `eprintln!("designos-doc: swapping instance {i:?} dropped {dropped_inner_swaps} inner instance swap(s) (#1433)");` when `dropped_inner_swaps > 0`, and adds to `Ctx::dropped_inner_swaps`. `swap_instance_inner` and `set_instance_prop_inner` read and return the per-call count from `Ctx`. |
| **3. `MINOR-3` (Behaviour test names & comment cleanup)** | `e542545c` | `crates/designos-doc/src/components.rs:4453..6295` (27 test renames) and `crates/designos-app/src/panels/components.rs:566` (`swapped_slot_follows_new_main_height_edit_in_app`) | Renamed all 28 audit-labeled tests on this branch to behaviour names and removed `(R-M2)` and all review/mutant finding codes from comments and assertion messages. |
| **4. `MINOR-4` (Round 5 report fixes & PR)** | (this commit) | `plans/reports/agy-1427-swap-overrides.md` | Corrected the Round 5 App entry-point citations (`comp_ops.rs:452`, `comp_ops.rs:462`, `ops.rs:244`), the three doc line citations (`:70`, `:1662`, `:1687`), the `designos-mcp` test count (`497`), and the `librar` harness count (`179 passed, 0 failed` with sibling `HOME` and `TMPDIR`), and opened PR [#1449](https://github.com/jangtrinh/design-os-studio/pull/1449). |

### 4. Acceptance Table & Pasted Command Outputs

Durable Round 6 evidence logs are committed in `plans/reports/agy-1427-swap-overrides-evidence/`:
- `plans/reports/agy-1427-swap-overrides-evidence/r6-mutants.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r6-clippy.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r6-gates.log`
- `plans/reports/agy-1427-swap-overrides-evidence/r6-swap-check.log`

| Acceptance Check | Command | Result | Evidence File |
|---|---|---|---|
| **1a. Three new `MINOR-2` tests red on mutant, green on head** | `python3` harness applying `V-COUNT-ASSIGN`, `V-CLEAR-NO-SIZE`, and `V-CLEAR-FILL-ONLY` to `crates/designos-doc/src/components.rs` | **PASS** (all 3 fail with `exit=101` on their review mutant and pass with `exit=0` on head) | `r6-mutants.log` |
| **1b. Round 5's 11 mutants + 3 review mutants** | `cargo test -p designos-doc --lib --locked` against all 14 mutants | **PASS** (`14 / 14 KILLED`) | `r6-mutants.log` |
| **2a. `cargo fmt`** | `cargo fmt --all -- --check` | **PASS** (`exit=0`) | `r6-gates.log` |
| **2b. `cargo clippy` (no `-A` flags)** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS** (only `origin/main`'s 5 known errors in `agent_permissions.rs:1163,1181` and `panels/components.rs:263,363,613`) | `r6-clippy.log`, `r6-gates.log` |
| **2c. `designos-doc` & `designos-mcp` tests** | `cargo test -p designos-doc --locked` & `cargo test -p designos-mcp --locked` | **PASS** (`designos-doc`: `706 + 3 + 5 + 2 = 716 passed, 0 failed`; `designos-mcp`: `497 passed, 0 failed`) | `r6-gates.log` |
| **2d. `designos-app --bin designos` filters** | `cargo test -p designos-app --bin designos --locked {component,instance,variant,librar}` | **PASS** (`component`: `116 passed`; `instance`: `97 passed, 1 ignored`; `variant`: `19 passed`; `librar` with sibling `HOME` & `TMPDIR`: `179 passed, 0 failed`) | `r6-gates.log` |
| **2e. Workspace lib tests** | `cargo test --workspace --locked --lib` | **PASS** (`1,449 passed, 0 failed, 3 ignored`) | `r6-gates.log` |
| **3. `swap_check` on scratch copy of `ff815148`** | `git show ff815148:design/studio-ui.designos` + head `designos-mcp` | **PASS** (`SWAP-OK #1427`, `sha1=8c39a339c7cda89e9fd0a7cecc9bc84a6c37c991` unchanged) | `r6-swap-check.log` |
| **4. Open PR against `main`** | `gh pr create --base main --head jangtrinh/agy-1427-swap-overrides ...` | **PASS** ([`https://github.com/jangtrinh/design-os-studio/pull/1449`](https://github.com/jangtrinh/design-os-studio/pull/1449)) | — |

```text
=== 1. r6-mutants.log (3 New Tests Red on Review Mutant -> Green on Head + All 14 Mutants Killed) ===
=== PART 1: The Three New Tests (Red on Review Mutant -> Green on Head) ===

--- V-COUNT-ASSIGN :: property_swap_accumulates_dropped_inner_swaps_across_bound_slots ---
[mutant applied] exit=101
  running 1 test
  test components::tests::property_swap_accumulates_dropped_inner_swaps_across_bound_slots ... FAILED
  thread 'components::tests::property_swap_accumulates_dropped_inner_swaps_across_bound_slots' (45232889) panicked at crates/designos-doc/src/components.rs:5862:9:
  assertion `left == right` failed
    left: (1, 0, 1, 0)
   right: (1, 0, 2, 0)
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 705 filtered out; finished in 0.01s
[head] exit=0
  running 1 test
  test components::tests::property_swap_accumulates_dropped_inner_swaps_across_bound_slots ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 705 filtered out; finished in 0.01s

--- V-CLEAR-NO-SIZE :: direct_cross_type_swap_with_fills_only_keeps_imported_outline_before_sync ---
[mutant applied] exit=101
  running 1 test
  test components::tests::direct_cross_type_swap_with_fills_only_keeps_imported_outline_before_sync ... FAILED
  thread 'components::tests::direct_cross_type_swap_with_fills_only_keeps_imported_outline_before_sync' (45234755) panicked at crates/designos-doc/src/components.rs:6406:13:
  assertion `left == right` failed: vector->frame Fills-only direct swap must keep new main's imported outline before sync
    left: None
   right: Some((3.0, 2.0, 13.0, 14.0))
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 705 filtered out; finished in 0.01s
[head] exit=0
  running 1 test
  test components::tests::direct_cross_type_swap_with_fills_only_keeps_imported_outline_before_sync ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 705 filtered out; finished in 0.01s

--- V-CLEAR-FILL-ONLY :: cross_type_swap_with_carried_size_clears_stroke_geometry_on_frame_glyph ---
[mutant applied] exit=101
  running 1 test
  test components::tests::cross_type_swap_with_carried_size_clears_stroke_geometry_on_frame_glyph ... FAILED
  thread 'components::tests::cross_type_swap_with_carried_size_clears_stroke_geometry_on_frame_glyph' (45236387) panicked at crates/designos-doc/src/components.rs:6443:9:
  assertion `left == right` failed: vector->frame with carried Size must clear stroke_geometry at swap
    left: Some([Path { cmds: [MoveTo(Vec2 { x: 0.0, y: 0.0 }), LineTo(Vec2 { x: 20.0, y: 0.0 }), LineTo(Vec2 { x: 20.0, y: 20.0 }), LineTo(Vec2 { x: 0.0, y: 20.0 }), Close], fill_rule: NonZero }])
   right: None
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 705 filtered out; finished in 0.01s
[head] exit=0
  running 1 test
  test components::tests::cross_type_swap_with_carried_size_clears_stroke_geometry_on_frame_glyph ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 705 filtered out; finished in 0.01s

=== PART 2: All 11 Round-5 Worker Mutants + 3 MINOR-2 Review Mutants ===
KILLED        R-AXIS-SYNC sync_pair whole-vector keep on later syncs  (14.9s) ['components::tests::nested_slot_size_rule_is_per_axis_across_swap_and_sync']
KILLED        R-COPYFIELD r3 Vector/BooleanOp guard back in shared copy_field  (15.3s) ['components::tests::copy_field_size_on_imported_shapes_after_main_resize']
KILLED        R-M2 top-level swaps carry diff-derived overrides  (16.5s) ['components::tests::top_level_swap_does_not_carry_diff_derived_overrides']
KILLED        R-PROMO Min/Min -> Scale promotion added back  (16.1s) ['components::tests::pinned_logo_keeps_size_on_user_resized_card_after_swap_and_sync', 'components::tests::pinned_logo_keeps_size_when_outer_width_is_kept_per_axis', 'components::tests::pinned_and_scale_glyphs_follow_constraint_solver_on_slot_swap']
KILLED        N-STAMP constraint pass stamps Size/Transform overrides again  (15.0s) ['components::tests::nested_slot_widened_by_outer_instance_keeps_width_on_swap', 'components::tests::slot_swap_scales_inset_glyph_by_scale_not_slot_size']
KILLED        N-NO-POSTPASS drop sync_pair's post-children constraint pass  (15.2s) ['components::tests::nested_slot_widened_by_outer_instance_keeps_width_on_swap', 'components::tests::pinned_and_scale_glyphs_follow_constraint_solver_on_slot_swap', 'components::tests::slot_swap_scales_inset_glyph_by_scale_not_slot_size']
KILLED        N-NO-RESTORE drop the kinds_differ geometry clear  (14.6s) ['components::tests::cross_type_swap_with_carried_size_clears_stroke_geometry_on_frame_glyph', 'components::tests::swap_drops_override_when_old_and_new_mains_differ', 'components::tests::glyph_type_change_with_carried_size_draws_new_outline']
KILLED        N-SLOTRN swap sync uses the MAIN's slot reference too (r3)  (14.8s) ['components::tests::nested_slot_widened_by_outer_instance_keeps_width_on_swap']
KILLED        N-COUNT-OFF dropped inner swaps never counted  (15.7s) ['components::tests::inner_swap_below_swapped_slot_is_dropped_and_counted', 'components::tests::swap_reports_dropped_inner_swaps_per_call', 'components::tests::property_swap_accumulates_dropped_inner_swaps_across_bound_slots']
KILLED        N-SETNAME auto-name ignores the set name  (16.2s) ['components::tests::set_named_variant_instance_takes_new_main_name_on_cross_family_swap']
KILLED        N-LAYOUT root Layout override survives a swap again  (15.8s) ['components::tests::swap_transfers_matching_overrides_across_node_types_and_preserves_slot_bindings']
KILLED        V-COUNT-ASSIGN per-call count overwritten instead of accumulated  (15.7s) ['components::tests::property_swap_accumulates_dropped_inner_swaps_across_bound_slots']
KILLED        V-CLEAR-NO-SIZE clear geometry on any cross-kind non-vector target, Size carried or not  (14.9s) ['components::tests::direct_cross_type_swap_with_fills_only_keeps_imported_outline_before_sync']
KILLED        V-CLEAR-FILL-ONLY stroke_geometry kept on the cleared target  (15.5s) ['components::tests::cross_type_swap_with_carried_size_clears_stroke_geometry_on_frame_glyph']

=== 2. MINOR-3 Test Name Audit Grep ===
$ git grep -nE 'fn (r[0-9]+_|r4c_|probe_p[0-9]+_|review_r[0-9]+_|ruling_m[0-9]+_)' -- crates/designos-doc/src/components.rs crates/designos-app/src/panels/components.rs
(prints nothing)

$ git grep -nE 'fn (r[0-9]+_|r4c_|probe_p[0-9]+_|review_r[0-9]+_|ruling_m[0-9]+_)' -- crates
crates/designos-doc/src/styles_ops.rs:2504:    fn r5_row1_three_modes_mixed_override_uses_fallback_preserves_delete_time_mode_and_reports_pair() {
crates/designos-doc/src/styles_ops.rs:2645:    fn r5_row2_three_modes_same_style_opacity_in_every_style_won_mode_is_exact_and_reports_nothing() {
crates/designos-doc/src/styles_ops.rs:2747:    fn r5_row3_style_variable_in_second_collection_brand_and_own_in_theme() {
crates/designos-doc/src/styles_ops.rs:2892:    fn r5_row4_style_font_weight_missing_in_light_with_own_font_style_italic_and_bold_italic() {
crates/designos-doc/src/styles_ops.rs:2963:    fn r5_row5_value_stored_only_under_light_plus_compact_key_classified_with_resolve_variable() {
crates/designos-doc/src/styles_ops.rs:3080:    fn r5_row6_mixed_stop_keys_keep_binding_and_match_every_mode() {

=== 3. r6-gates.log & grep -E '^(error|warning)' r6-clippy.log ===
=== GATE 1: cargo fmt --all -- --check ===
fmt_rc=0
=== GATE 2: cargo clippy --workspace --all-targets --locked -- -D warnings ===
clippy_rc=101
--- grep -E '^(error|warning)' r6-clippy.log ---
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1427-swap-overrides/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1427-swap-overrides/crates/designos-app/src/main.rs` found to be present in multiple build targets:
error: unneeded `return` statement
error: unneeded `return` statement
error: writing `&mut Vec` instead of `&mut [_]` involves a new object where a slice will do
error: could not compile `designos-app` (bin "designos-studio") due to 3 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `designos-app` (bin "designos") due to 3 previous errors
error: items after a test module
error: unused return value of `egui::Context::run_ui` that must be used
error: could not compile `designos-app` (bin "designos-studio" test) due to 5 previous errors
error: could not compile `designos-app` (bin "designos" test) due to 5 previous errors
=== GATE 3a: cargo test -p designos-doc --locked ===
test result: ok. 706 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.46s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
=== GATE 3b: cargo test -p designos-mcp --locked ===
297 + 4 + 28 + 11 + 1 + 16 + 11 + 4 + 3 + 3 + 22 + 1 + 14 + 9 + 42 + 8 + 5 + 1 + 7 + 6 + 4 = 497 passed; 0 failed
=== GATE 4a: cargo test -p designos-app --bin designos --locked component ===
test result: ok. 116 passed; 0 failed; 0 ignored; 0 measured; 3848 filtered out; finished in 17.06s
=== GATE 4b: cargo test -p designos-app --bin designos --locked instance ===
test result: ok. 97 passed; 0 failed; 1 ignored; 0 measured; 3866 filtered out; finished in 8.55s
=== GATE 4c: cargo test -p designos-app --bin designos --locked variant ===
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 3945 filtered out; finished in 1.34s
=== GATE 4d (sibling HOME & TMPDIR): cargo test -p designos-app --bin designos --locked librar ===
test result: ok. 179 passed; 0 failed; 0 ignored; 0 measured; 3785 filtered out; finished in 21.99s
=== GATE 5: cargo test --workspace --locked --lib ===
13 + 706 + 1 + 153 + 143 + 30 + 4 + 37 + 297 + 56 + 1 + 8 = 1,449 passed; 0 failed; 3 ignored

=== 4. r6-swap-check.log ===
server: {"name": "designos-mcp", "version": "0.7.5"}
studio-ui.designos sha1 before=8c39a339c7cda89e9fd0a7cecc9bc84a6c37c991 after=8c39a339c7cda89e9fd0a7cecc9bc84a6c37c991 unchanged=True
SWAP-OK #1427: 6 swapped variants keep colour, binding, opacity, visibility and the drawn glyph's bounds; SWAP_CHECK_FATAL can flip to True

=== 5. Pull Request URL ===
https://github.com/jangtrinh/design-os-studio/pull/1449
```

### 5. Round 6 Epistemic Ledger, Assumptions, Weakest Link & Unresolved Questions

- **`FACT`**: Each of the 3 new `MINOR-2` tests (`property_swap_accumulates_dropped_inner_swaps_across_bound_slots`, `direct_cross_type_swap_with_fills_only_keeps_imported_outline_before_sync`, `cross_type_swap_with_carried_size_clears_stroke_geometry_on_frame_glyph`) fails with `exit=101` when its review mutant (`V-COUNT-ASSIGN`, `V-CLEAR-NO-SIZE`, `V-CLEAR-FILL-ONLY`) is applied and passes with `exit=0` on head (`r6-mutants.log`).
- **`FACT`**: All 11 Round 5 worker mutants plus the 3 `MINOR-2` review mutants are killed (`14 / 14 KILLED` in `r6-mutants.log`).
- **`FACT`**: `mod log` is deleted from `crates/designos-doc/src/components.rs`, `swap_main` returns `()` and writes `eprintln!("designos-doc: swapping instance {i:?} dropped {dropped_inner_swaps} inner instance swap(s) (#1433)");`, and `Ctx::dropped_inner_swaps` is the single channel read by `swap_instance_inner` and `set_instance_prop_inner`.
- **`FACT`**: `swap_check` prints `SWAP-OK #1427` on a scratch copy of `ff815148:design/studio-ui.designos` (`sha1=8c39a339c7cda89e9fd0a7cecc9bc84a6c37c991` unchanged), and PR [#1449](https://github.com/jangtrinh/design-os-studio/pull/1449) is open against `main`.
- **`ASSUMPTION`**: The 6 `fn r5_row1..6_*` functions in `crates/designos-doc/src/styles_ops.rs:2504..3080` (which entered `origin/main` in `ccdac946`, PR `#1424`) must not be renamed here because `MINOR-3` specifies "Rename every test fn on the branch that matches..." and `## Files you may modify` restricts edits to `crates/designos-doc/src/components.rs` and `crates/designos-app/src/panels/components.rs`.
- **Weakest link**: Pre-existing `#1445` (when an outer instance stretches a nested slot via `Stretch` constraints without an explicit `Size` override on the slot, a subsequent `sync_instances` on the outer main resets the slot's width back to the main's width) remains tracked as a follow-up outside this PR's scope.
- **Unresolved questions**: None.

Status: DONE
