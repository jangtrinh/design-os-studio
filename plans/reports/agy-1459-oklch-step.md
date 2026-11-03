# Report: `#1459` — Colour picker: arrow keys step the OKLCH and OKLAB values

- **Issue:** `#1459` (Part of `#14`)
- **Branch:** `jangtrinh/agy-1459-oklch-step` (base commit `9a294a1b0d4f25596cdfca8c8b92f965084a59e7`)
- **Worktree:** `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1459-oklch-step`

---

## 0. What Could Not Be Verified & Why

1. **Live windowed OS capture was not used; visual evidence was captured via the deterministic headless `egui` software rasterizer (`crates/designos-app/src/widgets/picker.rs:3162-3328`).**
   - Per the headless visual evidence rule (`brief-agy-1459-oklch-step.txt:54-55`, `brief-common-argon.txt:48-50`), `headless_picker_render_oklch_before_and_after_three_arrowup_on_h` (`crates/designos-app/src/widgets/picker.rs:3232-3328`) rendered the colour picker panel in `Fmt::Oklch` before and after three `ArrowUp` presses on `H` (`"56.93% 0.14 254.47"` -> `"56.93% 0.14 257.47"`) without opening a desktop window or touching the OS clipboard/screen, and wrote the tight `240x514` Studio picker panel crops `plans/reports/agy-1459-oklch-step-evidence/picker-oklch-before-3-arrowup-h.png` and `plans/reports/agy-1459-oklch-step-evidence/picker-oklch-after-3-arrowup-h.png`.
2. **In the full `cargo test -p designos-app --bin designos --locked` run (`4,225` active tests), `2` unrelated tests failed (`4210 passed; 2 failed; 13 ignored`, `plans/reports/agy-1459-oklch-step-evidence/07-test-designos-app.log:4273`, `test_app_rc=101` at `plans/reports/agy-1459-oklch-step-evidence/07-test-designos-app.log:4276`) and passed when rerun alone (`isolated_rerun_rc=0` at `plans/reports/agy-1459-oklch-step-evidence/07-test-designos-app.log:4304`):**
   - `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` (`plans/reports/agy-1459-oklch-step-evidence/07-test-designos-app.log:4257-4260`): failed in the full run because the sibling scratch `HOME` path `/Users/jangtrinh/orca/workspaces/design-os-studio/.s1459/home` produced a 129-byte Unix socket path exceeding the 103-byte `SUN_LEN` check at `crates/designos-app/src/agent_approval_channel.rs:286`; passed in isolation (`1 passed; 0 failed`, `plans/reports/agy-1459-oklch-step-evidence/07-test-designos-app.log:4287-4289`).
   - `vecops::tests::two_deferred_text_commands_both_convert_in_order` (`plans/reports/agy-1459-oklch-step-evidence/07-test-designos-app.log:4263-4266`): failed under 4,225-test parallel font-loader contention (`"both texts still waiting for slow fonts"` at `crates/designos-app/src/vecops.rs:1882`); passed in isolation (`1 passed; 0 failed`, `plans/reports/agy-1459-oklch-step-evidence/07-test-designos-app.log:4300-4302`).

---

## 1. Skills Loaded & What Changed Because of Them

