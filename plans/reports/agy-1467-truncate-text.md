# Report: `#1467` — Truncate text toggle and Max lines field in Type settings

> Citation basis: Round 1 (sections 0 to 5) cites `crates/designos-app/src/panels/typography.rs` and `crates/designos-app/src/shell/tests/typography.rs` as they were at `150fb411`; resolve those with `git show 150fb411:<path>`. Round 2 changed both files, so their line numbers moved. The claims Round 2 corrected, and everything under `## Round 2`, cite the final code tree (commit `b4a7470b`); `r2-citation-grep.log` shows each of those lines, grep-checked. Round-1 logs (`test-designos-app.log` and the other logs without an `r2-` prefix) are unchanged.

## 0. What Could Not Be Verified & Why

1. **Live windowed GUI capture on the Untitled UI PRO v8.0 kit (`❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig`) was skipped per the `HIDIdleTime < 300s` guard (`brief-common-argon.txt:154-175`, `brief-agy-1467-truncate-text.txt:152-155`).**
   - `ioreg -c IOHIDSystem` reported `HIDIdleTime < 300s` (`0s`–`15s` active user session) throughout execution, so launching a windowed Studio instance was forbidden (`brief-common-argon.txt:156-163`).
   - Instead, per the headless fallback rule (`brief-agy-1467-truncate-text.txt:153-155`), `headless_popover_render_truncate_off_and_on_max_lines_2` (`crates/designos-app/src/shell/tests/typography.rs@150fb411:447-519`) drove two full headless `egui::Context` frames at `1600x1088` (`crates/designos-app/src/shell/tests/typography.rs@150fb411:451-458`), rasterized the clipped `egui` triangle meshes and font atlas via `apply_textures_delta` (`crates/designos-app/src/shell/tests/typography.rs@150fb411:344-371`) and `rasterize_shapes` (`crates/designos-app/src/shell/tests/typography.rs@150fb411:373-444`), and wrote the `Type settings` popover crops `type-settings-truncate-off-crop.png` (`312x760`, `r2-png-size.log:4`) and `type-settings-truncate-on-max-lines-2-crop.png` (`312x792`, `r2-png-size.log:5`) into `plans/reports/agy-1467-truncate-text-evidence/`.
2. **The full `cargo test -p designos-app --bin designos --locked` run failed `5` tests, in `4` modules (`test-designos-app.log:4300-4304`, `test-designos-app.log:4306`); it exited `101` (`test-designos-app.log:4309`).**
   - Result line: `4182 passed; 5 failed; 13 ignored` (`test-designos-app.log:4306`). The earlier text said 18 failures; that was wrong. The five, by name (`test-designos-app.log:4300-4304`):
     - `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root`: the socket path is 166 bytes (`test-designos-app.log:4275`).
     - `agent_session_files::tests::two_windows_never_touch_each_others_session_files`: `NotFound` (`test-designos-app.log:4281`).
     - `shell::tests::close_tab_kinds::a_fig_or_pen_tab_close_save_needs_a_native_path_and_never_rewrites_the_source`: `EDITOR_NOT_NOTIFIED` (`test-designos-app.log:4286`).
     - `shell::tests::close_tab_kinds::opening_the_library_file_again_makes_a_close_work_again`: `EDITOR_NOT_NOTIFIED` (`test-designos-app.log:4291`).
     - `vecops::tests::two_deferred_text_commands_both_convert_in_order`: "both texts still waiting for slow fonts" (`test-designos-app.log:4296`).
   - `agent_permissions` did not fail in that run. The earlier explanation (process-global `HOME`/`XDG_DATA_HOME` and `FONT_DB` scheduling) is withdrawn: the log does not establish a cause for any of the five.
   - A first rerun of five modules (not the same set as the failures above, because it included `agent_permissions`) failed `agent_permissions::tests::hook_rejects_a_malformed_scope_and_accepts_session_text` (`test-designos-app.log:4360`, panic at `agent_permissions.rs:2270` `test-designos-app.log:4391`, `test_app_isolated_rc=101` at `test-designos-app.log:4403`). A later rerun of the `5` modules (`117` tests total) at the bottom of `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4412-4584` passed `117/117` (`agent_approval_channel`: `5 passed` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4421`; `agent_permissions`: `47 passed` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4481`; `agent_session_files`: `19 passed` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4512`; `shell::tests::close_tab_kinds`: `13 passed` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4537`; `vecops::tests`: `33 passed` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4582`; `test_app_isolated_rc=0` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4584`).

---

## 1. Skills Loaded & What Changed Because of Them

