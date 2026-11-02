# Report: `#1467` — Truncate text toggle and Max lines field in Type settings

## 0. What Could Not Be Verified & Why

1. **Live windowed GUI capture on the Untitled UI PRO v8.0 kit (`❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig`) was skipped per the `HIDIdleTime < 300s` guard (`brief-common-argon.txt:154-175`, `brief-agy-1467-truncate-text.txt:152-155`).**
   - `ioreg -c IOHIDSystem` reported `HIDIdleTime < 300s` (`0s`–`15s` active user session) throughout execution, so launching a windowed Studio instance was forbidden (`brief-common-argon.txt:156-163`).
   - Instead, per the headless fallback rule (`brief-agy-1467-truncate-text.txt:153-155`), `headless_popover_render_truncate_off_and_on_max_lines_2` (`crates/designos-app/src/shell/tests/typography.rs:447-519`) drove two full headless `egui::Context` frames at `1600x1088` (`crates/designos-app/src/shell/tests/typography.rs:451-458`), rasterized the clipped `egui` triangle meshes and font atlas via `apply_textures_delta` (`crates/designos-app/src/shell/tests/typography.rs:344-371`) and `rasterize_shapes` (`crates/designos-app/src/shell/tests/typography.rs:373-444`), and wrote the `Type settings` popover crops `type-settings-truncate-off-crop.png` (`312x728`, `crates/designos-app/src/shell/tests/typography.rs:479-484`) and `type-settings-truncate-on-max-lines-2-crop.png` (`312x760`, `crates/designos-app/src/shell/tests/typography.rs:509-515`) into `plans/reports/agy-1467-truncate-text-evidence/`.
2. **Single-process `cargo test -p designos-app --bin designos --locked` contention in `5` unrelated test modules (`plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4286-4306`).**
   - In the `4,200`-test single-process binary run (`plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:9`), `4,182` tests passed (`plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4306`) while `18` tests in `5` unrelated modules (`agent_approval_channel`, `agent_permissions`, `agent_session_files`, `shell::tests::close_tab_kinds`, `vecops::tests`) failed due to process-global `HOME`/`XDG_DATA_HOME` and global `FONT_DB` thread scheduling when run without `cargo-nextest`.
   - Re-running those `5` modules (`117` tests total) with their required isolation at the bottom of `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4412-4584` passed `117/117` (`agent_approval_channel`: `5 passed` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4421`; `agent_permissions`: `47 passed` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4481`; `agent_session_files`: `19 passed` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4512`; `shell::tests::close_tab_kinds`: `13 passed` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4537`; `vecops::tests`: `33 passed` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4582`; `test_app_isolated_rc=0` at `plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4584`).

---

## 1. Skills Loaded & What Changed Because of Them

- **`es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md:1-60`)**: Enforced an evidence-first execution loop (`OBSERVE -> ORIENT -> ACT -> VERIFY`), keeping scope strictly within `crates/designos-app/src/panels/typography.rs`, `crates/designos-app/src/shell/tests/typography.rs`, and `crates/designos-mcp/tests/text_truncation_roundtrip.rs`, and verifying that existing `G2` layout baseline test `shell::tests::layout_unchanged::f::layout_unchanged_f` (`plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:3506`) stayed at `0.0pt` overlay rect drift.
- **`es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md:1-120`)**: Applied an explicit epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`) and falsification-first design: before editing `details_rows` (`crates/designos-app/src/panels/typography.rs:745-755`), predicted and verified how `f-type-details` in `crates/designos-app/src/shell/tests/layout_unchanged/f.rs` checks the `overlay:Foreground/0` height (`728.0` when `truncating == false`, expanding to `760.0` only when `truncating == true`), and verified test strength by injecting two live code mutants (`plans/reports/agy-1467-truncate-text-evidence/mutant1.log:14,36`, `plans/reports/agy-1467-truncate-text-evidence/mutant2.log:11,14,41`).
- **`es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md:1-90`)**: Kept multi-selection aggregation in `truncate_state` (`crates/designos-app/src/panels/typography.rs:633-640`) and `max_lines_state` (`crates/designos-app/src/panels/typography.rs:643-650`) zero-allocation (`&[NodeId]` slice iteration over `app.doc`), and released `ctx.data_mut` lock guards immediately inside `max_lines_field` (`crates/designos-app/src/panels/typography.rs:689-695,717-719,725-727`) and `clear_max_lines_test_rects` (`crates/designos-app/src/panels/typography.rs:733-741`).
- **`es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md:1-90`)**: Verified that `TextAutoResize::Truncate` and `max_lines` already flow through `designos_render::laid_out` (`crates/designos-render/src/text.rs:545-577`) and `designos_gpu::geom::build_geom` (`crates/designos-gpu/src/geom.rs:503-512`) so `app.changed()` (`crates/designos-app/src/panels/typography.rs:665,681`) invalidates the scene index and GPU geometry cache in one step.

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
     - `.fig` binary export (`crates/designos-fig/src/fig_export.rs:2481-2496`): `TextAutoResize::Truncate` writes `textAutoResize = "NONE"`, `textTruncation = "ENDING"`, and `maxLines = max_lines.unwrap_or(1)` (`crates/designos-fig/src/fig_export.rs:2486-2490`).
     - REST JSON export (`crates/designos-fig/src/export.rs:227-244`): `TextAutoResize::Truncate` writes `"textAutoResize": "TRUNCATE"`, `"textTruncation": "ENDING"`, and `"maxLines"` (`crates/designos-fig/src/export.rs:233,239-243`).