- **`es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md:1-60`)**: Enforced the RED -> GREEN -> MUTANT -> GATE loop with scope strictly restricted to `crates/designos-app/src/widgets/picker.rs` and evidence files in `plans/reports/agy-1459-oklch-step-evidence/`.
- **`es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md:1-120`)**: Maintained an explicit epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), diagnosed `egui 0.35.0`'s `Focus::begin_pass` / `Focus::end_pass` cardinal focus navigation on unmodified `ArrowUp`/`ArrowDown` (`egui-0.35.0/src/memory/mod.rs:568-585,605-610`), and verified test strength by killing both required mutants (`plans/reports/agy-1459-oklch-step-evidence/03-mutant-1-ignore-shift.log:58-61`, `plans/reports/agy-1459-oklch-step-evidence/04-mutant-2-always-first-component.log:70-73`).
- **`es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md:1-90`)**: Avoided nested `ctx.data_mut` lock guards in `grouped_ok_text` (`crates/designos-app/src/widgets/picker.rs:1844-1927`), keeping temporary buffer reads/writes scoped to short non-overlapping closures (`crates/designos-app/src/widgets/picker.rs:1879,1896-1900,1911`).
- **`es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md:1-90`)**: Preserved the colour picker's existing `Paint::Solid(Color)` commit path (`crates/designos-app/src/widgets/picker.rs:1400-1402,1452-1467`) and `properties::show_picker` transaction handling (`crates/designos-app/src/panels/properties.rs:2454-2526`) without touching scene compilation or GPU caches.
- **`design-os-color-decision` (`/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1459-oklch-step/.agent/skills/design-os-color-decision/SKILL.md:1-60`)**: Verified perceptual OKLCH/OKLAB channel bounds (`L` in `0..=100%`, `C` in `0.0..=0.4`, `H` wrapped modulo `360°`, `a`/`b` in `-0.4..=0.4`) and 3-decimal `Alt`-stepped (`0.001`) precision retention (`crates/designos-app/src/widgets/picker.rs:579-586,1792-1827`).

---

## 2. Re-anchor Answers & Epistemic Ledger

### 2.1 Re-anchor Answers (Section 2 of `brief-agy-1459-oklch-step.txt`)

1. **Where the OKLCH and OKLAB field (`"picker-ok"`) is drawn, edited, and parsed in `crates/designos-app/src/widgets/picker.rs`:**
   - **Formatting (`State::ok_cells` & `State::ok_text`)**: `State::ok_cells` (`crates/designos-app/src/widgets/picker.rs:496-501`) formats the 3 channel strings via `fmt_ok_channel` (`crates/designos-app/src/widgets/picker.rs:607-613`); `State::ok_text` (`crates/designos-app/src/widgets/picker.rs:504-509`) formats `[L%, C, H]` or `[L%, a, b]` via `fmt_ok_token` (`crates/designos-app/src/widgets/picker.rs:616-622`) and appends ` / <a>` when `self.hsv.a < 1.0 - 0.5 / 255.0`.
   - **Drawing & editing (`Fmt::Oklch | Fmt::Oklab` arm in `show`)**: At `crates/designos-app/src/widgets/picker.rs:1397-1402`, `show` computes `let disp = st.ok_text(st.fmt);` and calls `grouped_ok_text(ui, r, "picker-ok", st, &disp, theme::cr(theme::RADIUS_CONTROL), 3.0)` (`crates/designos-app/src/widgets/picker.rs:1844-1927`; on base `9a294a1b`, this called `grouped_text` at `crates/designos-app/src/widgets/picker.rs:1687-1699`).
   - **Parsing & committing (`State::apply_ok_input` & `parse_ok_channel`)**: When `grouped_ok_text` returns `Some(s)`, `st.apply_ok_input(st.fmt, &s, cur[3])` (`crates/designos-app/src/widgets/picker.rs:512-604`) parses plain `"L% C H [/ a]"` or CSS `oklch(...)` / `oklab(...)` functional strings via `parse_ok_channel` (`crates/designos-app/src/widgets/picker.rs:625-666`), updates `st.oklch`, `st.oklab`, and `st.hsv`, and returns `Some([r, g, b, out_a])` into `typed` (`crates/designos-app/src/widgets/picker.rs:1400-1401`), which `show` writes into `paint` and `out.changed = true` (`crates/designos-app/src/widgets/picker.rs:1452-1467`).