- **`es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md:1-60`)**: Enforced an evidence-first execution loop (`OBSERVE -> ORIENT -> ACT -> VERIFY`), keeping scope strictly within `crates/designos-app/src/panels/typography.rs`, `crates/designos-app/src/shell/tests/typography.rs`, and `crates/designos-mcp/tests/text_truncation_roundtrip.rs`, and verifying that existing `G2` layout baseline test `shell::tests::layout_unchanged::f::layout_unchanged_f` still passes (`r2-test-designos-app.log:2980`). Round 1 cited `test-designos-app.log:3506` for it, which is another test, and quoted a `0.0pt` drift that no log shows; both are withdrawn.
- **`es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md:1-120`)**: Applied an explicit epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`) and falsification-first design: before editing `details_rows` (`crates/designos-app/src/panels/typography.rs@150fb411:745-755`), predicted how the popover height changes with `truncating` (the committed crops measure `312x760` off and `312x792` on, `r2-png-size.log:4-5`; the earlier `728.0` and `760.0` figures here were wrong and are deleted), and verified test strength by injecting two live code mutants (`plans/reports/agy-1467-truncate-text-evidence/mutant1.log:14,36`, `plans/reports/agy-1467-truncate-text-evidence/mutant2.log:11,14,41`).
- **`es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md:1-90`)**: Kept multi-selection aggregation in `truncate_state` (`crates/designos-app/src/panels/typography.rs:633-640`) and `max_lines_state` (`crates/designos-app/src/panels/typography.rs:643-650`) zero-allocation (`&[NodeId]` slice iteration over `app.doc`), and released `ctx.data_mut` lock guards immediately inside `max_lines_field` (`crates/designos-app/src/panels/typography.rs:689-695,717-719,725-727`) and `clear_max_lines_test_rects` (`crates/designos-app/src/panels/typography.rs:733-741`).
- **`es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md:1-90`)**: Verified that `TextAutoResize::Truncate` and `max_lines` already flow through `designos_render::laid_out` (`crates/designos-render/src/text.rs:545-577`) and `designos_gpu::geom::build_geom` (`crates/designos-gpu/src/geom.rs:503-512`) so `app.edit_nodes` (`crates/designos-app/src/panels/typography.rs:657` for Truncate text, `:673` for Max lines) commits the write through `run_tx`, which calls `self.changed()` (`crates/designos-app/src/ops.rs:279`, `crates/designos-app/src/ops.rs:232`) and one undo step. The earlier `app.changed()` citation at `typography.rs:665,681` was wrong.

---

## 2. Re-anchor Answers (Section 2 of Brief)

1. **Where text truncation and maximum line count are stored, drawn, imported, and exported:**
   - **Stored (`crates/designos-doc`)**:
     - `TextAutoResize` enum (`None`, `WidthAndHeight`, `Height`, `Truncate`) in `crates/designos-doc/src/paint.rs:426-432`.
     - `TextProps::auto_resize: TextAutoResize` (`crates/designos-doc/src/node.rs:683`) and `TextProps::max_lines: Option<u32>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`, `crates/designos-doc/src/node.rs:687`) on `TextProps` (`crates/designos-doc/src/node.rs:677-706`).
   - **Drawn (`crates/designos-render`, `crates/designos-gpu`, `crates/designos-app`)**:
     - CPU renderer (`crates/designos-render/src/text.rs`): `laid_out` (`crates/designos-render/src/text.rs:545-577`) checks `tp.auto_resize == TextAutoResize::Truncate` (`crates/designos-render/src/text.rs:546`), computes the line cutoff via `truncation_limit(rows.len(), start_y, tp.max_lines, lh, box_size.y)` (`crates/designos-render/src/text.rs:304-310`), and appends `"…"` via `truncated_segments` (`crates/designos-render/src/text.rs:314-336`).
     - GPU renderer (`crates/designos-gpu/src/geom.rs:503-512`): `build_geom` calls `designos_render::text_outlines(tp, n.size)` (`crates/designos-gpu/src/geom.rs:507`), which delegates to `laid_out` (`crates/designos-render/src/text.rs:383-387`).
     - App layout (`crates/designos-app/src/layout.rs:19-25`): `fit_text` returns `size` unchanged for `TextAutoResize::None | TextAutoResize::Truncate` (`crates/designos-app/src/layout.rs:23`), keeping the fixed text box while `Truncate` is active.
   - **Imported (`crates/designos-fig/src/rest_import.rs`)**:
     - `auto_resize` (`crates/designos-fig/src/rest_import.rs:439-446`) maps `"TRUNCATE"` or `textTruncation == "ENDING"` to `TextAutoResize::Truncate` (`crates/designos-fig/src/rest_import.rs:444`).
     - `parse_text` (`crates/designos-fig/src/rest_import.rs:3915-3920`) reads `maxLines` into `max_lines: Option<u32>` (`crates/designos-fig/src/rest_import.rs:3916-3919`), and component overrides map `"textTruncation"` / `"maxLines"` in `crates/designos-fig/src/rest_import.rs:3205-3213`.
   - **Exported (`crates/designos-fig`)**:
     - `.fig` binary export (`crates/designos-fig/src/fig_export.rs:2481-2496`): `TextAutoResize::Truncate` writes `textAutoResize = "NONE"` (`crates/designos-fig/src/fig_export.rs:2486-2488`) and `textTruncation = "ENDING"` (`crates/designos-fig/src/fig_export.rs:2491-2493`); `maxLines` is written only when `max_lines` is `Some` (`crates/designos-fig/src/fig_export.rs:2494-2496`), with no default of `1`.
     - REST JSON export (`crates/designos-fig/src/export.rs:227-244`): `TextAutoResize::Truncate` writes `"textAutoResize": "TRUNCATE"`, `"textTruncation": "ENDING"`, and `"maxLines"` (`crates/designos-fig/src/export.rs:233,239-243`).

2. **How the Type settings popover is opened and laid out in `crates/designos-app/src/panels/typography.rs`:**
   - Clicking the `"type-details"` icon button (`crates/designos-app/src/panels/typography.rs:173-183`) toggles `ui.ctx().data_mut(|d| d.insert_temp(pop_id, !open))` (`crates/designos-app/src/panels/typography.rs:181`), which invokes `details_popover(ui.ctx(), app, &sel, btn.rect)` (`crates/designos-app/src/panels/typography.rs:251-253`).
   - `details_popover` (`crates/designos-app/src/panels/typography.rs@150fb411:760-1120`) renders an `egui::Area` (`Id::new("type-details-popover")`, `Order::Foreground`, width `w = 312.0`, `crates/designos-app/src/panels/typography.rs:769,787-791`) whose height `h = details_rows(&ss_tags, truncating) * 32.0 + 24.0` (`crates/designos-app/src/panels/typography.rs:770,745-755`) is driven by a `next` row-stepper closure (`crates/designos-app/src/panels/typography.rs:809-813`).
   - Below `"Paragraph"` (`crates/designos-app/src/panels/typography.rs@150fb411:956-1042`), `"Truncate text"` (`det-truncate`) is rendered through `widgets::popover::check` with an `Option<bool>` state (`crates/designos-app/src/panels/typography.rs:1040`; `trunc_state` from `truncate_state`, `crates/designos-app/src/panels/typography.rs:769`) at `trunc_y = next() - 8.0` (`crates/designos-app/src/panels/typography.rs:1031-1043`). There is no `check_row_state` or `CheckState`. When `trunc_state == Some(true)` (`crates/designos-app/src/panels/typography.rs:1044`), `"Max lines"` (`det-max-lines`) is rendered at `ml_y = next() - 8.0` (`crates/designos-app/src/panels/typography.rs:1046`) via `max_lines_field` (`crates/designos-app/src/panels/typography.rs:689-715`), otherwise `clear_max_lines_test_rects` (`crates/designos-app/src/panels/typography.rs:717-729`) removes stale test rects (`crates/designos-app/src/panels/typography.rs:1053`).

3. **Every place in the UI where text resize mode can be set, and what turning truncation off should restore:**
   - **Layout section (`crates/designos-app/src/panels/properties.rs`)**: `resizing_ui` (`crates/designos-app/src/panels/properties.rs:1455-1467`, called at `crates/designos-app/src/panels/properties.rs:1039-1041`) calls `set_sizing` (`crates/designos-app/src/panels/properties.rs:1396-1418`), mapping `(Hug, Hug) -> WidthAndHeight`, `(Fixed, Hug) -> Height`, and `(_, _) -> None` (`crates/designos-app/src/panels/properties.rs:1405-1409`), while `sizing_of_with_parent` (`crates/designos-app/src/panels/properties.rs:1382-1394`) reports `(Fixed, Fixed)` for both `TextAutoResize::None` and `TextAutoResize::Truncate` (`crates/designos-app/src/panels/properties.rs:1389`).
   - **Canvas interactions**: double-clicking a text resize handle (`crates/designos-app/src/canvas/interact.rs:2033-2038`) sets `WidthAndHeight` or `Height`, and dragging a text box corner/edge (`crates/designos-app/src/ops.rs:53-54`) sets `TextAutoResize::None`.
   - **Type settings popover (`crates/designos-app/src/panels/typography.rs:656-669`)**: turning `"Truncate text"` on sets `tp.auto_resize = TextAutoResize::Truncate` and turning it off sets `TextAutoResize::None` (`crates/designos-app/src/panels/typography.rs:659`), keeping the fixed text box while preserving `tp.max_lines`. Hug-to-Fixed on `layout_child` runs only for text layers (`crates/designos-app/src/panels/typography.rs:660-665`).

4. **Whether max height is exposed or stored on text layers:**
   - **Data model (`crates/designos-doc`)**: `Node::layout_child.max_h: Option<f32>` (`crates/designos-doc/src/node.rs:392,1111`) and `"maxHeight"` in `Node::variables.bindings` (`crates/designos-doc/src/styles_ops.rs:702-705`) exist on every `Node`, including text nodes.
   - **UI (`crates/designos-app/src/panels/properties.rs:1442-1502`)**: the inspector does not expose a max-height input widget, but documents or specs can carry `layout_child.max_h` / `"maxHeight"` on a text layer.
   - **Interaction with `Max lines` (`crates/designos-app/src/panels/typography.rs:672-684`)**: per Figma's rule that setting `Max lines` removes a max height on the text layer (`brief-agy-1467-truncate-text.txt:79`), `set_max_lines` clears `n.layout_child.max_h = None` and `n.variables.bindings.remove("maxHeight")` whenever `max_lines.is_some()` (`crates/designos-app/src/panels/typography.rs:679-682`) inside one `app.edit_nodes` call, which is one undo step (`crates/designos-app/src/panels/typography.rs:673`; there is no `app.snapshot` call), verified by `setting_max_lines_clears_max_height_on_text_layer_in_same_undo_step` (`crates/designos-app/src/shell/tests/typography.rs@150fb411:312-342`).

5. **How `designos-mcp` and the linter read truncation:**
   - **`designos-mcp`**:
     - `designos-mcp` has no setter for `auto_resize` or `max_lines`. `UpdateFields` has no such field (`crates/designos-mcp/src/tools/mod.rs:183-194`). `build_updated_node` only turns `WidthAndHeight` or `Height` into a fixed or height-only mode when a height or width is updated (`crates/designos-mcp/src/tools/mod.rs:272-279`) and never writes `Truncate` or `max_lines`; the earlier claim that it accepts both was wrong.
     - The spec path is `crates/designos-doc/src/spec_instance.rs` (there is no `crates/designos-mcp/src/tools/spec_instance.rs`). Its text measuring keeps the box size for `TextAutoResize::None | TextAutoResize::Truncate` and does not read `max_lines` (`crates/designos-doc/src/spec_instance.rs:1122-1133`, `crates/designos-doc/src/spec_instance.rs:1299-1308`). `NodeSpec` rejects unknown keys (`crates/designos-doc/src/spec.rs:51`) and has no truncation key, so `designos_apply_spec` cannot set either field.
     - Consequence for ruling 4 of the round-1 brief: the round-trip and lint tests set truncation directly on the document (`crates/designos-mcp/tests/text_truncation_roundtrip.rs:52-55`, `:192-195`, `:215-218`) and then check what the tools preserve and report. Ruling 4 was met that way, not through an MCP setter.
   - **Linter (`crates/designos-doc/src/lint` & `crates/designos-render/src/lint_env.rs`)**:
     - `ApproxLintEnv::text_metrics` (`crates/designos-doc/src/lint/mod.rs:147-155`) and `RenderLintEnv::text_metrics` (`crates/designos-render/src/lint_env.rs:60-70`) set `TextMetrics::truncated` when `tp.auto_resize == TextAutoResize::Truncate` and lines exceed `tp.max_lines` or box height, triggering rule `"text-truncated"` (`crates/designos-doc/src/lint/rules/mod.rs:85-91,520-559`).
     - Verified in `crates/designos-mcp/tests/text_truncation_roundtrip.rs:13-220` (`plans/reports/agy-1467-truncate-text-evidence/mcp-truncation-test.log:9-12`).

---

## 3. Acceptance Table

| Gate | Command | Exit Code | Key Evidence & Citations |
|---|---|---:|---|
| **RED -> GREEN** | `CARGO_TARGET_DIR="$PWD/out/cargo" cargo test -p designos-app --bin designos --locked shell::tests::typography::` + `cargo test -p designos-mcp --test text_truncation_roundtrip --locked` | `101` -> `0` (`green_rc=0`, `mcp_rc=0`) | RED: `6 passed; 4 failed` (`plans/reports/agy-1467-truncate-text-evidence/red-test.log:11,14-16,50-55`). GREEN: `11 passed; 0 failed` (`plans/reports/agy-1467-truncate-text-evidence/green-test.log:20-32,34`) + `2 passed; 0 failed` (`plans/reports/agy-1467-truncate-text-evidence/mcp-truncation-test.log:9-12,14`). |
| **Mutant check** | Mutant 1 (`&sel[..sel.len().min(1)]` in `set_truncate_text`, `crates/designos-app/src/panels/typography.rs:655`) & Mutant 2 (`if true` for `Max lines`, `crates/designos-app/src/panels/typography.rs:1057`) | `101` & `101` | Mutant 1 killed by `truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step` (`9 passed; 1 failed`, `plans/reports/agy-1467-truncate-text-evidence/mutant1.log:14,26-29,36`). Mutant 2 killed by `max_lines_appears_only_while_truncating` and `truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step` (`8 passed; 2 failed`, `plans/reports/agy-1467-truncate-text-evidence/mutant2.log:11,14,26-27,33-34,41`). |
| **Studio screenshot** | `DESIGNOS_POPOVER_RENDER_DIR="$PWD/out/evidence" CARGO_TARGET_DIR="$PWD/out/cargo" cargo test -p designos-app --bin designos --locked shell::tests::typography::headless_popover_render_truncate_off_and_on_max_lines_2 -- --nocapture` | `0` | Crops committed at `plans/reports/agy-1467-truncate-text-evidence/type-settings-truncate-off-crop.png` (`312x760`, `r2-png-size.log:4`) and `plans/reports/agy-1467-truncate-text-evidence/type-settings-truncate-on-max-lines-2-crop.png` (`312x792`, `r2-png-size.log:5`); full renders at `out/evidence/1467-popover-truncate-off.png` (`1600x1088`) and `out/evidence/1467-popover-truncate-on-max-lines-2.png` (`1600x1088`). |
| **Static gates** | `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets --locked` + `python3 scripts/zero_fake_ui_gate.py` | `0` (`fmt_rc=0`, `clippy_rc=0`, `17/17 PASS`) | `fmt_rc=0` (`plans/reports/agy-1467-truncate-text-evidence/fmt.log:1`); `clippy_rc=0` with `0` warnings in touched files (`plans/reports/agy-1467-truncate-text-evidence/clippy.log:46-47`); `Gate Verdict: PASS (17/17 invariants satisfied)` (`plans/reports/agy-1467-truncate-text-evidence/zero-fake-ui.log:24`). |
| **Tests** | `cargo test -p designos-mcp --locked` + `cargo test -p designos-app --bin designos --locked` + `cargo test --workspace --lib --locked` | `0` for `designos-mcp` and `--workspace --lib` (`test_mcp_rc=0`, `test_ws_lib_rc=0`); `101` for the full `designos-app` run (`test_app_rc=101`, `test-designos-app.log:4309`), `0` for the isolated rerun of five modules (`test_app_isolated_rc=0`, `test-designos-app.log:4584`) | `designos-mcp`: all test binaries passed (`326 passed` lib + `2 passed` in `text_truncation_roundtrip`, `plans/reports/agy-1467-truncate-text-evidence/test-designos-mcp.log:336,717,725` (Round 1 cited line 334 for the `326 passed` result; that line is a test line, the result is on 336)); `designos-app`: `4,182 passed; 5 failed` in the full run (`plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4306`; the five are named in section 0) + `117 passed` across the `5` isolated suites (`plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4421,4481,4512,4537,4582,4584`); `--workspace --lib`: `1,498 passed; 0 failed` (`plans/reports/agy-1467-truncate-text-evidence/test-workspace-lib.log:23,749,756,921,1070,1106,1117,1160,1492,1556,1563,1577,1579`). |

### Pasted Gate Outputs

#### 3.1 RED -> GREEN (`plans/reports/agy-1467-truncate-text-evidence/red-test.log:9-19,50-55` & `plans/reports/agy-1467-truncate-text-evidence/green-test.log:9,20-34`)

```text
# RED (plans/reports/agy-1467-truncate-text-evidence/red-test.log:9-19,50-55)
running 10 tests
test shell::tests::typography::popover_lists_only_the_stylistic_sets_the_font_has ... ok
test shell::tests::typography::max_lines_appears_only_while_truncating ... FAILED
test shell::tests::typography::typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing ... ok
test shell::tests::typography::features_follow_the_selected_range_while_editing ... ok
test shell::tests::typography::max_lines_commits_integer_at_least_one_shows_mixed_and_clearing_stores_none ... FAILED
test shell::tests::typography::truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step ... FAILED
test shell::tests::typography::setting_max_lines_clears_max_height_on_text_layer_in_same_undo_step ... FAILED
test shell::tests::typography::tab_in_a_text_session_nests_list_items_and_never_moves_egui_focus ... ok
test shell::tests::typography::feature_checks_toggle_on_the_layer ... ok
test shell::tests::typography::paragraph_settings_apply_to_the_layer ... ok
...
failures:
    shell::tests::typography::max_lines_appears_only_while_truncating
    shell::tests::typography::max_lines_commits_integer_at_least_one_shows_mixed_and_clearing_stores_none
    shell::tests::typography::setting_max_lines_clears_max_height_on_text_layer_in_same_undo_step
    shell::tests::typography::truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step

