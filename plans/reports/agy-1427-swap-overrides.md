# Fix #1427: Preserve Swapped Slot Child Overrides, Variable Bindings, Visibility, Name, and Resized Slot/Glyph Geometry

## 1. What Could NOT Be Verified (Stated First)

1. **Interactive desktop GUI pointer click on the Properties panel combo box**: Verified headlessly in `crates/designos-app/src/panels/components.rs` (`properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo`) by calling the exact `App` mutation methods that the Properties panel combo boxes invoke (`app.set_instance_prop(inst_id, "Icon", Some(PropValue::Instance(icon_check)))` at `crates/designos-app/src/panels/components.rs:584` and `app.swap_instance(slot_id, icon_plus)` at `crates/designos-app/src/panels/components.rs:80`), plus end-to-end MCP subprocess verification via `swap_probe.py` and `swap_vis.py` against `studio-ui.designos`. Interactive OS window click injection was not run.
2. **Pre-existing `clippy::needless_return` warnings in untouched `crates/designos-app/src/agent_permissions.rs` on `origin/main`**: Running `cargo clippy --workspace --all-targets --locked -- -D warnings` flags two pre-existing `clippy::needless_return` lints at `crates/designos-app/src/agent_permissions.rs:1163` and `:1181` (untouched out-of-scope file). Passing `-A clippy::needless_return` (or running `cargo clippy -p designos-doc -p designos-mcp --all-targets --locked -- -D warnings` on touched crates) exits `0` with zero warnings.
3. **Isolated `$HOME` harness missing `$HOME/Library/Caches` on macOS for `agent_permissions` / `ask_mode_tests`**: In Gate 5 (`cargo test -p designos-app --bin designos --locked`), when `HOME="$S/home"` is freshly created without `$S/home/Library/Caches`, 23 tests in `agent_permissions::tests`, `agent_turn_ledger::tests`, and `terminal::ask_mode_tests` fail with `NotFound: "no private auto-allow log"` because `crates/designos-app/src/agent_permissions.rs:1181` calls `tempfile::TempDir::new_in(PathBuf::from(HOME).join("Library/Caches"))` on macOS without `create_dir_all`. Pre-creating `$S/home/Library/Caches` (or running outside an empty `$HOME`) resolves those 23 unrelated failures; the known flake `terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled` (`#996`/`#1179`/`#1211`) passed when rerun alone (`test result: ok. 1 passed; 0 failed`).

---

## 2. Shared Swap Path & Caller Inventory

All instance swaps in DESIGN:OS Studio converge on `crates/designos-doc/src/components.rs`:

| Surface / Crate | Entry Point | Call Chain into Shared Swap Engine | Notes |
|---|---|---|---|
| **`designos-doc`** (core scene graph) | `swap_instance(doc, instance_id, new_component_id)` (`crates/designos-doc/src/components.rs:285`) | `swap_instance` -> `swap_main(d, id, new_comp)` -> `sync_one(d, id)` -> `sync_pair` | Direct swap of a top-level or nested `NodeKind::Instance`. |
| **`designos-doc`** (component props & variant sync) | `set_instance_prop(doc, instance_id, prop_name, value)` (`crates/designos-doc/src/components.rs:412`) & `sync_instances(doc)` (`:192`) | `set_instance_prop` -> `sync_one(d, instance_id)` -> `sync_pair(d, ...)` -> detects `PropValue::Instance(sc)` on `ComponentPropRef::MainComponent` -> `swap_main(d, inst_child, sc)` -> `apply_reference` -> recurses into children | Evaluates `InstanceSwap` property bindings (`"Icon": "Icon/Check"`) and variant switches on nested slots. |
| **`designos-mcp`** (`designos_apply_spec` / `designos_apply_batch`) | `spec.props` (`crates/designos-doc/src/spec_instance.rs:1066`, `apply_instance_props_with_ctx`) | `set_instance_prop(doc, instance_id, &canonical_name, Some(PropValue::Instance(target_comp_id)))` | Previously had a post-hoc `saved_slot_names_and_sizes` workaround at `spec_instance.rs:1054-1091` that only restored root slot `name`/`size` and missed child overrides, variable bindings, visibility, and `spec.overrides[].swap`. Removed now that `components.rs` preserves them at the document layer. |
| **`designos-mcp`** (`designos_apply_spec` / `designos_apply_batch`) | `spec.overrides[].swap` (`crates/designos-doc/src/spec_instance.rs:1212`, `apply_single_override_with_ctx`) | `swap_instance(doc, target_id, new_comp)` | Shares the exact same `swap_main` + `sync_one` path. |
| **`designos-app`** (Properties panel) | `instance_header` (`crates/designos-app/src/panels/components.rs:80`) & `instance_props` (`:584`) | `app.swap_instance(id, comps[i])` (`app/components.rs:424`) and `app.set_instance_prop(inst, &def.name, Some(PropValue::Instance(comps[i])))` (`app/components.rs:509`) | Both snapshot undo via `self.Modify(...)` and call `designos_doc::swap_instance` / `designos_doc::set_instance_prop`. |
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
- **`INFERENCE`**: Removing the partial `saved_slot_names_and_sizes` workaround in `crates/designos-doc/src/spec_instance.rs` has no regressions because `swap_main` and `sync_pair` now preserve slot `name` and `size` for all callers (verified by all 448 tests in `designos-mcp`).
- **`ASSUMPTION`**: Matching child overrides by `(child.name, discriminant(child.kind))` across swapped components matches Figma's layer-name + node-type preservation rule for icon/component swaps.
- **Weakest link**: Child override transfer currently matches immediate children of the swapped instance by `(name, kind discriminant)` rather than recursive multi-level descendant paths inside the swapped slot.
- **Unresolved questions**: None.

Status: DONE
