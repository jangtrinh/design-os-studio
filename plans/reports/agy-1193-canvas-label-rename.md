# DEV Report · #1193 (P1, v0.6.4): Canvas Frame Label Keeps Old Name After Agent Renames a Component

## 1. What Could NOT Be Verified (Stated First)

1. **Live interactive external agent CLI (`claude`, `codex`, `gemini`, `grok`, `agy`):** Forbidden by Controller Ruling 5 (*"Never run a real agent CLI. Every test runs in-process or with stubs"*). Instead, the exact `designos-mcp` tool execution and live-session file bridge were exercised in-process via `designos_mcp::server::DesignOSServer::call_tool_raw("designos_update_node", ...)` + `LiveSessionBridge::for_session_dir` writing `doc-patch.jsonl` + `live_patch_apply::apply_live_patches_with_offsets` + `Sim::frame()` egui paint inspection.
2. **Mid-stream `doc-patch.jsonl` arrival during `designos --screenshot`:** `LivePatchLog::bind` (`crates/designos-app/src/live_patch_log.rs:46-48`) initializes `offset` to the current file length (`std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)`) when binding a session directory on startup, and `--screenshot` exits immediately after rendering its initial capture frames. Therefore, the 1440×900 visual evidence (`plans/reports/agy-1193-canvas-label-rename-evidence/canvas-label-rename-1440.png`) was captured by running the real `designos-mcp` binary (`designos_open_file` -> `designos_update_node` on the inner `Rectangle` node -> `designos_save_file`) and rendering the resulting `.designos` document with `designos --screenshot --size 1440x900 --select Card --no-toast`, while the live `doc-patch.jsonl` tailing path (`poll_with_offsets` -> `apply_live_patches_with_offsets` -> `canvas::draw::overlays`) is verified in-process by the 7 `shell::tests::canvas_label_rename` tests.

---

## 2. Conclusion & Epistemic Ledger

### Conclusion