2. **How the Type settings popover is opened and laid out in `crates/designos-app/src/panels/typography.rs`:**
   - Clicking the `"type-details"` icon button (`crates/designos-app/src/panels/typography.rs:173-183`) toggles `ui.ctx().data_mut(|d| d.insert_temp(pop_id, !open))` (`crates/designos-app/src/panels/typography.rs:181`), which invokes `details_popover(ui.ctx(), app, &sel, btn.rect)` (`crates/designos-app/src/panels/typography.rs:251-253`).
   - `details_popover` (`crates/designos-app/src/panels/typography.rs:760-1120`) renders an `egui::Area` (`Id::new("type-details-popover")`, `Order::Foreground`, width `w = 312.0`, `crates/designos-app/src/panels/typography.rs:769,787-791`) whose height `h = details_rows(&ss_tags, truncating) * 32.0 + 24.0` (`crates/designos-app/src/panels/typography.rs:770,745-755`) is driven by a `next` row-stepper closure (`crates/designos-app/src/panels/typography.rs:809-813`).
   - Below `"Paragraph"` (`crates/designos-app/src/panels/typography.rs:956-1042`), `"Truncate text"` (`det-truncate`) is rendered via `check_row_state` at `trunc_y = next() - 8.0` (`crates/designos-app/src/panels/typography.rs:1045-1056`), and when `truncating` (`matches!(trunc_state, CheckState::Checked(true))`, `crates/designos-app/src/panels/typography.rs:768`) is true, `"Max lines"` (`det-max-lines`) is rendered at `ml_y = next() - 8.0` (`crates/designos-app/src/panels/typography.rs:1057-1067`) via `max_lines_field` (`crates/designos-app/src/panels/typography.rs:686-728`), otherwise `clear_max_lines_test_rects` (`crates/designos-app/src/panels/typography.rs:730-742`) removes stale test rects (`crates/designos-app/src/panels/typography.rs:1066`).