2. **How `crates/designos-app/src/widgets/field.rs` (`edit_num_text`, `edit_hex_text`) handles `ArrowUp`, `ArrowDown`, `Shift`, and `Alt`:**
   - **`edit_num_text` (`crates/designos-app/src/widgets/field.rs:141-196`)**: While `ui.memory(|m| m.has_focus(id))` is true (`crates/designos-app/src/widgets/field.rs:141`), scans `ui.input_mut(|i| i.events)` for `egui::Event::Key { key: egui::Key::ArrowUp | egui::Key::ArrowDown, pressed: true, .. }` (`crates/designos-app/src/widgets/field.rs:144-146`), removes the event (`crates/designos-app/src/widgets/field.rs:147`), computes `dir = 1.0` (`ArrowUp`) or `-1.0` (`ArrowDown`) (`crates/designos-app/src/widgets/field.rs:148`), and computes `step = if modifiers.alt { 0.1 } else if modifiers.shift { 10.0 } else { 1.0 }` (`crates/designos-app/src/widgets/field.rs:149-155`). It then seeds `id` and `seed_id` in `ui.data_mut`, calls `edit_text_hint`, re-requests focus, selects the value, and returns `res.or(Some(next))` immediately (`crates/designos-app/src/widgets/field.rs:163-171`).
   - **`edit_hex_text` (`crates/designos-app/src/widgets/field.rs:212-246`)**: Uses the same `i.events` removal and `TextEdit::load_state` pattern (`crates/designos-app/src/widgets/field.rs:214-239`) to step hex values while focused and return `res.or(Some(next_hex))`.

### 2.2 Epistemic Ledger (`es:fable-thinking`)

- **`FACT`**: On base `9a294a1b`, `"picker-ok"` in `crates/designos-app/src/widgets/picker.rs:1397-1402` rendered through `grouped_text` (`crates/designos-app/src/widgets/picker.rs:1687-1699`), which delegates directly to `field::edit_text` without intercepting `ArrowUp` or `ArrowDown`.
- **`FACT`**: In `egui 0.35.0`, `Focus::begin_pass` sets `focus_direction = FocusDirection::Up | Down` on unmodified `ArrowUp`/`ArrowDown` unless `focused_widget.filter.vertical_arrows` is `true`, and `TextEdit::singleline` sets `vertical_arrows: false` (`crates/designos-app/src/widgets/field.rs:70-79`). Calling `m.move_focus(egui::FocusDirection::None)` and `m.set_focus_lock_filter(id, EventFilter { horizontal_arrows: true, vertical_arrows: true, escape: true, ..Default::default() })` in `grouped_ok_text` (`crates/designos-app/src/widgets/picker.rs:1904-1906,1920-1923`) keeps focus locked on `"picker-ok"` across unmodified and modified arrow key presses.
- **`FACT`**: Inside the colour picker popover (`crates/designos-app/src/panels/properties.rs:2406-2538`), `"picker-hex"` (`grouped_hex_text`), `"picker-ch"` (`grouped_num`), and `"picker-ok"` (`grouped_ok_text`) all emit `out.changed = true` on each `ArrowUp`/`ArrowDown` press, which immediately updates `app.doc` via `pk.tx.ed.revert(&mut app.doc); apply_picker(&mut app.doc, &mut pk); app.changed();` (`crates/designos-app/src/panels/properties.rs:2461-2464`) and commits `app.run_tx(tx)` (`crates/designos-app/src/panels/properties.rs:2525`) as one undo step when the picker closes (verified across `Fmt::Hex`, `Fmt::Rgb`, `Fmt::Oklch`, and `Fmt::Oklab` in `oklch_and_oklab_arrow_key_press_is_one_undo_step`, `crates/designos-app/src/widgets/picker.rs:3042-3108`).

---

## 3. Implementation Summary & Weakest Link

### 3.1 What Changed (`crates/designos-app/src/widgets/picker.rs`)