When a user draws a non-frame shape (`Rectangle 1`) and presses `Ctrl+Alt+K` (`Document::create_component`), DESIGN:OS Studio wraps the shape (`rect_id`) in a new top-level `NodeKind::Component` node (`comp_id`) with the same name (`"Rectangle 1"`), leaving **two distinct nodes** representing the same single-shape component. Because canvas hit-testing (`selects_through(&NodeKind::Component(_)) == true`) selects the inner shape (`rect_id`) while the canvas frame label (`canvas::draw::overlays`) only paints top-level `page.children` (`comp_id`), renaming `rect_id` (via agent `designos_update_node` or user rename) updated `rect_id.name` to `"Card"` (displayed in Properties, the Agents tab selection chip, and `rect_id`'s Layers row) while `comp_id.name` remained `"Rectangle 1"` on the canvas frame label indefinitely. In addition, renaming a main `Component` (or `ComponentSet`) did not update un-overridden `Instance` node names because `apply_reference` skips `OverrideField::Name` on `top` and `NodeKind::Instance`.

We fixed both at the command generation root (`crates/designos-doc/src/writer_command.rs`, `crates/designos-doc/src/live_patch/replay.rs`, `crates/designos-app/src/comp_ops.rs`, and `crates/designos-app/src/agent_turn_ledger.rs`) by computing companion `Command::Update` mutations via `companion_component_rename_updates` whenever a single-shape component wrapper, its sole matching inner shape, or a main `Component` / `ComponentSet` is renamed, and recording `OverrideField::Name` when an `Instance` is directly renamed.

### Epistemic Ledger

| Tag | Statement | Evidence |
|---|---|---|
| **`FACT`** | `canvas::draw::overlays` reads `n.name` live from `doc.get(id)` on each painted frame (`crates/designos-app/src/canvas/draw.rs:816-818`), and `label_rects()` (`draw.rs:101-171`) only caches `HashMap<NodeId, Rect>` keyed on `(doc.runtime_id(), page, doc.mutation_epoch(), zoom, offset, rect)` for top-level `page.children` (`draw.rs:136-142, 802`). | Direct source inspection + `repro_qa_1193_rectangle_ctrl_alt_k_agent_rename_updates_canvas_frame_label` (`red.log` vs `green.log`). |
| **`FACT`** | `LiveOp::Update` in `crates/designos-doc/src/live_patch/replay.rs:173-216` builds `Command::Update` via `writer_command` (`crates/designos-doc/src/writer_command.rs:33-38`) and executes `doc.get_mut(node.id)` (`crates/designos-doc/src/history.rs:133`), which bumps `mutation_epoch` (`crates/designos-doc/src/lib.rs:887-889, 1007-1009`). Thus the hypothesis that `LiveOp::Update` bypassed `get_mut` was **falsified**. | Direct source inspection + `red.log`. |
| **`FACT`** | `Document::create_component` (`crates/designos-doc/src/components.rs:546-578`) converts `NodeKind::Frame` in-place (`line 550`), but for any other shape (`_ =>` at `lines 559-577`, including `NodeKind::Rectangle`) allocates a new wrapper node `wid` (`NodeKind::Component(FrameProps::default())`) with `w.name = n.name.clone()` (`"Rectangle 1"`) and reparents the original `Rectangle` (`id`, still named `"Rectangle 1"`) as child `0`. | `crates/designos-doc/src/components.rs:546-578`. |
| **`FACT`** | `canvas::hit::selects_through` (`crates/designos-app/src/canvas/hit.rs:25-27, 64-85`) returns `true` for `NodeKind::Component(_)`, so clicking the component on the canvas selects the inner `Rectangle` (`rect_id`), which is then published in `context.json` (`selected_ids`) and shown in Properties (`crates/designos-app/src/panels/properties.rs:773, 799`) and the Agents selection chip (`crates/designos-app/src/agent_selection_chip.rs:68`). | `crates/designos-app/src/canvas/hit.rs:25-27, 64-85`. |
| **`FACT`** | `apply_reference` (`crates/designos-doc/src/components.rs:1129-1134`) explicitly skips `OverrideField::Name` when `dst_id == top || matches!(d.kind, NodeKind::Instance(_))`, so renaming a main `Component` or `ComponentSet` never propagated to un-overridden `Instance` nodes. | `crates/designos-doc/src/components.rs:1129-1134` + `renaming_main_component_updates_instance_canvas_labels_and_preserves_overridden_instance_names`. |
| **`INFERENCE`** | Paired wrapper/inner-shape rename sync should only fire when a `NodeKind::Component` has **exactly one child**, that child is a **leaf non-frame shape** (`!matches!(kind, Frame \| Component \| ComponentSet \| Instance \| Section \| Group \| Slice) && children.is_empty()`), and both nodes currently share `old_name`, preventing unintended child renames inside multi-layer or frame-based components. | Verified across all 633 `designos-doc` unit tests, 245 `designos-mcp` tests, and 7 `canvas_label_rename` tests. |
| **`ASSUMPTION`** | When an `Instance` is renamed directly (by user or agent) to a name different from its current name, that rename is an intentional instance-level name override (`OverrideField::Name`) and should be preserved across subsequent main-component renames unless undone/reverted. | Locked by `renaming_main_component_updates_instance_canvas_labels_and_preserves_overridden_instance_names`. |

- **Weakest link (one line):** Companion wrapper/child sync relies on the wrapper and sole leaf child sharing `old_name` at the start of the rename step (`companion_component_rename_updates`), which always holds after `create_component` (`Ctrl+Alt+K`) and stays invariant once every rename path syncs both nodes together.

---

## 3. Drift Table (Code Map vs `HEAD` at `origin/release/v0.6.3` `eac72061a`)

| Code Map Reference (`scout-261026-v064-p1-code-map.md` §A) | Actual Location on `HEAD` (`eac72061a`) | Status |
|---|---|---|
| `crates/designos-app/src/canvas/draw.rs:816-818` (`let text = truncate_label(&n.name, 28);` in `overlays()`) | `crates/designos-app/src/canvas/draw.rs:816-818` | Exact match |
| `crates/designos-app/src/canvas/draw.rs:101-171` (`label_rects()` cache keyed on `(doc.runtime_id(), page, doc.mutation_epoch(), zoom, offset.x, offset.y, rect)`) | `crates/designos-app/src/canvas/draw.rs:101-171` | Exact match |
| `crates/designos-doc/src/lib.rs:887-889` (`mark_node_dirty` increments `dirty.mutation_epoch`), `1007-1009` (`get_mut`), `998-1032` (`insert`/`remove`/`reparent`), `1064` (`set_in_progress`) | `crates/designos-doc/src/lib.rs:887-889, 998-1032, 1007-1009, 1064` | Exact match |
| `crates/designos-app/src/terminal.rs:3367-3375` (`live_patch.poll_with_offsets()`) and `crates/designos-app/src/live_patch_apply.rs:180-242` (`apply_live_patches_with_offsets`) | `crates/designos-app/src/terminal.rs:3367-3375`, `crates/designos-app/src/live_patch_apply.rs:180-242` | Exact match |
| `crates/designos-app/src/live_log_wake.rs:16` (`500 ms` poll interval) and `crates/designos-app/src/shell.rs:1508` | `crates/designos-app/src/live_log_wake.rs:16`, `crates/designos-app/src/shell.rs:1508` | Exact match |
| `crates/designos-doc/src/live_patch/replay.rs:173-216` (`LiveOp::Update` replay path) | `crates/designos-doc/src/live_patch/replay.rs:173-216` (calls `writer_command` in `crates/designos-doc/src/writer_command.rs:33-38`) | Confirmed: `LiveOp::Update` goes through `Command::Update` -> `doc.get_mut`, falsifying the missing-`get_mut` hypothesis and confirming the split-node target hypothesis |

---

## 4. Root Cause (`file:line` Evidence)

1. **Split identity created by `Ctrl+Alt+K` (`Document::create_component`) on non-frame shapes (`crates/designos-doc/src/components.rs:546-578`):**
   - When `Document::create_component(&mut self, id: NodeId)` is called on a `NodeKind::Rectangle` (`"Rectangle 1"`), the `_ =>` arm (`crates/designos-doc/src/components.rs:559-577`) allocates a new parent node `wid` of kind `NodeKind::Component(FrameProps::default())` with `w.name = n.name.clone()` (`"Rectangle 1"`) and reparents the original `Rectangle` (`id`, which keeps `inner.name = "Rectangle 1"`) as child `0` of `wid`.
   - When the user clicks the component on the canvas, `canvas::hit::selects_through` (`crates/designos-app/src/canvas/hit.rs:25-27, 64-85`) treats `NodeKind::Component(_)` as a container to select through, selecting the inner `Rectangle` (`rect_id`), or when the agent inspects/updates the selected node (`rect_id`) or the component (`comp_id`), only one of the two nodes was updated.
   - `canvas::draw::overlays` (`crates/designos-app/src/canvas/draw.rs:136-142, 802, 816-818`) iterates only over `p.children` (top-level nodes on the page, i.e., `comp_id`) and paints `n.name` (`comp_id.name`) as the canvas frame label. Meanwhile, Properties (`crates/designos-app/src/panels/properties.rs:773, 799`) and the Agents tab selection chip (`crates/designos-app/src/agent_selection_chip.rs:68`) display the selected `rect_id.name` (`"Card"`), leaving the canvas frame label stuck on `"Rectangle 1"` forever.
2. **Main component rename not propagating to `Instance` labels (`crates/designos-doc/src/components.rs:1129-1134`):**
   - `apply_reference` (`crates/designos-doc/src/components.rs:1129-1134`) explicitly skips `OverrideField::Name` when `dst_id == top || matches!(d.kind, NodeKind::Instance(_))`. Consequently, neither `writer_command` (`crates/designos-doc/src/writer_command.rs:33-38`) nor `comp_ops::with_sync` (`crates/designos-app/src/comp_ops.rs:112-157`) updated `inst.name` on top-level or nested instances when their main `Component` (or `ComponentSet`) was renamed.

### Fix Summary

- **`crates/designos-doc/src/writer_command.rs` (`companion_component_rename_updates` & `update_boundary_step`):**
  - Centralizes companion rename resolution in `designos_doc::writer_command::companion_component_rename_updates(doc, target_id, old_name, new_name) -> Vec<Command>`.
  - When a `NodeKind::Component` with a single leaf shape child sharing `old_name` is renamed, or when that sole leaf shape child is renamed, emits a companion `Command::Update` for the paired node (`comp_id` <-> `inner_id`).
  - When an `Instance` is directly renamed (`old_name != new_name`), inserts `OverrideField::Name` into `inst.component.overrides` so its custom name is preserved across future main-component renames (and restored on undo/revert via `Command::Update`).
  - When a main `Component` (or `ComponentSet`, or a single-shape component via its inner child) is renamed, emits companion `Command::Update`s for every `NodeKind::Instance` referencing that component whose `overrides` does not contain `OverrideField::Name` and whose `name == old_name`.
  - Wraps single `Command::Update` + companion updates into a single `Command::Batch` in `writer_command` (`update_boundary_step`) so `designos-mcp` (`Engine::apply`), `live_patch::replay::apply_patch` (`doc-patch.jsonl`), and `History::apply` treat the rename and its companions as one atomic undo step.
- **`crates/designos-doc/src/live_patch/replay.rs`:**
  - Records `(node.id, TouchKind::Updated)` in `ReplayReport::touched` for every companion `Command::Update` inside the batch returned by `writer_command`.
- **`crates/designos-app/src/agent_turn_ledger.rs`:**
  - Updates `snapshot_pre_step` on `LiveOp::Update` to also snapshot pre-step names and fingerprints for all companion nodes returned by `companion_component_rename_updates`, ensuring the agent Revert/Keep card's fallback path (`revert_turn`) restores companion nodes as well as the fast `doc_undo` path.
- **`crates/designos-app/src/comp_ops.rs`:**
  - Calls `companion_component_rename_updates` inside `with_sync` (`collect_rename_companions`) so interactive user renames (`App::rename` -> `App::apply`) include the same companion `Command::Update`s in the same undo batch.

---

## 5. Skills Loaded & Concrete Changes Made Because of Them

| Skill Loaded | Concrete Action Taken Because of the Skill |
|---|---|
| `/es:dev` -> `es:feature-dev` | Followed the Classify -> Diagnose -> Falsification Test (Red) -> Minimal Root Fix (Green) -> Review Gate pipeline; isolated Commit 1 (`efeb1e86f`, failing reproduction test + `red.log`) before writing any fix code. |
| `es:rust` | Preserved `CLAUDE.md` §4 zero-clone / zero-`to_json()` invariants: zero `doc.to_json()` on the UI thread, zero new allocations in `canvas::draw::overlays`, and deterministic `BTreeSet<NodeId>` deduplication in `companion_component_rename_updates`. |
| `es:rust-gpu-canvas` | Verified `canvas::draw::label_rects()` cache invalidation (`mutation_epoch`) and preserved viewport-only invariants (`s.shell.app.view.offset` pan in off-screen test never calls `app.changed()` or bumps `TileView::rev`). |
| `es:designer` | Captured a 1440×900 visual verification screenshot (`plans/reports/agy-1193-canvas-label-rename-evidence/canvas-label-rename-1440.png`) via `designos --screenshot --size 1440x900 --select Card --no-toast` showing Layers (`❖ Card Component`), Canvas frame label (`❖ Card`), and Properties (`Card`) all in agreement after an MCP rename. |
| `es:fable-thinking` | Constructed an explicit epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), stated predictions before each test/build command, falsified the code map's "missing `get_mut` in `LiveOp::Update`" hypothesis, and proved the split-node (`Component` wrapper + inner `Rectangle`) root cause with `file:line` evidence. |
| `es:astra-work` | Verified all acceptance criteria (AC-1 through AC-4, all 3 PM edge cases, and Controller Rulings 1–5) with automated tests and saved raw artifacts (`red.log`, `green.log`, `canvas-label-rename-1440.png`). |