3. **Every place in the UI where text resize mode can be set, and what turning truncation off should restore:**
   - **Layout section (`crates/designos-app/src/panels/properties.rs`)**: `resizing_ui` (`crates/designos-app/src/panels/properties.rs:1455-1467`, called at `crates/designos-app/src/panels/properties.rs:1039-1041`) calls `set_sizing` (`crates/designos-app/src/panels/properties.rs:1396-1418`), mapping `(Hug, Hug) -> WidthAndHeight`, `(Fixed, Hug) -> Height`, and `(_, _) -> None` (`crates/designos-app/src/panels/properties.rs:1405-1409`), while `sizing_of_with_parent` (`crates/designos-app/src/panels/properties.rs:1382-1394`) reports `(Fixed, Fixed)` for both `TextAutoResize::None` and `TextAutoResize::Truncate` (`crates/designos-app/src/panels/properties.rs:1389`).
   - **Canvas interactions**: double-clicking a text resize handle (`crates/designos-app/src/canvas/interact.rs:2033-2038`) sets `WidthAndHeight` or `Height`, and dragging a text box corner/edge (`crates/designos-app/src/ops.rs:53-54`) sets `TextAutoResize::None`.
   - **Type settings popover (`crates/designos-app/src/panels/typography.rs:654-666`)**: turning `"Truncate text"` on sets `tp.auto_resize = TextAutoResize::Truncate` (`crates/designos-app/src/panels/typography.rs:660`), and turning `"Truncate text"` off restores `tp.auto_resize = TextAutoResize::None` (`crates/designos-app/src/panels/typography.rs:660-662`), keeping the fixed text box while preserving `tp.max_lines`.

4. **Whether max height is exposed or stored on text layers:**
   - **Data model (`crates/designos-doc`)**: `Node::layout_child.max_h: Option<f32>` (`crates/designos-doc/src/node.rs:392,1111`) and `"maxHeight"` in `Node::variables.bindings` (`crates/designos-doc/src/styles_ops.rs:702-705`) exist on every `Node`, including text nodes.
   - **UI (`crates/designos-app/src/panels/properties.rs:1442-1502`)**: the inspector does not expose a max-height input widget, but documents or specs can carry `layout_child.max_h` / `"maxHeight"` on a text layer.
   - **Interaction with `Max lines` (`crates/designos-app/src/panels/typography.rs:670-682`)**: per Figma's rule that setting `Max lines` removes a max height on the text layer (`brief-agy-1467-truncate-text.txt:79`), `set_max_lines` clears `n.layout_child.max_h = None` and `n.variables.bindings.remove("maxHeight")` whenever `max_lines.is_some()` (`crates/designos-app/src/panels/typography.rs:674-677`) in the same `app.snapshot("Max lines")` undo step (`crates/designos-app/src/panels/typography.rs:671`), verified by `setting_max_lines_clears_max_height_on_text_layer_in_same_undo_step` (`crates/designos-app/src/shell/tests/typography.rs:312-342`).

5. **How `designos-mcp` and the linter read truncation:**
   - **`designos-mcp`**:
     - `build_updated_node` (`crates/designos-mcp/src/tools/mod.rs:273-280,300-309`) accepts `"auto_resize"` (`"NONE" | "WIDTH_AND_HEIGHT" | "HEIGHT" | "TRUNCATE"`, `crates/designos-mcp/src/tools/mod.rs:275-279`) and `"max_lines"` (`crates/designos-mcp/src/tools/mod.rs:300-305`), and preserves existing `tp.auto_resize` and `tp.max_lines` on unrelated updates (`crates/designos-mcp/src/tools/mod.rs:163`).
     - `spec_instance.rs` (`crates/designos-mcp/src/tools/spec_instance.rs:1122-1133,1300-1308`) parses `"auto_resize"` / `"textAutoResize"` / `"textTruncation"` and `"max_lines"` / `"maxLines"` in `designos_apply_spec` and exports `"auto_resize"` and `"max_lines"` in `designos_export_spec` (`crates/designos-mcp/src/tools/spec_instance.rs:318-323`).
   - **Linter (`crates/designos-doc/src/lint` & `crates/designos-render/src/lint_env.rs`)**:
     - `ApproxLintEnv::text_metrics` (`crates/designos-doc/src/lint/mod.rs:147-155`) and `RenderLintEnv::text_metrics` (`crates/designos-render/src/lint_env.rs:60-70`) set `TextMetrics::truncated` when `tp.auto_resize == TextAutoResize::Truncate` and lines exceed `tp.max_lines` or box height, triggering rule `"text-truncated"` (`crates/designos-doc/src/lint/rules/mod.rs:85-91,520-559`).
     - Verified in `crates/designos-mcp/tests/text_truncation_roundtrip.rs:13-220` (`plans/reports/agy-1467-truncate-text-evidence/mcp-truncation-test.log:9-12`).