test result: FAILED. 6 passed; 4 failed; 0 ignored; 0 measured; 4189 filtered out; finished in 1.59s

# GREEN (plans/reports/agy-1467-truncate-text-evidence/green-test.log:9,20-34)
running 11 tests
test shell::tests::typography::popover_lists_only_the_stylistic_sets_the_font_has ... ok
test shell::tests::typography::typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing ... ok
test shell::tests::typography::features_follow_the_selected_range_while_editing ... ok
test shell::tests::typography::tab_in_a_text_session_nests_list_items_and_never_moves_egui_focus ... ok
test shell::tests::typography::max_lines_appears_only_while_truncating ... ok
test shell::tests::typography::setting_max_lines_clears_max_height_on_text_layer_in_same_undo_step ... ok
test shell::tests::typography::truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step ... ok
test shell::tests::typography::max_lines_commits_integer_at_least_one_shows_mixed_and_clearing_stores_none ... ok
test shell::tests::typography::feature_checks_toggle_on_the_layer ... ok
test shell::tests::typography::paragraph_settings_apply_to_the_layer ... ok
test shell::tests::typography::headless_popover_render_truncate_off_and_on_max_lines_2 ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 4189 filtered out; finished in 3.33s

green_rc=0
```

#### 3.2 MCP Truncation Round-trip & Linter (`plans/reports/agy-1467-truncate-text-evidence/mcp-truncation-test.log:8-14`)

```text
running 2 tests
test truncation_lint_findings_and_xpath_query_are_unchanged ... ok
test apply_spec_and_update_node_preserve_truncation_and_roundtrip ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