1. **`step_ok_text(fmt, cur_triple, text, caret_char, dir, mult) -> Option<(String, usize)>` (`crates/designos-app/src/widgets/picker.rs:1702-1841`)**:
   - Supports both the picker's 3-token format (`"L% C H [/ a]"` / `"L% a b [/ a]"`) and pasted CSS functional forms (`oklch(...)` / `oklab(...)`) (`crates/designos-app/src/widgets/picker.rs:1716-1752`).
   - Refuses to step when the caret is inside an `/ a` alpha suffix (`caret_char > sp` at `crates/designos-app/src/widgets/picker.rs:1742-1744`), preserving the ruling that an alpha part is not stepped.
   - Resolves the component index `idx` (`0`, `1`, or `2`) under `caret_char` so a caret on a space/comma separator steps the component to its left and a caret at the start steps component `0` (`let idx = if caret_char < tokens[1].0 { 0 } else if caret_char < tokens[2].0 { 1 } else { 2 };`, `crates/designos-app/src/widgets/picker.rs:1771-1777`).
   - Steps `L` by `1% * mult` clamped to `0.0..=100.0%` (`crates/designos-app/src/widgets/picker.rs:1794-1805`), `C` by `0.01 * mult` clamped to `0.0..=0.4` (`crates/designos-app/src/widgets/picker.rs:1806-1810`), `H` by `1° * mult` wrapped via `.rem_euclid(360.0)` (`crates/designos-app/src/widgets/picker.rs:1811-1819`), and `a`/`b` by `0.01 * mult` clamped to `-0.4..=0.4` (`crates/designos-app/src/widgets/picker.rs:1820-1824`), using `designos_export::util::num` (up to 3 decimals) for `0.001`-stepped `C`/`a`/`b` tokens so `Alt` steps (`±0.001`) are not rounded away (`crates/designos-app/src/widgets/picker.rs:1809,1823`).
   - Reconstructs the formatted text in the field's own format and maps the caret back onto the same component (`new_caret = new_start + rel.min(new_len)`, `crates/designos-app/src/widgets/picker.rs:1830-1840`).
2. **`grouped_ok_text` (`crates/designos-app/src/widgets/picker.rs:1844-1927`) & `State::apply_ok_input` (`crates/designos-app/src/widgets/picker.rs:579-586`)**:
   - Intercepts `ArrowUp` and `ArrowDown` while `"picker-ok"` has focus, applies the `field.rs:149-155` modifier rule (`Alt -> 0.1`, `Shift -> 10.0`, plain `-> 1.0`, `crates/designos-app/src/widgets/picker.rs:1872-1878`), stores the stepped string and `CCursorRange`, locks `vertical_arrows: true` so `egui` does not move focus away from `"picker-ok"` (`crates/designos-app/src/widgets/picker.rs:1903-1907,1919-1924`), and returns `Some(next_text)` to commit immediately through `st.apply_ok_input(st.fmt, &s, cur[3])` (`crates/designos-app/src/widgets/picker.rs:1399-1401`).
   - Preserves 3-decimal `C`/`a`/`b` tokens (`exact_3dec`) across subsequent nudges in `State::apply_ok_input` (`crates/designos-app/src/widgets/picker.rs:579-586`) and `grouped_ok_text` (`crates/designos-app/src/widgets/picker.rs:1892-1917`).
3. **Undo behaviour comparison (Ruling 4)**:
   - Inside the open colour picker popover (`crates/designos-app/src/panels/properties.rs:2406-2538`), `"picker-ok"` (`grouped_ok_text`), `"picker-hex"` (`grouped_hex_text`), and `"picker-ch"` (`grouped_num`) all behave identically: each `ArrowUp`/`ArrowDown` press sets `out.changed = true`, which immediately updates `app.doc` via `pk.tx.ed.revert(&mut app.doc); apply_picker(&mut app.doc, &mut pk); app.changed();` (`crates/designos-app/src/panels/properties.rs:2461-2464`) and commits `app.run_tx(tx)` (`crates/designos-app/src/panels/properties.rs:2525`) as one undo step when the popover closes (whereas the inline hex field on the properties Fill/Stroke row outside the popover, `crates/designos-app/src/panels/properties.rs:1897`, runs `app.edit_selected` directly because no popover transaction is open).

### 3.2 Weakest Link & Guarding Test