---

## 6. Acceptance Table & Verification Outputs

| Requirement | Verification Test / Artifact | Verdict |
|---|---|---|
| **AC-1**: After an agent rename is applied, the canvas frame label shows the new name within 500 ms, with no user interaction (`live_log_wake` 500 ms tick + `apply_live_patches_with_offsets`). | `shell::tests::canvas_label_rename::repro_qa_1193_rectangle_ctrl_alt_k_agent_rename_updates_canvas_frame_label` | **PASS** |
| **AC-2**: Layers, Properties and the canvas label always agree after any rename source: user, agent/MCP, undo or redo. | `repro_qa_1193_...`, `user_rename_on_ctrl_alt_k_component_keeps_canvas_label_layers_and_properties_in_sync`, `undo_and_redo_after_component_rename_keep_canvas_label_layers_and_properties_in_sync` | **PASS** |
| **AC-3**: Revert on the agent's Revert/Keep card restores the old name on the canvas label too (both fast `doc_undo` path and fallback `revert_turn` path after intervening user edits). | `shell::tests::canvas_label_rename::revert_on_agent_turn_card_restores_old_name_on_canvas_label_in_fast_and_fallback_paths` | **PASS** |
| **AC-4**: A regression test covers rename-via-tool, and the label render text after it. | `crates/designos-app/src/shell/tests/canvas_label_rename.rs` (7 integration tests inspecting painted `egui::ClippedShape` text glyphs in the canvas label band above the frame) | **PASS** |
| **Edge Case 1**: Renaming a main component updates its instances' labels where they show (and preserves explicitly overridden instance names). | `shell::tests::canvas_label_rename::renaming_main_component_updates_instance_canvas_labels_and_preserves_overridden_instance_names` | **PASS** |
| **Edge Case 2**: Several renames in one transaction show only the final name. | `shell::tests::canvas_label_rename::several_renames_in_one_transaction_show_only_the_final_name_on_canvas_label` | **PASS** |
| **Edge Case 3**: An off-screen rename or a rename on an unloaded page is correct when you scroll to it (without bumping `TileView::rev` on viewport pan). | `shell::tests::canvas_label_rename::off_screen_and_unloaded_page_renames_paint_updated_canvas_label_when_scrolled_or_switched_to` | **PASS** |
| **Visual Evidence (1440×900)**: 1440 capture of the canvas showing the label after MCP rename. | `plans/reports/agy-1193-canvas-label-rename-evidence/canvas-label-rename-1440.png` (`1440x900` PNG) | **PASS** |