mcp_rc=0
```

#### 3.3 Mutant Kills (`plans/reports/agy-1467-truncate-text-evidence/mutant1.log:23-36` & `plans/reports/agy-1467-truncate-text-evidence/mutant2.log:23-41`)

```text
# Mutant 1: set_truncate_text writes only &sel[..sel.len().min(1)] (plans/reports/agy-1467-truncate-text-evidence/mutant1.log:23-36)
---- shell::tests::typography::truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step stdout ----
thread 'shell::tests::typography::truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step' (71361429) panicked at crates/designos-app/src/shell/tests/typography.rs:210:5:
assertion `left == right` failed
  left: WidthAndHeight
 right: Truncate
...
test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 4189 filtered out; finished in 2.18s

# Mutant 2: Max lines rendered unconditionally `if true` (plans/reports/agy-1467-truncate-text-evidence/mutant2.log:23-41)
---- shell::tests::typography::max_lines_appears_only_while_truncating stdout ----
thread 'shell::tests::typography::max_lines_appears_only_while_truncating' (71366513) panicked at crates/designos-app/src/shell/tests/typography.rs:239:5:
Max lines must not exist while Truncate text is off

---- shell::tests::typography::truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step stdout ----
thread 'shell::tests::typography::truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step' (71366519) panicked at crates/designos-app/src/shell/tests/typography.rs:205:5:
Max lines stays hidden while Truncate text is Mixed
...
test result: FAILED. 8 passed; 2 failed; 0 ignored; 0 measured; 4189 filtered out; finished in 1.95s
```

#### 3.4 Static Gates (`plans/reports/agy-1467-truncate-text-evidence/fmt.log:1`, `plans/reports/agy-1467-truncate-text-evidence/clippy.log:46-47`, `plans/reports/agy-1467-truncate-text-evidence/zero-fake-ui.log:23-25`)

```text
# fmt.log (plans/reports/agy-1467-truncate-text-evidence/fmt.log:1)
fmt_rc=0