- **Weakest link:** Keeping `egui` keyboard focus and caret position on the stepped component across repeated unmodified `ArrowUp`/`ArrowDown` presses when a component's character width changes (e.g. `H` wrapping `"359"` -> `"0"` -> `"1"` -> `"2"`, or `C` stepping `"0.14"` -> `"0.141"` with `Alt+ArrowUp` and then stepping `H`), where `egui::Focus::begin_pass` would otherwise interpret unmodified vertical arrows as directional widget focus navigation and where `State::ok_text`'s 2-decimal display formatting would otherwise overwrite a 3-decimal `Alt`-stepped token on the next frame.
- **Guarding tests:** `oklch_and_oklab_caret_stays_on_stepped_component_and_respects_separators` (`crates/designos-app/src/widgets/picker.rs:2999-3039`) and `oklch_arrow_keys_step_l_c_h_with_plain_shift_and_alt_modifiers` (`crates/designos-app/src/widgets/picker.rs:2817-2904`).

---

## 4. Acceptance Table & Verification Evidence

| # | Gate | Command | Exit Code | Key Evidence & Citations |
|---|---|---|---|---|
| **1** | **RED -> GREEN (Null control)** | `CARGO_TARGET_DIR="$PWD/out/cargo" cargo test -p designos-app --bin designos --locked widgets::picker::tests -- --nocapture` | `101` -> `0` (`green_rc=0`) | RED on base `9a294a1b`: `25 passed; 5 failed` (`plans/reports/agy-1459-oklch-step-evidence/01-red-on-base.log:9,22-24,29,37,77-83`). GREEN on head: `31 passed; 0 failed` (`plans/reports/agy-1459-oklch-step-evidence/02-green-null-control.log:9,28,34-39,42,44`). |
| **2** | **Two Mutants Killed** | Mutant 1 (`m.shift { 1.0 }` at `crates/designos-app/src/widgets/picker.rs:1875`) & Mutant 2 (`let idx = 0;` at `crates/designos-app/src/widgets/picker.rs:1771`) | `101` & `101` (`mutant1_rc=101`, `mutant2_rc=101`) | Mutant 1 ("the step ignores `Shift`"): `28 passed; 2 failed` (`plans/reports/agy-1459-oklch-step-evidence/03-mutant-1-ignore-shift.log:25-39,55-61`). Mutant 2 ("the caret component is always the first"): `26 passed; 4 failed` (`plans/reports/agy-1459-oklch-step-evidence/04-mutant-2-always-first-component.log:22-56,65-73`). |
| **3** | **Visual Evidence** | `DESIGNOS_1459_EVIDENCE_DIR="$PWD/plans/reports/agy-1459-oklch-step-evidence" ... cargo test -p designos-app --bin designos --locked widgets::picker::tests::headless_picker_render_oklch_before_and_after_three_arrowup_on_h` | `0` | Committed `240x514` Studio picker crops: `plans/reports/agy-1459-oklch-step-evidence/picker-oklch-before-3-arrowup-h.png` (`"56.93% 0.14 254.47"`, contrast `4.50 : 1`) and `plans/reports/agy-1459-oklch-step-evidence/picker-oklch-after-3-arrowup-h.png` (`"56.93% 0.14 257.47"`, contrast `4.54 : 1`). Visually inspected via `Read`. |
| **4a** | **Rustfmt** | `cargo fmt --all -- --check; echo fmt_rc=$?` | `0` (`fmt_rc=0`) | `fmt_rc=0` (`plans/reports/agy-1459-oklch-step-evidence/05-fmt.log:1`). |
| **4b** | **Clippy** | `cargo clippy --workspace --all-targets --locked` | `0` (`clippy_rc=0`) | `clippy_rc=0` with `0` warnings in `crates/designos-app/src/widgets/picker.rs` (`plans/reports/agy-1459-oklch-step-evidence/06-clippy.log:401-415`). |
| **4c** | **App binary tests** | `cargo test -p designos-app --bin designos --locked` + isolated reruns of the 2 contention failures | `101` full -> `0` isolated (`isolated_rerun_rc=0`) | Full run: `4210 passed; 2 failed; 13 ignored` (`plans/reports/agy-1459-oklch-step-evidence/07-test-designos-app.log:4273`). Isolated reruns of `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` and `vecops::tests::two_deferred_text_commands_both_convert_in_order`: `1 passed` each (`isolated_rerun_rc=0`, `plans/reports/agy-1459-oklch-step-evidence/07-test-designos-app.log:4287-4304`). |
| **4d** | **Workspace lib tests** | `cargo test --workspace --locked --lib` | `0` (`test_ws_lib_rc=0`) | `1,501 passed; 0 failed; 3 ignored` across all workspace lib targets (`plans/reports/agy-1459-oklch-step-evidence/08-test-workspace-lib.log:1637,1644,1658,1660`). |
| **4e** | **Zero Fake UI Gate** | `python3 scripts/zero_fake_ui_gate.py` | `0` (`zero_fake_ui_rc=0`) | `Gate Verdict: PASS (17/17 invariants satisfied)` (`plans/reports/agy-1459-oklch-step-evidence/09-zero-fake-ui.log:24,26`). |

