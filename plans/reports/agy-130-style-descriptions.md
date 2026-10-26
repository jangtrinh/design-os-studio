# Report — DEV · #130: Edit style descriptions and show them on hover in the style pickers

## 1. What Could NOT Be Verified (and Why)

- **Interactive OS window hover delay with a physical mouse on macOS WindowServer:** Headless `designos --screenshot` runs without an OS cursor, so `style-picker-tooltip-hover.png` uses a dedicated `--panel effect-styles:hover` screenshot hook (`set_screenshot_hover_tip(true)` in `panels/properties.rs` + `panels/style_picker.rs`) that highlights the first described style row via `widgets::menu::preset_path` and renders its tooltip at `Order::Tooltip` with `egui::Frame::popup`. Actual multi-frame pointer hover (`Event::PointerMoved` + 30 frames to cross egui's tooltip delay) is verified end-to-end in `shell::tests::style_edit::style_picker_and_styles_list_show_description_tooltip_only_when_non_empty` across the Effect/Grid style picker menu, the Fill/Stroke colour style menu, and the Inspector Styles list.
- Everything else in the acceptance contract was verified directly and is recorded below.

---

## 2. Acceptance Table (Commands & Pasted Output)

| # | Acceptance Item | Command | Result / Output |
|---|---|---|---|
| **1(a)** | Doc test: `set_style_description` sets the text in one undo step, one undo restores the old text, and `sort_position`, name, and bindings are unchanged; setting the same text is a no-op | `cargo test -p designos-doc --locked set_style_description_sets_text_in_one_undo_step_and_keeps_other_fields` | `PASS` (`red.log` -> `green.log`) |
| **1(b)** | Doc test: a library style returns `StyleOpError::LibraryStyle` and the document is unchanged | `cargo test -p designos-doc --locked set_style_description_refuses_library_style` | `PASS` (`red.log` -> `green.log`) |
| **1(c)** | Shell test: open `"Edit style"`, type a description, commit; native save (`to_json`) and load (`from_json`) keeps it; one undo restores `""`; library style edit is refused | `cargo test -p designos-app --bin designos --locked editing_a_style_description_in_the_popover_survives_native_save_and_load` | `PASS` (`red.log` -> `green.log`) |
| **1(d)** | Shell test: style picker menu row, colour-style menu row, and Styles list row carry the description as hover tooltip when non-empty; empty description and folder header rows show no tooltip | `cargo test -p designos-app --bin designos --locked style_picker_and_styles_list_show_description_tooltip_only_when_non_empty` | `PASS` (`red.log` -> `green.log`) |
| **1(e)** | `.fig` test: a file whose styles have no description round-trips with every description still empty, and a style with a description keeps it across `.fig` export and import (without modifying `crates/designos-fig/src/*`) | `cargo test -p designos-fig --test fig_effect_grid_styles --locked style_description_fig_round_trip_keeps_empty_and_non_empty_descriptions` | `PASS` (`red.log` -> `green.log`) |
| **2** | Rustfmt & Clippy (`-D warnings`) | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings` | `PASS` (exit 0, 0 warnings) |
| **3** | Full crate test suites (`designos-doc`, `designos-fig`, `designos-app --bin designos`) | `cargo test -p designos-doc --locked && cargo test -p designos-fig --locked && cargo test -p designos-app --bin designos --locked` | `PASS` (`designos-doc`: 653 passed; `designos-fig`: all integration suites passed; `designos-app`: 2957 passed, 0 failed) |
| **4** | Zero Fake UI gate & spacing ratchet | `python3 scripts/zero_fake_ui_gate.py && python3 scripts/check_spacing_relationships.py` | `PASS` (`17/17 invariants satisfied`; `PASS: no count rose above the baseline`) |
| **5(a)** | `1440x900` screenshot: `"Edit style"` popover with `"Description"` row filled on Untitled UI PRO VARIABLES v8.0 copy | `"$CARGO_TARGET_DIR/debug/designos" "$PWD/out/untitled-ui-copy.fig" --size 1440x900 --panel edit-style-effect:desc --screenshot plans/reports/agy-130-style-descriptions-evidence/edit-style-description.png` | `PASS` (`plans/reports/agy-130-style-descriptions-evidence/edit-style-description.png`, 273,087 bytes) |
| **5(b)** | `1440x900` screenshot: style picker with description tooltip showing on hover on Untitled UI PRO VARIABLES v8.0 copy | `"$CARGO_TARGET_DIR/debug/designos" "$PWD/out/untitled-ui-copy.fig" --size 1440x900 --panel effect-styles:hover --screenshot plans/reports/agy-130-style-descriptions-evidence/style-picker-tooltip-hover.png` | `PASS` (`plans/reports/agy-130-style-descriptions-evidence/style-picker-tooltip-hover.png`, 296,261 bytes) |

### Pasted Output — Red Phase (`plans/reports/agy-130-style-descriptions-evidence/red.log`)

```text
=== (a) & (b) designos-doc ===
running 2 tests
test styles_ops::tests::set_style_description_sets_text_in_one_undo_step_and_keeps_other_fields ... FAILED
test styles_ops::tests::set_style_description_refuses_library_style ... FAILED

failures:
    styles_ops::tests::set_style_description_refuses_library_style
    styles_ops::tests::set_style_description_sets_text_in_one_undo_step_and_keeps_other_fields

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 651 filtered out; finished in 0.03s

=== (c) & (d) designos-app ===
running 1 test
test shell::tests::style_edit::editing_a_style_description_in_the_popover_survives_native_save_and_load ... FAILED
thread 'shell::tests::style_edit::editing_a_style_description_in_the_popover_survives_native_save_and_load' panicked at crates/designos-app/src/shell/tests.rs:194:113:
nothing recorded for se-description

running 1 test
test shell::tests::style_edit::style_picker_and_styles_list_show_description_tooltip_only_when_non_empty ... FAILED
thread 'shell::tests::style_edit::style_picker_and_styles_list_show_description_tooltip_only_when_non_empty' panicked at crates/designos-app/src/shell/tests/style_edit.rs:497:5:
assertion failed: s.shell.app.set_style_description_to("Effect style 1", "Soft card drop shadow")

=== (e) designos-fig ===
running 1 test
test style_description_fig_round_trip_keeps_empty_and_non_empty_descriptions ... FAILED
thread 'style_description_fig_round_trip_keeps_empty_and_non_empty_descriptions' panicked at crates/designos-fig/tests/fig_effect_grid_styles.rs:965:5:
assertion `left == right` failed
  left: ""
 right: "Primary brand surface"
```

### Pasted Output — Green Phase (`plans/reports/agy-130-style-descriptions-evidence/green.log`)

```text
=== (a) & (b) designos-doc ===
running 2 tests
test styles_ops::tests::set_style_description_sets_text_in_one_undo_step_and_keeps_other_fields ... ok
test styles_ops::tests::set_style_description_refuses_library_style ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 651 filtered out; finished in 0.01s

=== (c) & (d) designos-app ===
running 1 test
test shell::tests::style_edit::editing_a_style_description_in_the_popover_survives_native_save_and_load ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2964 filtered out; finished in 0.31s

running 1 test
test shell::tests::style_edit::style_picker_and_styles_list_show_description_tooltip_only_when_non_empty ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2964 filtered out; finished in 0.39s

=== (e) designos-fig ===
running 1 test
test style_description_fig_round_trip_keeps_empty_and_non_empty_descriptions ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.05s
```

### Pasted Output — Format, Clippy & Gates

```text
$ cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 40s

$ python3 scripts/zero_fake_ui_gate.py && python3 scripts/check_spacing_relationships.py
 Gate Verdict: PASS (17/17 invariants satisfied)
literal geometry outside theme.rs: offset_literals=1675, inset_literals=45, radius_literals=260, font_literals=25, nonconcentric_pairs=2
note: 11 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

---

## 3. Changes Per File

- [`crates/designos-doc/src/styles_ops.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-130-style-descriptions/crates/designos-doc/src/styles_ops.rs):
  - Added `pub fn set_style_description(doc: &Document, key: &str, text: &str) -> Result<Command, StyleOpError>` next to `rename_style`.
  - Validates that `key` exists in `doc.styles` (`StyleOpError::UnknownStyle`) and is not a library style (`is_library_style(doc, key)` -> `StyleOpError::LibraryStyle`).
  - Trims trailing whitespace via `text.trim_end()`. When the trimmed description equals the stored description, returns `Ok(Command::Batch(Vec::new()))` so no undo step is recorded.
  - Otherwise updates only `StyleMeta.description` (or inserts a `StyleMeta` preserving `name`, `property`, `bindings`, and `sort_position`) in a single `Command::SetStyles` undo step.
  - Added unit tests `set_style_description_sets_text_in_one_undo_step_and_keeps_other_fields` and `set_style_description_refuses_library_style`.
- [`crates/designos-app/src/styles.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-130-style-descriptions/crates/designos-app/src/styles.rs):
  - Added `pub fn style_description<'a>(doc: &'a Document, key: &str) -> &'a str` returning `doc.style_meta.get(key).map(|m| m.description.as_str()).unwrap_or("")`.
  - Added `App::set_style_description_to(&mut self, key: &str, text: &str) -> bool` next to `rename_style_to`, routing `set_style_description(&self.doc, key, text)` through `self.run_style_op(op)`.
  - Updated `App::run_style_op_then` so an empty `Command::Batch(v)` (`v.is_empty()`) does not push a no-op command into `Tx`, returning `false` without adding an undo step.
- [`crates/designos-app/src/panels/style_edit.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-130-style-descriptions/crates/designos-app/src/panels/style_edit.rs):
  - Grew `popover_ui` row count by 1 (`let rows = 2 + usize::from(count > 1) + kind_rows(&style, item);`).
  - Added the `"Description"` row (`index = 1`) in `body_ui` directly below `"Name"` (`index = 0`), recording `"se-description"` for UI tests, guarding `is_read_only(app, key)` with `LIBRARY_STYLE_TIP`, and committing `field::text_input(ui, rc, "se-description", desc)` via `app.set_style_description_to(key, &text)`. Shifted subsequent rows to start at `let mut first = 2;`.
- [`crates/designos-app/src/panels/style_picker.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-130-style-descriptions/crates/designos-app/src/panels/style_picker.rs):
  - In `menu_model`, attached `.tooltip(crate::styles::style_description(&app.doc, &e.key))` to each style `MenuItem` while leaving folder header rows without tooltips.
  - Added `set_screenshot_hover_tip` and the `SCREENSHOT_HOVER_TIP` hook in `header_button` so `--panel effect-styles:hover` (or `grid-styles:hover`) highlights the first described style row and renders its tooltip for headless screenshot capture.
- [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-130-style-descriptions/crates/designos-app/src/panels/properties.rs):
  - In `paint_title` (Fill and Stroke colour-style picker menus), attached `.tooltip(crate::styles::style_description(&app.doc, &e.key))` to each style `MenuItem` while leaving folder header rows without tooltips.
  - In `styles_list` (Inspector local styles list covering Text, Color, Effect, and Grid styles), attached `resp.clone().on_hover_text(desc)` when `!desc.is_empty() && !over_edit`, leaving folder header rows without tooltips and keeping the trailing `"Edit style"` icon button tooltip unshadowed when hovered.
  - In `preset_popup`, parsed `let (base, sub) = spec.split_once(':').unwrap_or((spec, ""));` to support `:hover` on `effect-styles`/`grid-styles` and `:desc` on `edit-style-*`.
- [`crates/designos-app/src/shell/tests/style_edit.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-130-style-descriptions/crates/designos-app/src/shell/tests/style_edit.rs):
  - Added `editing_a_style_description_in_the_popover_survives_native_save_and_load` and `style_picker_and_styles_list_show_description_tooltip_only_when_non_empty`.
- [`crates/designos-fig/tests/fig_effect_grid_styles.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-130-style-descriptions/crates/designos-fig/tests/fig_effect_grid_styles.rs):
  - Added `style_description_fig_round_trip_keeps_empty_and_non_empty_descriptions`.

---

## 4. Every Style List Found & How Each Shows the Tooltip

A full search (`git grep -n "style_entries" crates/designos-app/src`) identified all three style-picking surfaces in `designos-app`:

1. **Effect & Grid Style Picker Menus (`crates/designos-app/src/panels/style_picker.rs` -> `menu_model`, lines 94–104):**
   - Iterates `style_entries_of(&app.doc, kind_of(prop))`.
   - Folder header rows (`MenuItem::new(e.folder.clone()).disabled(true)`) carry an empty tooltip (`""`).
   - Style rows call `.tooltip(crate::styles::style_description(&app.doc, &e.key))`. `widgets/menu.rs` (`draw_item`, line 445) calls `resp.on_hover_text(&it.tooltip)` only when `!it.tooltip.is_empty()`.
2. **Fill & Stroke Colour Style Menus (`crates/designos-app/src/panels/properties.rs` -> `paint_title`, lines 251–261):**
   - Iterates `crate::styles::style_entries(&app.doc, false)` filtered by `StyleProperty::Fill` or `StyleProperty::Stroke`.
   - Folder header rows (`MenuItem::new(e.folder.clone()).disabled(true)`) carry an empty tooltip (`""`).
   - Style rows call `.tooltip(crate::styles::style_description(&app.doc, &e.key))`.
3. **Inspector Styles Section List — Text, Color, Effect, and Grid Styles (`crates/designos-app/src/panels/properties.rs` -> `styles_list`, lines 492–649):**
   - Iterates `[StyleKind::Text, StyleKind::Color, StyleKind::Effect, StyleKind::Grid]` (text styles are picked and edited from this list; there is no separate text style menu file).
   - Folder header rows (`r` at line 512) do not call `on_hover_text`.
   - Painted style rows (`let resp = ui.interact(r, ...)`) read `let desc = crate::styles::style_description(&app.doc, &e.key);` and call `resp.clone().on_hover_text(desc)` when `!desc.is_empty() && !over_edit` (so hovering the trailing `"Edit style"` button shows `"Edit style"` / `LIBRARY_STYLE_TIP` without colliding with the row tooltip).

---

## 5. UI Decisions, Reasons & Skills Loaded

- **Skills Loaded:** `es:dev`, `es:feature-dev`, `es:rust`, `es:rust-gpu-canvas`, `es:designer` (`SKILL.md`, `craft-floor.md`, `checklist.md`), `es-astra-work`, `es:fable-thinking`.
- **UI Decisions:**
  1. **Single `"Description"` row directly under `"Name"` in `panels/style_edit.rs`:** Built with `popover::row(ui, body, 1, "Description", ...)` and `field::text_input(ui, rc, "se-description", desc)`, matching `"Name"` in height (`popover::ROW_H = 28px >= 24px` pointer target), vertical rhythm (`popover::ROW_GAP = 6px`, popover height grows by exactly one row via `popover::height(rows)`), label column (`popover::LABEL_W = 88px`), and commit behaviour (commits on Enter or focus loss).
  2. **Library style read-only protection at both entry and field level:** `style_edit::is_read_only` continues to block opening the `"Edit style"` popover for library styles and shows `LIBRARY_STYLE_TIP` (`"Edit in the library file"`). Inside `body_ui`, if any code path ever opens the popover for a library style, the `"Description"` row suppresses `field::text_input` and attaches `.on_hover_text(LIBRARY_STYLE_TIP)`.
  3. **Hover suppression over the trailing `"Edit style"` action button in `styles_list`:** In `properties.rs:styles_list`, the row's description tooltip is attached only when `!over_edit`, preventing dual overlapping tooltips when the user hovers the trailing edit icon button.
  4. **Zero new magic spacing literals:** Used `theme::MENU_PADDING` for the screenshot hover preview offset so `scripts/check_spacing_relationships.py` remains strictly at baseline (`offset_literals = 1675`).

---

## 6. Assumptions & Epistemic Ledger

- **`FACT`:** `StyleMeta.description` already exists in `crates/designos-doc/src/lib.rs` with `#[serde(default, skip_serializing_if = "String::is_empty")]`, so native `.designos` JSON serialization omits empty descriptions and preserves non-empty descriptions without format changes.
- **`FACT`:** `crates/designos-fig/src/fig_export.rs` (~line 683/697) and `crates/designos-fig/src/rest_import.rs` (~line 366) already export and import `styleDescription` on shared styles; our round-trip test `style_description_fig_round_trip_keeps_empty_and_non_empty_descriptions` confirmed `.fig` export/import works unchanged without touching `crates/designos-fig/src/*`.
- **`FACT`:** `MenuItem::tooltip` in `crates/designos-app/src/widgets/menu.rs` (~line 445) already renders `resp.on_hover_text(&it.tooltip)` only when `!it.tooltip.is_empty()`.
- **`INFERENCE`:** Because `Command::SetStyles` swaps `doc.style_meta` with its pre-command snapshot on undo, a style that had no entry in `doc.style_meta` prior to `set_style_description` returns to having no entry in `doc.style_meta` after undo (`style_description(&doc, key) == ""`).
- **`ASSUMPTION`:** None remaining.

Status: DONE