---

## 3. Acceptance Table

| Gate | Command | Exit Code | Key Evidence & Citations |
|---|---|---:|---|
| **RED -> GREEN** | `CARGO_TARGET_DIR="$PWD/out/cargo" cargo test -p designos-app --bin designos --locked shell::tests::typography::` + `cargo test -p designos-mcp --test text_truncation_roundtrip --locked` | `101` -> `0` (`green_rc=0`, `mcp_rc=0`) | RED: `6 passed; 4 failed` (`plans/reports/agy-1467-truncate-text-evidence/red-test.log:11,14-16,50-55`). GREEN: `11 passed; 0 failed` (`plans/reports/agy-1467-truncate-text-evidence/green-test.log:20-32,34`) + `2 passed; 0 failed` (`plans/reports/agy-1467-truncate-text-evidence/mcp-truncation-test.log:9-12,14`). |
| **Mutant check** | Mutant 1 (`&sel[..sel.len().min(1)]` in `set_truncate_text`, `crates/designos-app/src/panels/typography.rs:655`) & Mutant 2 (`if true` for `Max lines`, `crates/designos-app/src/panels/typography.rs:1057`) | `101` & `101` | Mutant 1 killed by `truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step` (`9 passed; 1 failed`, `plans/reports/agy-1467-truncate-text-evidence/mutant1.log:14,26-29,36`). Mutant 2 killed by `max_lines_appears_only_while_truncating` and `truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step` (`8 passed; 2 failed`, `plans/reports/agy-1467-truncate-text-evidence/mutant2.log:11,14,26-27,33-34,41`). |
| **Studio screenshot** | `DESIGNOS_POPOVER_RENDER_DIR="$PWD/out/evidence" CARGO_TARGET_DIR="$PWD/out/cargo" cargo test -p designos-app --bin designos --locked shell::tests::typography::headless_popover_render_truncate_off_and_on_max_lines_2 -- --nocapture` | `0` | Crops committed at `plans/reports/agy-1467-truncate-text-evidence/type-settings-truncate-off-crop.png` (`312x728`, `crates/designos-app/src/shell/tests/typography.rs:479-484`) and `plans/reports/agy-1467-truncate-text-evidence/type-settings-truncate-on-max-lines-2-crop.png` (`312x760`, `crates/designos-app/src/shell/tests/typography.rs:509-515`); full renders at `out/evidence/1467-popover-truncate-off.png` (`1600x1088`) and `out/evidence/1467-popover-truncate-on-max-lines-2.png` (`1600x1088`). |
| **Static gates** | `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets --locked` + `python3 scripts/zero_fake_ui_gate.py` | `0` (`fmt_rc=0`, `clippy_rc=0`, `17/17 PASS`) | `fmt_rc=0` (`plans/reports/agy-1467-truncate-text-evidence/fmt.log:1`); `clippy_rc=0` with `0` warnings in touched files (`plans/reports/agy-1467-truncate-text-evidence/clippy.log:46-47`); `Gate Verdict: PASS (17/17 invariants satisfied)` (`plans/reports/agy-1467-truncate-text-evidence/zero-fake-ui.log:24`). |
| **Tests** | `cargo test -p designos-mcp --locked` + `cargo test -p designos-app --bin designos --locked` + `cargo test --workspace --lib --locked` | `0` (`test_mcp_rc=0`, `test_app_isolated_rc=0`, `test_ws_lib_rc=0`) | `designos-mcp`: all test binaries passed (`326 passed` lib + `2 passed` in `text_truncation_roundtrip`, `plans/reports/agy-1467-truncate-text-evidence/test-designos-mcp.log:334,717,725`); `designos-app`: `4,182 passed` in-process (`plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4306`) + `117 passed` across the `5` isolated suites (`plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:4421,4481,4512,4537,4582,4584`); `--workspace --lib`: `1,498 passed; 0 failed` (`plans/reports/agy-1467-truncate-text-evidence/test-workspace-lib.log:23,749,756,921,1070,1106,1117,1160,1492,1556,1563,1577,1579`). |

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