### Pasted Gate Outputs

#### 4.1 RED -> GREEN (`plans/reports/agy-1459-oklch-step-evidence/01-red-on-base.log:9,22-39,76-85` & `plans/reports/agy-1459-oklch-step-evidence/02-green-null-control.log:9,28-44`)

```text
# RED on base 9a294a1b (plans/reports/agy-1459-oklch-step-evidence/01-red-on-base.log:9,22-39,76-85)
running 30 tests
...
test widgets::picker::tests::oklch_and_oklab_caret_stays_on_stepped_component_and_respects_separators ... FAILED
test widgets::picker::tests::oklab_arrow_keys_step_a_and_b_by_hundredths_and_clamp ... FAILED
test widgets::picker::tests::oklch_arrow_keys_step_l_c_h_with_plain_shift_and_alt_modifiers ... FAILED
...
test widgets::picker::tests::oklch_hue_wraps_from_359_to_0_and_lightness_stops_at_100 ... FAILED
...
test widgets::picker::tests::oklch_and_oklab_arrow_key_press_is_one_undo_step ... FAILED
...
failures:
    widgets::picker::tests::oklab_arrow_keys_step_a_and_b_by_hundredths_and_clamp
    widgets::picker::tests::oklch_and_oklab_arrow_key_press_is_one_undo_step
    widgets::picker::tests::oklch_and_oklab_caret_stays_on_stepped_component_and_respects_separators
    widgets::picker::tests::oklch_arrow_keys_step_l_c_h_with_plain_shift_and_alt_modifiers
    widgets::picker::tests::oklch_hue_wraps_from_359_to_0_and_lightness_stops_at_100

test result: FAILED. 25 passed; 5 failed; 0 ignored; 0 measured; 4194 filtered out; finished in 0.25s

# GREEN on head (plans/reports/agy-1459-oklch-step-evidence/02-green-null-control.log:9,28-44)
running 31 tests
...
test widgets::picker::tests::oklch_hue_wraps_from_359_to_0_and_lightness_stops_at_100 ... ok
...
test widgets::picker::tests::oklch_and_oklab_caret_stays_on_stepped_component_and_respects_separators ... ok
test widgets::picker::tests::oklch_arrow_keys_step_l_c_h_with_plain_shift_and_alt_modifiers ... ok
test widgets::picker::tests::oklab_arrow_keys_step_a_and_b_by_hundredths_and_clamp ... ok
test widgets::picker::tests::pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness ... ok
test widgets::picker::tests::oklch_and_oklab_arrow_key_press_is_one_undo_step ... ok
test widgets::picker::tests::headless_picker_render_oklch_before_and_after_three_arrowup_on_h ... ok
test widgets::picker::tests::value_cell_at_rest_has_no_alpha_and_never_elides_across_grid ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 4194 filtered out; finished in 0.25s

green_rc=0
```

#### 4.2 Mutant Kills (`plans/reports/agy-1459-oklch-step-evidence/03-mutant-1-ignore-shift.log:25-39,54-61` & `plans/reports/agy-1459-oklch-step-evidence/04-mutant-2-always-first-component.log:22-56,64-73`)