### Exact Gate Commands & Outputs

#### 1. Red Log on Commit 1 (`efeb1e86f` — `plans/reports/agy-1193-canvas-label-rename-evidence/red.log`)

Command:
```bash
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=2 RUST_TEST_THREADS=2 cargo test -p designos-app --bin designos --locked canvas_label_rename
```

Output (`red.log`):
```text
running 7 tests
test shell::tests::canvas_label_rename::renaming_main_component_updates_instance_canvas_labels_and_preserves_overridden_instance_names ... FAILED
test shell::tests::canvas_label_rename::off_screen_and_unloaded_page_renames_paint_updated_canvas_label_when_scrolled_or_switched_to ... FAILED
test shell::tests::canvas_label_rename::repro_qa_1193_rectangle_ctrl_alt_k_agent_rename_updates_canvas_frame_label ... FAILED
test shell::tests::canvas_label_rename::revert_on_agent_turn_card_restores_old_name_on_canvas_label_in_fast_and_fallback_paths ... FAILED
test shell::tests::canvas_label_rename::several_renames_in_one_transaction_show_only_the_final_name_on_canvas_label ... FAILED
test shell::tests::canvas_label_rename::undo_and_redo_after_component_rename_keep_canvas_label_layers_and_properties_in_sync ... FAILED
test shell::tests::canvas_label_rename::user_rename_on_ctrl_alt_k_component_keeps_canvas_label_layers_and_properties_in_sync ... FAILED

test result: FAILED. 0 passed; 7 failed; 0 ignored; 0 measured; 3321 filtered out; finished in 4.96s
```