- **`FACT`**: `TextAutoResize::Truncate` (`crates/designos-doc/src/paint.rs:426-432`) and `TextProps::max_lines: Option<u32>` (`crates/designos-doc/src/node.rs:687`) were already wired in `designos-doc`, `designos-render` (`crates/designos-render/src/text.rs:304-336,545-577`), `designos-gpu` (`crates/designos-gpu/src/geom.rs:503-512`), `designos-fig` (`crates/designos-fig/src/rest_import.rs:439-446,3915-3920`, `crates/designos-fig/src/fig_export.rs:2481-2496`), and `designos-mcp` (`crates/designos-mcp/src/tools/mod.rs:273-309`, `crates/designos-mcp/src/tools/spec_instance.rs:1122-1133`), but `details_popover` in `crates/designos-app/src/panels/typography.rs:760-1120` had no `Truncate text` toggle or `Max lines` field.
- **`FACT`**: Keeping `details_rows(&ss_tags, false)` equal to `(1 + 7 + chips + 8) as f32 + 3.0` (`728.0px` popover height, `crates/designos-app/src/panels/typography.rs:745-755`) and placing `"Truncate text"` at `next() - 8.0` (`y = 1026.0..1050.0` inside the popover bottom `322.0..1050.0`, `crates/designos-app/src/panels/typography.rs:1045-1047`) preserves `0.0pt` drift in `shell::tests::layout_unchanged::f::layout_unchanged_f` (`plans/reports/agy-1467-truncate-text-evidence/test-designos-app.log:3506`), while growing the popover by `32.0px` (`760.0px` total height) when `truncating == true` (`crates/designos-app/src/panels/typography.rs:754,1057-1064`).
- **`FACT`**: When `"Truncate text"` is hidden or unchecked/mixed, `clear_max_lines_test_rects` (`crates/designos-app/src/panels/typography.rs:730-742,1066`) clears both `widgets::test_rect_id("det-max-lines")` and `Id::new(("field-rect", "det-max-lines"))` from `ctx.data_mut`, ensuring hit-test helpers (`has_control` / `has_field` in `crates/designos-app/src/shell/tests/typography.rs:92-104`) and UI automation never observe stale rects after toggling truncation off.
- **`INFERENCE`**: Restoring `TextAutoResize::None` when `"Truncate text"` is turned off (`crates/designos-app/src/panels/typography.rs:660-662`) matches Figma's fixed-size text box behavior after truncation and preserves `tp.max_lines` on the layer so re-enabling `"Truncate text"` remembers the previous line limit (`crates/designos-app/src/shell/tests/typography.rs:249-252`).
- **`ASSUMPTION`**: None remaining.

### Weakest Link

- **Weakest link**: Live windowed GPU screenshot capture on the Untitled UI PRO v8.0 kit could not be executed because the host session had `HIDIdleTime < 300s` (`0s`–`15s`). This is mitigated by the headless `egui` + `cosmic-text` software rasterizer in `headless_popover_render_truncate_off_and_on_max_lines_2` (`crates/designos-app/src/shell/tests/typography.rs:447-519`), which renders the exact `details_popover` widget tree and font atlas into `plans/reports/agy-1467-truncate-text-evidence/type-settings-truncate-off-crop.png` (`312x728`) and `plans/reports/agy-1467-truncate-text-evidence/type-settings-truncate-on-max-lines-2-crop.png` (`312x760`), both visually inspected via `read_file`.

---

## 5. Git & Pull Request

- **Branch**: `jangtrinh/agy-1467-truncate-text` (based on `origin/main` = `a98ec2b1bcd6668092cbb4e97d8c82f305be8d54`)
- **Draft PR**: Pending creation in final step (will be updated immediately below)

Status: READY