```text
# Mutant 1: "the step ignores Shift" (`else if m.shift { 1.0 }` in `grouped_ok_text`, crates/designos-app/src/widgets/picker.rs:1875)
thread 'widgets::picker::tests::oklch_arrow_keys_step_l_c_h_with_plain_shift_and_alt_modifiers' (77204156) panicked at crates/designos-app/src/widgets/picker.rs:2882:9:
assertion `left == right` failed
  left: "57.93% 0.14 254.47"
 right: "66.93% 0.14 254.47"
thread 'widgets::picker::tests::oklab_arrow_keys_step_a_and_b_by_hundredths_and_clamp' (77204152) panicked at crates/designos-app/src/widgets/picker.rs:2972:9:
assertion `left == right` failed
  left: "56.93% -0.03 -0.14"
 right: "56.93% 0.06 -0.14"
failures:
    widgets::picker::tests::oklab_arrow_keys_step_a_and_b_by_hundredths_and_clamp
    widgets::picker::tests::oklch_arrow_keys_step_l_c_h_with_plain_shift_and_alt_modifiers

test result: FAILED. 28 passed; 2 failed; 0 ignored; 0 measured; 4194 filtered out; finished in 0.25s
mutant1_rc=101

# Mutant 2: "the caret component is always the first" (`let idx = 0;` in `step_ok_text`, crates/designos-app/src/widgets/picker.rs:1771)
failures:
    widgets::picker::tests::oklab_arrow_keys_step_a_and_b_by_hundredths_and_clamp
    widgets::picker::tests::oklch_and_oklab_caret_stays_on_stepped_component_and_respects_separators
    widgets::picker::tests::oklch_arrow_keys_step_l_c_h_with_plain_shift_and_alt_modifiers
    widgets::picker::tests::oklch_hue_wraps_from_359_to_0_and_lightness_stops_at_100

test result: FAILED. 26 passed; 4 failed; 0 ignored; 0 measured; 4194 filtered out; finished in 0.26s
mutant2_rc=101
```

#### 4.3 Static & Test Gates (`05-fmt.log`, `06-clippy.log`, `07-test-designos-app.log`, `08-test-workspace-lib.log`, `09-zero-fake-ui.log`) & `out/cargo` Cleanup

```text
# plans/reports/agy-1459-oklch-step-evidence/05-fmt.log:1
fmt_rc=0

# plans/reports/agy-1459-oklch-step-evidence/06-clippy.log:412-415
warning: `designos-app` (bin "designos-studio" test) generated 3 warnings (3 duplicates)
warning: `designos-app` (bin "designos" test) generated 3 warnings (2 duplicates)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 21s
clippy_rc=0

# plans/reports/agy-1459-oklch-step-evidence/07-test-designos-app.log:4269-4304
failures:
    agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root
    vecops::tests::two_deferred_text_commands_both_convert_in_order

test result: FAILED. 4210 passed; 2 failed; 13 ignored; 0 measured; 0 filtered out; finished in 407.28s
test_app_rc=101
=== ISOLATED RERUN OF THE 2 FAILED TESTS ===
running 1 test
test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4224 filtered out; finished in 0.00s
running 1 test
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4224 filtered out; finished in 1.50s
isolated_rerun_rc=0

# plans/reports/agy-1459-oklch-step-evidence/08-test-workspace-lib.log:1658-1660
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
test_ws_lib_rc=0

# plans/reports/agy-1459-oklch-step-evidence/09-zero-fake-ui.log:23-26
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
zero_fake_ui_rc=0

# out/cargo cleanup verification
$ rm -rf out/cargo out/evidence && ls out/cargo 2>&1; echo "ls_out_cargo_rc=$?"
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```

---

## 5. Assumptions

- None beyond the five rulings in `/Users/jangtrinh/Products/design-os-studio/plans/261030-2041-e1-wave-1/brief-agy-1459-oklch-step.txt:21-38`.

Status: COMPLETE