#### 2. Green Log on Commit 2 (`plans/reports/agy-1193-canvas-label-rename-evidence/green.log`)

Command:
```bash
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=2 RUST_TEST_THREADS=2 cargo test -p designos-app --bin designos --locked canvas_label_rename
```

Output (`green.log`):
```text
running 7 tests
test shell::tests::canvas_label_rename::renaming_main_component_updates_instance_canvas_labels_and_preserves_overridden_instance_names ... ok
test shell::tests::canvas_label_rename::off_screen_and_unloaded_page_renames_paint_updated_canvas_label_when_scrolled_or_switched_to ... ok
test shell::tests::canvas_label_rename::repro_qa_1193_rectangle_ctrl_alt_k_agent_rename_updates_canvas_frame_label ... ok
test shell::tests::canvas_label_rename::revert_on_agent_turn_card_restores_old_name_on_canvas_label_in_fast_and_fallback_paths ... ok
test shell::tests::canvas_label_rename::several_renames_in_one_transaction_show_only_the_final_name_on_canvas_label ... ok
test shell::tests::canvas_label_rename::undo_and_redo_after_component_rename_keep_canvas_label_layers_and_properties_in_sync ... ok
test shell::tests::canvas_label_rename::user_rename_on_ctrl_alt_k_component_keeps_canvas_label_layers_and_properties_in_sync ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 3321 filtered out; finished in 11.01s
```