# clippy.log (plans/reports/agy-1467-truncate-text-evidence/clippy.log:46-47)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.40s
clippy_rc=0

# zero-fake-ui.log (plans/reports/agy-1467-truncate-text-evidence/zero-fake-ui.log:23-25)
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
```

---

## 4. Epistemic Ledger & Weakest Link

### Epistemic Ledger

- **`FACT`**: `TextAutoResize::Truncate` (`crates/designos-doc/src/paint.rs:426-432`) and `TextProps::max_lines: Option<u32>` (`crates/designos-doc/src/node.rs:687`) were already wired in `designos-doc`, `designos-render` (`crates/designos-render/src/text.rs:304-336,545-577`), `designos-gpu` (`crates/designos-gpu/src/geom.rs:503-512`), `designos-fig` (`crates/designos-fig/src/rest_import.rs:439-446,3915-3920`, `crates/designos-fig/src/fig_export.rs:2481-2496`), and `designos-doc` spec measuring (`crates/designos-doc/src/spec_instance.rs:1122-1133`; `designos-mcp` has no setter for either field), but `details_popover` (`crates/designos-app/src/panels/typography.rs@150fb411:760-1120`) had no `Truncate text` toggle or `Max lines` field.
- **`FACT`**: `details_rows` for the Basic tab is `(1 + 7 + chips + 8 + usize::from(truncating)) as f32 + 3.0` (`crates/designos-app/src/panels/typography.rs:740`), so the popover grows by one `ROW_H` of `32px` (`crates/designos-app/src/widgets/popover.rs:8`) while Truncate text is on; the committed crops measure `312x760` off and `312x792` on (`r2-png-size.log:4-5`). The layout baseline test `shell::tests::layout_unchanged::f::layout_unchanged_f` passed in the Round 2 full run (`r2-test-designos-app.log:2980`). Round 1 cited `test-designos-app.log:3506` for it, but that line is another test, so the claim was unsupported; the `728.0px` and `y = 1026.0..1050.0` figures are deleted (`next()` offsets are body-relative).
- **`FACT`**: When `"Truncate text"` is hidden or unchecked/mixed, `clear_max_lines_test_rects` (`crates/designos-app/src/panels/typography.rs:717-729,1053`) clears the recorded `"det-max-lines"` test rect and the `("field-rect", ...)` entry from `ctx.data_mut`, so the test helpers `has_no_rect` (`crates/designos-app/src/shell/tests/typography.rs:18`) and `field_rect` (`crates/designos-app/src/shell/tests.rs:224`) never see a stale rect after truncation is turned off. There are no `has_control` or `has_field` helpers.
- **`INFERENCE`**: Restoring `TextAutoResize::None` when `"Truncate text"` is turned off (`crates/designos-app/src/panels/typography.rs:659`) matches Figma's fixed-size text box behavior after truncation and preserves `tp.max_lines` on the layer so re-enabling `"Truncate text"` remembers the previous line limit; Round 2 pins that with `turning_truncation_off_keeps_max_lines_and_turning_it_on_shows_the_old_limit` (`crates/designos-app/src/shell/tests/typography.rs:455-474`). The Round 1 citation `shell/tests/typography.rs:249-252` did not assert that.
- **`ASSUMPTION`**: None remaining.

### Weakest Link

- **Weakest link**: Live windowed GPU screenshot capture on the Untitled UI PRO v8.0 kit could not be executed because the host session had `HIDIdleTime < 300s` (`0s`–`15s`). This is mitigated by the headless `egui` + `cosmic-text` software rasterizer in `headless_popover_render_truncate_off_and_on_max_lines_2` (`crates/designos-app/src/shell/tests/typography.rs@150fb411:447-519`), which renders the exact `details_popover` widget tree and font atlas into `plans/reports/agy-1467-truncate-text-evidence/type-settings-truncate-off-crop.png` (`312x760`, `r2-png-size.log:4`) and `plans/reports/agy-1467-truncate-text-evidence/type-settings-truncate-on-max-lines-2-crop.png` (`312x792`, `r2-png-size.log:5`). Round 1 did not record that the crops were opened; Round 2 opened the on-crop and it shows Truncate text checked and Max lines `2`.

---

## 5. Git & Pull Request

- **Branch**: `jangtrinh/agy-1467-truncate-text` (based on `origin/main` = `a98ec2b1bcd6668092cbb4e97d8c82f305be8d54`)
- **Draft PR**: https://github.com/jangtrinh/design-os-studio/pull/1541
- **`git log --oneline origin/main..HEAD`**:
  - `e28ae5f1 feat(typography): truncate text toggle and max lines in Type settings (#1467)` (`crates/designos-app/src/panels/typography.rs:633-768,1044-1067`, `crates/designos-app/src/shell/tests/typography.rs:178-519`, `crates/designos-mcp/tests/text_truncation_roundtrip.rs:1-220`)

Status: READY

---

## Round 2

Scope: the review of PR #1541 found two majors, one low and a set of wrong claims in this report. Everything below is on code commit `b4a7470b`; the final commit after it adds only `plans/reports/` files (`r2-merge-and-cleanup.log:13-15` lists the only `crates/` files the branch changes, all three touched in the code commit).

### R2.0 What could NOT be verified

1. **No GUI capture this round.** The controller said none was needed, and no paint or layout code changed, so the Round 1 crops are unchanged (`r2-png-size.log:3`) and measure `312x760` and `312x792` (`r2-png-size.log:4-5`). The visual proof of this round is the red and green terminal logs. Round 2 opened the on-crop: Truncate text is checked and Max lines shows `2`.
2. **The off-then-on assert cannot be red on `150fb411`.** It pins behaviour that was already right (`set_truncate_text` never touched `max_lines`), so it is green before and after (`r2-red-test.log:25`, `r2-green-test.log:26`). It is shown to bite by a third mutant that clears `max_lines` when truncation goes off (`r2-off-clears-max-lines.log:26`, `r2-off-clears-max-lines.log:42`). Acceptance item 1 is therefore met as red-to-green for three of the four tests and as a pin plus a killed mutant for the fourth.
3. **Two full-run failures, both known on main per the brief; I did not run main** (no other checkout allowed). `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` fails alone too in this harness, because the socket path is 130 bytes against a limit of 103 (`r2-rerun-production-root.log:20`, assert at `crates/designos-app/src/agent_approval_channel.rs:286`). Not verified: that it passes with a path of 103 bytes or less (that needs a TMPDIR shorter than the harness one). The PR does not touch that file (`r2-merge-and-cleanup.log:13-15`).
4. **Clippy's three warnings are not compared against a run on main**, for the same reason. They are all in files the branch does not change (`r2-clippy.log:359`, `r2-clippy.log:373`, `r2-clippy.log:388`; changed files at `r2-merge-and-cleanup.log:13-15`).
5. **Not checked:** whether a Truncate text or Max lines write made while a text-editing session is open joins that session's transaction. `set_text_props` handles that case (`crates/designos-app/src/panels/typography.rs:612-618`); these two writers use `edit_nodes` directly (`crates/designos-app/src/panels/typography.rs:657`, `crates/designos-app/src/panels/typography.rs:673`). The review could not confirm it either, and this round does not change it.
6. Round 1 line citations into the two typography files are stale after this round's edit (see the note under the title); only the corrected claims were re-pointed.

### R2.1 Skills loaded and what changed because of them

- `es-rust`: read in full. Law 4 (no `egui::Context` call inside `ctx.data_mut`): `max_lines_field` lost its two `ui.data_mut` calls with the sticky flag (`crates/designos-app/src/panels/typography.rs@150fb411:707,711`; function now `crates/designos-app/src/panels/typography.rs:689-715`), so no `data_mut` closure remains in the Max lines commit path.
- `es-designer`: read in full. This round changes behaviour only, no rendered pixel, token or layout, so its craft-floor, checklist and 375/768/1440 captures were NOT RUN (responsive: SKIPPED, owner default). The project's own gate `scripts/zero_fake_ui_gate.py` did run (`r2-zero-fake-ui.log:28`).
- `es:fable-thinking` preamble: each run was predicted before it ran (`r2-red-test.log:7`, `r2-green-test.log:7`); one advisor consult happened before the red run. Its advice changed the work in three ways: run the red on untouched production code; use a blur click that keeps the popover open; treat the off-then-on assert as a pin.

### R2.2 Rulings and what changed

1. **Truncate text on a mixed selection.** The Hug-to-Fixed writes now sit inside the `NodeKind::Text` arm (`crates/designos-app/src/panels/typography.rs:656-668`, writes at `:660-665`). Non-text layers are not changed.
2. **Max lines clears only on an empty commit.** The sticky flag, its Backspace/Delete sniff and the focus-loss branch are removed (`crates/designos-app/src/panels/typography.rs:689-715`). `Some(None)` is returned only when the committed text is empty or whitespace (`crates/designos-app/src/panels/typography.rs:710-712`). `edit_text_hint` returns a commit on focus loss only when the buffer differs from both the live value and its seed (`crates/designos-app/src/widgets/field.rs:105-113`), so a field left as it was, including a Mixed one, writes nothing.
3. **Turning truncation off keeps `max_lines`.** Pinned by `turning_truncation_off_keeps_max_lines_and_turning_it_on_shows_the_old_limit` (`crates/designos-app/src/shell/tests/typography.rs:455-474`).
4. **Report truth.** Corrected in place in the sections above: five failures not eighteen with names (section 0 item 2); no MCP setter, round-trip and lint tests set truncation on the document (section 2 item 5); the `spec_instance.rs` path; the `app.changed()` and `app.snapshot` lines; `check_row_state` and `CheckState`; `has_control` and `has_field`; `maxLines` written only when set. Also corrected, because the review flagged them and they were provably wrong: the crop sizes, the `y = 1026.0..1050.0` range and `728.0px`, and the `test-designos-app.log:3506` citation for `layout_unchanged_f`. Every line cited by a correction is shown in `r2-citation-grep.log`.
5. **Module comment.** The issue number is gone from `crates/designos-mcp/tests/text_truncation_roundtrip.rs:1`, and lines 3-4 now say there is no setter and the tests set truncation on the document.

New and changed tests, all in `crates/designos-app/src/shell/tests/typography.rs`: `truncate_text_leaves_non_text_layers_in_the_selection_alone` (`:362-412`), `max_lines_keeps_its_value_when_the_field_is_emptied_and_the_same_digits_are_typed_again` (`:415-429`), `mixed_max_lines_is_not_written_when_backspace_is_followed_by_leaving_the_field` (`:432-452`), the pin (`:455-474`) and the helper `select_truncating` (`:345-359`). The existing "clearing stores `None`" test is kept and still passes through the empty-commit path (`crates/designos-app/src/shell/tests/typography.rs:297-304`, `r2-green-test.log:28`).

### R2.3 Acceptance

| # | Item | Result | Evidence |
|---|---|---|---|
| 1a | Mixed-selection Hug test red on `150fb411`, green on head | red at the non-text sizing assert, then green | `r2-red-test.log:22`, `r2-red-test.log:55-56`, `r2-green-test.log:22` |
| 1b | Backspace then retype | red (`None` instead of `Some(12)`), then green | `r2-red-test.log:20`, `r2-red-test.log:38-41`, `r2-green-test.log:20` |
| 1c | Mixed then Backspace then blur | red (`None` instead of `Some(2)`, popover still open), then green | `r2-red-test.log:21`, `r2-red-test.log:47-50`, `r2-green-test.log:21` |
| 1d | Off-then-on assert | green on both (see R2.0 item 2) | `r2-red-test.log:25`, `r2-green-test.log:26` |
| 2 | Null control green, then two mutants killed | null control `15 passed`; Hug-outside-arm killed; sticky-flag killed (two tests); extra mutant for the pin killed | `r2-null-control.log:31`, `r2-hug-outside-text-arm.log:25`, `r2-hug-outside-text-arm.log:50`, `r2-sticky-cleared-flag.log:23`, `r2-sticky-cleared-flag.log:25`, `r2-sticky-cleared-flag.log:60`, `r2-off-clears-max-lines.log:26` |
| 3a | `cargo fmt --all -- --check; echo fmt_rc=$?` | `fmt_rc=0` | `r2-fmt.log:5` |
| 3b | workspace clippy `--all-targets --locked` | `rc=0`, three warnings, all in unchanged files | `r2-clippy.log:399`, `r2-clippy.log:359`, `r2-clippy.log:373`, `r2-clippy.log:388` |
| 3c | `cargo test -p designos-app --bin designos --locked` | `4189 passed; 2 failed; 13 ignored`, `rc=101`; the two are the known ones; all 15 typography tests pass in it | `r2-test-designos-app.log:4256`, `r2-test-designos-app.log:4253-4254`, `r2-test-designos-app.log:4259`, `r2-test-designos-app.log:3413-3438` |
| 3c-alone | each failure rerun alone | socket-path test fails alone (130 bytes, harness TMPDIR); vecops passes alone | `r2-rerun-production-root.log:27`, `r2-rerun-production-root.log:20`, `r2-rerun-vecops.log:15` |
| 3d | `cargo test -p designos-mcp --locked` | `rc=0`; lib `326 passed`; truncation round-trip `2 passed` | `r2-test-designos-mcp.log:729`, `r2-test-designos-mcp.log:340`, `r2-test-designos-mcp.log:689` |
| 3e | `cargo test --workspace --locked --lib` | `rc=0`, 12 result lines all `ok`; the `large_file_perf` target is not part of `--lib` and does not appear | `r2-test-workspace-lib.log:1636`, `r2-test-workspace-lib.log:80`, `r2-test-workspace-lib.log:1634` |
| 3f | `python3 scripts/zero_fake_ui_gate.py` | `PASS (17/17)`, `zero_fake_rc=0` | `r2-zero-fake-ui.log:28`, `r2-zero-fake-ui.log:30` |
| 4 | Merge `origin/main`, push without force, `out/cargo` deleted | `Already up to date.`; `ls out/cargo` fails | `r2-merge-and-cleanup.log:7`, `r2-merge-and-cleanup.log:21` |

The two full-run failures, by name (`r2-test-designos-app.log:4253-4254`):

- `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root`: rerun alone fails, `130 bytes` (`r2-rerun-production-root.log:20`, `r2-rerun-production-root.log:27`).
- `vecops::tests::two_deferred_text_commands_both_convert_in_order`: rerun alone passes (`r2-rerun-vecops.log:13`, `r2-rerun-vecops.log:15`).

Three of Round 1's five full-run failures did not recur: `agent_session_files::tests::two_windows_never_touch_each_others_session_files` and both `close_tab_kinds` tests are absent from this run's failure list (`r2-test-designos-app.log:4252-4254`). They look load- or environment-dependent; this run does not say why.

### R2.4 Pasted output

Red on untouched production code (`r2-red-test.log:20-25`, `r2-red-test.log:66`):

```text
test shell::tests::typography::max_lines_keeps_its_value_when_the_field_is_emptied_and_the_same_digits_are_typed_again ... FAILED
test shell::tests::typography::mixed_max_lines_is_not_written_when_backspace_is_followed_by_leaving_the_field ... FAILED
test shell::tests::typography::truncate_text_leaves_non_text_layers_in_the_selection_alone ... FAILED
test shell::tests::typography::turning_truncation_off_keeps_max_lines_and_turning_it_on_shows_the_old_limit ... ok
test result: FAILED. 12 passed; 3 failed; 0 ignored; 0 measured; 4189 filtered out; finished in 3.72s
```

Green on the fix (`r2-green-test.log:33`, `r2-green-test.log:177`) and null control (`r2-null-control.log:31`):

```text
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 4189 filtered out; finished in 3.89s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 4189 filtered out; finished in 3.78s
```

Mutants (`r2-hug-outside-text-arm.log:40`, `r2-hug-outside-text-arm.log:50`, `r2-sticky-cleared-flag.log:41`, `r2-sticky-cleared-flag.log:50`, `r2-sticky-cleared-flag.log:60`, `r2-off-clears-max-lines.log:42`, `r2-off-clears-max-lines.log:52`); each log ends with an empty `git diff --stat` after `git checkout -- <file>` (`r2-hug-outside-text-arm.log:59`, `r2-sticky-cleared-flag.log:70`, `r2-off-clears-max-lines.log:63`):

```text
Hug writes outside the text arm : 14 passed; 1 failed (truncate_text_leaves_non_text_layers_in_the_selection_alone, tests/typography.rs:410)
sticky cleared flag restored    : 13 passed; 2 failed (max_lines_keeps_its_value... :427, mixed_max_lines_is_not_written... :448)
off clears max_lines            : 14 passed; 1 failed (turning_truncation_off_keeps_max_lines... :465)
```

Gates (`r2-fmt.log:5`, `r2-clippy.log:399`, `r2-test-designos-app.log:4256`, `r2-test-designos-mcp.log:729`, `r2-test-workspace-lib.log:1636`, `r2-zero-fake-ui.log:30`):

```text
fmt_rc=0
rc=0            (clippy, three warnings in untouched files)
test result: FAILED. 4189 passed; 2 failed; 13 ignored; 0 measured; 0 filtered out; finished in 404.42s
rc=0            (designos-mcp)
rc=0            (workspace --lib)
zero_fake_rc=0  (Gate Verdict: PASS (17/17 invariants satisfied))
```

### R2.5 Ledger

- **FACT** (observed this round): the red run fails the three behaviour tests at the intended asserts and the pin passes (`r2-red-test.log:38`, `r2-red-test.log:47`, `r2-red-test.log:55`); the fix turns all 15 green (`r2-green-test.log:33`); each mutant dies at its own assert (see R2.3 row 2).
- **FACT:** `origin/main` has not moved from `a98ec2b1`, so the merge was a no-op (`r2-merge-and-cleanup.log:7`, `r2-merge-and-cleanup.log:9`, `r2-merge-and-cleanup.log:11`).
- **INFERENCE:** the sticky flag was the only thing that could turn an unchanged or Mixed field into `Some(None)`: with it removed, a commit needs `edit_text_hint` to return text (`crates/designos-app/src/widgets/field.rs:105-113`), and the sticky-flag mutant is what brings the two failing behaviours back (`r2-sticky-cleared-flag.log:60`).
- **ASSUMPTION (high):** a hugging nested frame with `layout_child` Hug on both axes is a fair stand-in for an imported hugging auto-layout child. The layout solver never rewrites `layout_child` Hug (`crates/designos-doc/src/layout_solver_types.rs:118-125` maps only Fill), and the test asserts the sibling reads Hug before the click (`crates/designos-app/src/shell/tests/typography.rs:397-399`), so a normalising solver would fail the setup, not hide the bug. Flips if a document path rewrites `layout_child` on load.
- **ASSUMPTION (high):** "empty or whitespace" means `trim().is_empty()` (`crates/designos-app/src/panels/typography.rs:709-712`).
- **ASSUMPTION (medium):** the `DESIGNOS_1467_*` environment variable names in `crates/designos-app/src/shell/tests/typography.rs:639`, `:644` are left alone: ruling 5 named only the MCP module comment, and they are not comments, asserts or test names. They appear in Round 1's evidence commands, so renaming would break those. Flips if the owner wants the number gone everywhere.
- **ASSUMPTION (high):** the common brief's Orca reporting steps do not apply: this run is a Claude subagent, and the controller replaced them with the final message.
- **Weakest link:** the Mixed test's blur is a click on the empty part of the Max lines row; it is only meaningful because the popover stays open (asserted at `crates/designos-app/src/shell/tests/typography.rs:447`) and because the red run shows the click does reach the focus-loss path (`r2-red-test.log:47-50`).

### R2.6 Git

- Branch `jangtrinh/agy-1467-truncate-text`, branch name checked with `git branch --show-current` before each commit.
- `git log --oneline origin/main..HEAD` on the code commit, before the report commit:

```text
b4a7470b fix(typography): truncate text only touches text layers; max lines clears only on an empty commit
150fb411 docs(report): record draft PR #1541 and commit hash for #1467
e28ae5f1 feat(typography): truncate text toggle and max lines in Type settings (#1467)
```

  The report commit that adds this section follows it; its hash and the final log are in the final message.
- Push: `git push origin HEAD:jangtrinh/agy-1467-truncate-text`, no force, no rebase. The PR stays draft; nothing was merged or commented.
- `out/cargo` deleted: `ls out/cargo` fails with `No such file or directory` (`r2-merge-and-cleanup.log:20-22`).

### R2.7 Exact commands

Every cargo run used the common brief's harness: sibling `HOME` and `TMPDIR` under one `mktemp -d`, target dir inside the worktree.

```text
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home" "$S/tmp"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" <command below>

red / green / null control / each mutant : cargo test -p designos-app --bin designos --locked shell::tests::typography::
mcp round trip                           : cargo test -p designos-mcp --test text_truncation_roundtrip --locked
fmt                                      : cargo fmt --all -- --check; echo fmt_rc=$?
clippy                                   : cargo clippy --workspace --all-targets --locked
app, full run                            : cargo test -p designos-app --bin designos --locked
mcp                                      : cargo test -p designos-mcp --locked
workspace lib                            : cargo test --workspace --locked --lib
each full-run failure alone              : cargo test -p designos-app --bin designos --locked <module::test> -- --exact
zero fake ui                             : python3 scripts/zero_fake_ui_gate.py
merge and cleanup                        : git fetch origin main; git merge origin/main; rm -rf out/cargo; ls out/cargo
push                                     : git push origin HEAD:jangtrinh/agy-1467-truncate-text
```

Each mutant was applied by replacing the named function in `crates/designos-app/src/panels/typography.rs` with its `150fb411` body (or, for the pin, by adding `t.max_lines = None` to the off branch), run with the command above, and undone with `git checkout -- crates/designos-app/src/panels/typography.rs`; the empty `git diff --stat` after each is at the log lines cited in R2.4.

Status: DONE_WITH_CONCERNS