#### 3. `cargo fmt --all -- --check`

Command:
```bash
cargo fmt --all -- --check
```

Output:
```text
(exit code 0, zero diff)
```

#### 4. `CARGO_BUILD_JOBS=2 cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```bash
CARGO_BUILD_JOBS=2 cargo clippy --workspace --all-targets --locked -- -D warnings
```

Output:
```text
    Checking designos-doc v0.6.2 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1193-canvas-label-rename/crates/designos-doc)
    Checking designos-mcp v0.6.2 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1193-canvas-label-rename/crates/designos-mcp)
    Checking designos-gpu v0.6.2 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1193-canvas-label-rename/crates/designos-gpu)
   Compiling designos-app v0.6.2 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1193-canvas-label-rename/crates/designos-app)
    Checking designos-eval v0.6.2 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1193-canvas-label-rename/crates/designos-eval)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 48s
```

#### 5. `cargo test --workspace --locked --lib`

Command:
```bash
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=2 RUST_TEST_THREADS=2 cargo test --workspace --locked --lib
```

Output:
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-ed3ecce3feb87bcf)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-bb064a8859214ef0)
test result: ok. 633 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-3e0687289d681ece)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-b2412c33d679e737)
test result: ok. 136 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.59s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-681656322fbabd0d)
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.38s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-958af4fb3c30e08a)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-937751f493831fc6)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-7e0f22231ff8111b)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.37s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-afe87df9282153f5)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.13s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-1d095711e2fddfa7)
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 47.46s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-907a3b742c402775)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-09c1acfe5b19c236)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
```

#### 5b. `cargo test -p designos-app --bin designos --locked`

Command:
```bash
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=2 RUST_TEST_THREADS=2 cargo test -p designos-app --bin designos --locked
```

Output:
```text
     Running unittests src/main.rs (out/cargo/debug/deps/designos-853c7e514bcd0028)
running 3328 tests
...
test result: ok. 3317 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 505.53s
```

#### 6. `python3 scripts/zero_fake_ui_gate.py`

Command:
```bash
python3 scripts/zero_fake_ui_gate.py
```

Output:
```text
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
```

#### 7. `git log --oneline origin/release/v0.6.3..HEAD`

```text
<HEAD> fix(doc,app): sync component wrapper and instance names on rename (#1193)
efeb1e86f test(app): reproduce stale canvas frame label after component rename (#1193)
```

---

## 7. Assumptions & Unresolved Questions

- **Assumptions:**
  - Single-shape component wrapper/child name coupling applies specifically when a `NodeKind::Component` has a single leaf non-frame child whose current name equals the component wrapper's current name (`old_name`), which is the exact structure created by `Document::create_component` (`Ctrl+Alt+K`).
  - Direct renames on `NodeKind::Instance` mark `OverrideField::Name` in `inst.component.overrides` so user/agent instance-specific names are not overwritten by subsequent main-component renames.
- **Unresolved Questions:** None.

Status: DONE
